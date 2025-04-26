use std::{str::FromStr, time::Duration};

use anyhow::anyhow;
use axum::http::HeaderName;
use base64::{Engine as _, engine::general_purpose};
use rand::RngCore;
use serde::Deserialize;
use serde_with::{DurationSeconds, serde_as};
use sha2::{Sha256, Sha384, Sha512};

// https://security.stackexchange.com/questions/95972/what-are-requirements-for-hmac-secret-key#96176
// https://www.w3.org/TR/websub/#x5-1-subscriber-sends-subscription-request
pub const HMAC_SECRET_LENGTH: usize = 64; // 64 bytes = 512 bits

#[allow(clippy::declare_interior_mutable_const)]
pub const X_HUB_SIGNATURE: HeaderName = HeaderName::from_static("x-hub-signature");

pub fn generate_hmac_secret() -> String {
	let mut bytes = [0u8; HMAC_SECRET_LENGTH];
	rand::rng().fill_bytes(&mut bytes);
	general_purpose::STANDARD.encode(&bytes)
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

pub mod mac {
	use hmac::{
		Hmac, Mac,
		digest::{
			HashMarker,
			block_buffer::Eager,
			consts::U256,
			core_api::{BlockSizeUser, BufferKindUser, CoreProxy, FixedOutputCore, UpdateCore},
			typenum::{IsLess, Le, NonZero},
		},
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
