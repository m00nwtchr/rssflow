use std::{str::FromStr, time::Duration};

use anyhow::anyhow;
use axum::{
	body::Bytes,
	extract::{Path, Query, State},
	http::{HeaderMap, HeaderName, StatusCode},
	response::IntoResponse,
	routing::{get, post},
	Router,
};
use chrono::Utc;
use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};
use sha2::{Sha256, Sha384, Sha512};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
	app::AppState,
	flow::node::{Data, DataKind, NodeTrait},
	route::internal_error,
	subscriber::websub::{WebSub, WebSubSubscriber},
};

#[allow(clippy::declare_interior_mutable_const)]
const X_HUB_SIGNATURE: HeaderName = HeaderName::from_static("x-hub-signature");

pub async fn receive(
	Path(uuid): Path<Uuid>,
	State(pool): State<SqlitePool>,
	State(state): State<AppState>,
	headers: HeaderMap,
	body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	let record = sqlx::query!("SELECT secret, topic, hub FROM websub WHERE uuid = ?", uuid)
		.fetch_optional(&mut *conn)
		.await
		.map_err(internal_error)?;

	if let Some(record) = record {
		let signature = headers.get(X_HUB_SIGNATURE);

		let Some(signature) = signature
			.and_then(|v| v.to_str().ok())
			.and_then(|s| XHubSignature::from_str(s).ok())
		else {
			return Ok(StatusCode::OK);
		};

		let verified = signature
			.verify(record.secret.as_bytes(), &body)
			.map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;

		if verified {
			tracing::info!("Received WebSub push for `{}`", record.topic);

			state
				.web_sub_subscriber
				.handle(
					&state,
					&WebSub {
						topic: record.topic,
						hub: record.hub,
					},
					body,
				)
				.await
				.map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
		}
	}

	Ok(StatusCode::OK)
}

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(tag = "hub.mode", rename_all = "lowercase")]
pub enum Verification {
	Subscribe {
		#[serde(rename = "hub.topic")]
		topic: String,
		#[serde(rename = "hub.challenge")]
		challenge: String,
		#[serde_as(as = "DurationSeconds<String>")]
		#[serde(rename = "hub.lease_seconds")]
		lease_seconds: Duration,
	},
	Unsubscribe {
		#[serde(rename = "hub.topic")]
		topic: String,
		#[serde(rename = "hub.challenge")]
		challenge: String,
	},
}

#[tracing::instrument(skip(pool, verification))]
pub async fn verify(
	Path(uuid): Path<Uuid>,
	State(pool): State<SqlitePool>,
	Query(verification): Query<Verification>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	if let Some(record) = sqlx::query!("SELECT subscribed, topic FROM websub WHERE uuid = ?", uuid)
		.fetch_optional(&mut *conn)
		.await
		.map_err(internal_error)?
	{
		match verification {
			Verification::Subscribe {
				topic,
				challenge,
				lease_seconds,
			} => {
				let lease_end = Utc::now() + lease_seconds;
				sqlx::query!(
					"UPDATE websub SET lease_end = ? WHERE topic = ?",
					lease_end,
					record.topic
				)
				.execute(&mut *conn)
				.await
				.map_err(internal_error)?;

				if record.subscribed && topic.eq(&record.topic) {
					Ok((StatusCode::OK, challenge))
				} else {
					Err((StatusCode::BAD_REQUEST, "Bad request".to_string()))
				}
			}
			Verification::Unsubscribe { topic, challenge } => {
				if !record.subscribed && topic.eq(&record.topic) {
					tracing::info!("Unsubscribed from `{}`", record.topic);

					sqlx::query!("DELETE FROM websub WHERE topic = ?", record.topic)
						.execute(&mut *conn)
						.await
						.map_err(internal_error)?;
					Ok((StatusCode::OK, challenge))
				} else {
					Err((StatusCode::BAD_REQUEST, "Bad request".to_string()))
				}
			}
		}
	} else {
		Err((StatusCode::NOT_FOUND, "Not found".to_string()))
	}
}

pub fn router() -> Router<AppState> {
	Router::new()
		.route("/{uuid}", post(receive))
		.route("/{uuid}", get(verify))
		.route("/check", get(|| async {}))
}

#[derive(Debug)]
pub struct XHubSignature {
	method: String,
	signature: Vec<u8>,
}

impl XHubSignature {
	#[tracing::instrument(skip(secret, message))]
	pub fn verify(&self, secret: &[u8], message: &[u8]) -> anyhow::Result<bool> {
		Ok(match self.method.as_str() {
			#[cfg(feature = "sha1")]
			"sha1" => mac::verify_hmac::<sha1::Sha1>(&self.signature, secret, message)?,
			#[cfg(not(feature = "sha1"))]
			"sha1" => {
				tracing::error!("Unsupported sha1 signature on WebSub push");
				false
			}
			"sha256" => mac::verify_hmac::<Sha256>(&self.signature, secret, message)?,
			"sha384" => mac::verify_hmac::<Sha384>(&self.signature, secret, message)?,
			"sha512" => mac::verify_hmac::<Sha512>(&self.signature, secret, message)?,
			_ => {
				tracing::error!("Unknown signature algorithm");
				false
			}
		})
	}
}

impl FromStr for XHubSignature {
	type Err = anyhow::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let Some((method, signature)) = s.split_once('=') else {
			return Err(anyhow!(""));
		};

		Ok(XHubSignature {
			method: method.to_string(),
			signature: hex::decode(signature)?,
		})
	}
}

mod mac {
	use hmac::{
		digest::{
			block_buffer::Eager,
			consts::U256,
			core_api::{BlockSizeUser, BufferKindUser, CoreProxy, FixedOutputCore, UpdateCore},
			typenum::{IsLess, Le, NonZero},
			HashMarker,
		},
		Hmac, Mac,
	};

	pub fn verify_hmac<D>(signature: &[u8], secret: &[u8], message: &[u8]) -> anyhow::Result<bool>
	where
		D: CoreProxy,
		D::Core: HashMarker
			+ UpdateCore
			+ FixedOutputCore
			+ BufferKindUser<BufferKind = Eager>
			+ Default
			+ Clone,
		<D::Core as BlockSizeUser>::BlockSize: IsLess<U256>,
		Le<<D::Core as BlockSizeUser>::BlockSize, U256>: NonZero,
	{
		let mut hmac: Hmac<D> = Hmac::new_from_slice(secret)?;
		hmac.update(message);
		Ok(hmac.verify_slice(signature).is_ok())
	}
}
