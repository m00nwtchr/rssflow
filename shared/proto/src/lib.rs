use std::{fmt::Debug, time::Duration};

use tokio::time::sleep;
#[warn(clippy::pedantic)]
use tonic::transport::{Server, server::Router};

pub mod feed;
pub mod node {
	tonic::include_proto!("rssflow.node");
}
pub mod registry {
	use std::str::FromStr;

	use tonic::{
		Request, Response,
		transport::{Channel, Endpoint},
	};

	use crate::node::{ProcessRequest, ProcessResponse, node_service_client::NodeServiceClient};

	tonic::include_proto!("rssflow.registry");

	impl Node {
		pub fn endpoint(&self) -> anyhow::Result<Endpoint> {
			Ok(Endpoint::from_str(&self.address)?)
		}

		pub async fn client(&self) -> anyhow::Result<NodeServiceClient<Channel>> {
			Ok(NodeServiceClient::new(self.endpoint()?.connect().await?))
		}

		pub async fn process(
			&self,
			req: ProcessRequest,
		) -> anyhow::Result<Response<ProcessResponse>> {
			let mut req = Request::new(req);
			req.metadata_mut().insert("x-node", self.node_name.parse()?);
			Ok(self.client().await?.process(req).await?)
		}
	}
}
pub mod websub {
	use std::str::FromStr;

	use anyhow::anyhow;

	use crate::impl_name;

	tonic::include_proto!("rssflow.websub");

	impl_name!(WebSubEvent, "rssflow.websub");

	impl FromStr for WebSub {
		type Err = anyhow::Error;

		fn from_str(header: &str) -> Result<Self, Self::Err> {
			let mut hub = None;
			let mut topic = None;

			// Split the header into individual link parts
			for part in header.split(',') {
				let segments: Vec<&str> = part.trim().split(';').collect();
				if segments.len() < 2 {
					continue;
				}

				let url_part = segments[0].trim();
				let rel_part = segments[1].trim();

				if !url_part.starts_with('<') || !url_part.ends_with('>') {
					continue;
				}

				// Extract the URL and rel values
				let url = &url_part[1..url_part.len() - 1];
				let rel = rel_part
					.split('=')
					.nth(1)
					.map_or("", |s| s.trim_matches('"'));

				match rel {
					"hub" => hub = Some(url.to_string()),
					"self" => topic = Some(url.to_string()),
					_ => (),
				}
			}

			Ok(WebSub {
				topic: topic.ok_or_else(|| anyhow!(""))?,
				hub: hub.ok_or_else(|| anyhow!(""))?,
			})
		}
	}
}
#[cfg(feature = "cache")]
pub mod cache;

#[cfg(debug_assertions)]
const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("proto_descriptor");

#[cfg(debug_assertions)]
pub fn add_reflection_service(mut s: Server, name: impl Into<String>) -> anyhow::Result<Router> {
	let reflection = tonic_reflection::server::Builder::configure()
		.register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
		.with_service_name(name)
		.build_v1()?;

	Ok(s.add_service(reflection))
}

#[cfg(not(debug_assertions))]
pub fn add_reflection_service(s: Server, _name: impl Into<String>) -> anyhow::Result<Server> {
	Ok(s)
}

#[macro_export]
macro_rules! impl_name {
	($type:ty, $package:expr) => {
		impl ::prost::Name for $type {
			const NAME: &'static str = stringify!($type);
			const PACKAGE: &'static str = $package;
		}

		impl From<&$type> for ::prost_types::Any {
			fn from(value: &$type) -> Self {
				::prost_types::Any {
					value: ::prost::Message::encode_to_vec(value),
					type_url: <$type as ::prost::Name>::type_url(),
				}
			}
		}

		impl From<$type> for ::prost_types::Any {
			fn from(value: $type) -> Self {
				Self::from(&value)
			}
		}

		impl TryFrom<&::prost_types::Any> for $type {
			type Error = ::prost::DecodeError;

			fn try_from(any: &::prost_types::Any) -> Result<Self, Self::Error> {
				if any.type_url == <Self as ::prost::Name>::type_url() {
					::prost::Message::decode(any.value.as_slice())
				} else {
					Err(::prost::DecodeError::new("invalid type"))
				}
			}
		}

		impl TryFrom<::prost_types::Any> for $type {
			type Error = ::prost::DecodeError;

			fn try_from(any: ::prost_types::Any) -> Result<Self, Self::Error> {
				Self::try_from(&any)
			}
		}
	};
}

/// Retry an async operation up to `retries` times with a fixed `delay` between attempts.
///
/// - `operation`: a closure returning a `Future` that yields `Result<T, E>`.
/// - `retries`: how many times to retry on failure.
/// - `delay`: how long to wait between retries.
///
/// Returns `Ok(T)` on the first successful attempt, or the last `Err(E)` if all retries fail.
pub async fn retry_async<Op, Fut, T, E>(
	mut operation: Op,
	mut retries: usize,
	delay: Duration,
) -> Result<T, E>
where
	E: Debug,
	Op: FnMut() -> Fut,
	Fut: Future<Output = Result<T, E>>,
{
	loop {
		match operation().await {
			Ok(v) => return Ok(v),
			Err(err) if retries > 0 => {
				retries -= 1;
				eprintln!("Operation failed: {:?}. Retries left: {}", err, retries);
				sleep(delay).await;
			}
			Err(err) => return Err(err),
		}
	}
}
