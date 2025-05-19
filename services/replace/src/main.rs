#![warn(clippy::pedantic)]

use rssflow_service::{proto, proto::node::node_service_server::NodeServiceServer};
use runesys::Service;

mod service;

#[derive(Service)]
#[service("Replace")]
#[server(NodeServiceServer)]
#[fd_set(proto::FILE_DESCRIPTOR_SET)]
struct ReplaceNode;

#[tokio::main]
async fn main() -> Result<(), runesys::error::Error> {
	ReplaceNode.builder().run().await
}
