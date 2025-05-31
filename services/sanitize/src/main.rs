#![warn(clippy::pedantic)]

use rssflow_service::{
	ServiceExt,
	proto::{self, node::node_service_server::NodeServiceServer},
};
use runesys::Service;
use tracing::instrument;

mod service;

#[derive(Service)]
#[service("Sanitize")]
#[server(NodeServiceServer)]
#[fd_set(proto::FILE_DESCRIPTOR_SET)]
struct SanitizeNode {
	ammonia: ammonia::Builder<'static>,
}

impl Default for SanitizeNode {
	fn default() -> Self {
		let mut ammonia = ammonia::Builder::new();
		ammonia.add_generic_attributes(["style"]);
		Self { ammonia }
	}
}

#[tokio::main]
#[instrument]
async fn main() -> Result<(), runesys::error::Error> {
	SanitizeNode::default()
		.builder()
		.with_reporter()
		.run()
		.await
}
