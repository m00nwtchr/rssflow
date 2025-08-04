#![warn(clippy::pedantic)]

use rssflow_service::{ServiceExt, proto, proto::node::node_service_server::NodeServiceServer};
use runesys::Service;

mod service;

#[derive(Service)]
#[service("Filter")]
#[server(NodeServiceServer)]
#[fd_set(proto::FILE_DESCRIPTOR_SET)]
struct FilterNode;

#[tokio::main]
async fn main() -> Result<(), runesys::error::Error> {
	FilterNode.builder().run().await
}
