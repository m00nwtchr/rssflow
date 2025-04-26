use std::{str::FromStr, sync::Arc};

use axum::{
	Extension, Router,
	body::Bytes,
	extract::{Path, Query, State},
	http::{HeaderMap, StatusCode},
	response::IntoResponse,
	routing::{get, post},
};
use rssflow_service::{
	NodeExt,
	proto::{node::ProcessRequest, websub::WebSubEvent},
	service::ServiceState,
};
use sqlx::{PgPool, types::chrono::Utc};
use tracing::{info, instrument};
use uuid::Uuid;

use crate::{
	Subscription, WebSubSVC,
	ws::{Verification, X_HUB_SIGNATURE, XHubSignature},
};

async fn send_to_listeners(sub: &Subscription, body: Bytes) {
	let mut options = prost_types::Struct::default();
	options
		.fields
		.insert("url".to_string(), sub.web_sub.topic.clone().into());
	options
		.fields
		.insert("hub".to_string(), sub.web_sub.hub.clone().into());

	let payload: prost_types::Any = WebSubEvent {
		body: body.to_vec(),
	}
	.into();

	for node in &sub.nodes {
		let _ = node
			.process(ProcessRequest {
				payload: Some(payload.clone()),
				options: Some(options.clone()),
			})
			.await;
	}
}

#[instrument(skip_all)]
pub async fn receive(
	Path(uuid): Path<Uuid>,
	Extension(pool): Extension<PgPool>,
	headers: HeaderMap,
	body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	let record = sqlx::query!(
		"SELECT secret, topic, hub FROM subscription WHERE uuid = $1",
		uuid
	)
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
			info!("Received WebSub push for `{}`", record.topic);

			// send_to_listeners(&record, body).await;
		}
	}

	Ok(StatusCode::OK)
}

#[instrument(skip_all)]
pub async fn verify(
	Path(uuid): Path<Uuid>,
	Extension(pool): Extension<PgPool>,
	Query(verification): Query<Verification>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	if let Some(record) = sqlx::query!(
		"SELECT subscribed, topic FROM subscription WHERE uuid = $1",
		uuid
	)
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
					"UPDATE subscription SET lease_end = $1 WHERE topic = $2",
					lease_end,
					record.topic
				)
				.execute(&mut *conn)
				.await
				.map_err(internal_error)?;

				info!("Verified subscription: `{topic}`");

				if record.subscribed && topic.eq(&record.topic) {
					Ok((StatusCode::OK, challenge))
				} else {
					Err((StatusCode::BAD_REQUEST, "Bad request".to_string()))
				}
			}
			Verification::Unsubscribe { topic, challenge } => {
				if !record.subscribed && topic.eq(&record.topic) {
					info!("Unsubscribed from `{}`", record.topic);

					sqlx::query!("DELETE FROM subscription WHERE topic = $1", record.topic)
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

pub fn app(state: WebSubSVC) -> Router {
	Router::new()
		.route("/websub/{uuid}", post(receive))
		.route("/websub/{uuid}", get(verify))
		.route("/websub/check", get(|| async {}))
		.with_state(state)
}

fn internal_error<E>(err: E) -> (StatusCode, String)
where
	E: std::error::Error,
{
	(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
