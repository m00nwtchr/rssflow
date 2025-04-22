#![warn(clippy::pedantic)]

pub mod feed;
pub mod node {
	use tonic::Status;

	use crate::node::tfv::TryFromValue;

	tonic::include_proto!("rssflow.node");

	impl ProcessRequest {
		pub fn get_option<'a, T: 'a + TryFromValue<'a>>(
			&'a self,
			key: &str,
		) -> Option<Result<T, Status>> {
			self.options
				.as_ref()
				.and_then(|o| o.fields.get(key))
				.map(T::try_from_value)
				.map(|r| {
					r.map_err(|_| Status::invalid_argument(format!("wrong type for {key} option")))
				})
		}

		pub fn get_option_required<'a, T: 'a + TryFromValue<'a>>(
			&'a self,
			key: &str,
		) -> Result<T, Status> {
			match self.get_option(key) {
				Some(v) => Ok(v?),
				None => Err(Status::invalid_argument(format!("{key} option is missing")))?,
			}
		}
	}

	pub(crate) mod tfv {
		use anyhow::anyhow;
		use prost_types::{ListValue, Struct};

		pub trait TryFromValue<'a>: Sized {
			fn try_from_value(v: &'a prost_types::Value) -> anyhow::Result<Self>;
		}

		macro_rules! impl_try_from_value {
			($ty:ty, $kind_variant:ident) => {
				impl<'a> TryFromValue<'a> for &'a $ty {
					fn try_from_value(v: &'a prost_types::Value) -> anyhow::Result<Self> {
						if let Some(prost_types::value::Kind::$kind_variant(inner)) = &v.kind {
							Ok(inner)
						} else {
							Err(anyhow!("wrong type"))
						}
					}
				}
			};
		}

		impl_try_from_value!(f64, NumberValue);
		impl_try_from_value!(String, StringValue);
		impl_try_from_value!(bool, BoolValue);
		impl_try_from_value!(Struct, StructValue);
		impl_try_from_value!(ListValue, ListValue);
	}
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
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("proto_descriptor");

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
