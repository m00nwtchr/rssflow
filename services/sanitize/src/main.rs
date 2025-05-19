#![warn(clippy::pedantic)]

use rssflow_service::proto::{self, node::node_service_server::NodeServiceServer};
use runesys::Service;
use tracing::instrument;

mod service;

#[must_use]
pub fn default_ammonia() -> ammonia::Builder<'static> {
	let mut ammonia = ammonia::Builder::new();
	ammonia.add_generic_attributes(["style"]);
	ammonia
}

#[derive(Service)]
#[server(NodeServiceServer)]
#[fd_set(proto::FILE_DESCRIPTOR_SET)]
struct SanitizeNode {
	ammonia: ammonia::Builder<'static>,
}

impl Default for SanitizeNode {
	fn default() -> Self {
		Self {
			ammonia: default_ammonia(),
		}
	}
}

#[tokio::main]
#[instrument]
async fn main() -> Result<(), runesys::error::Error> {
	SanitizeNode::default().builder().run().await
}
