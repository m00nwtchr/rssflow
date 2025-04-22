#![warn(clippy::pedantic)]

use std::net::SocketAddr;

use rssflow_service::{
	add_reflection_service, config::config, proto::node::node_service_server::NodeServiceServer,
	report, service::ServiceBuilder,
};
use tonic::transport::Server;
use tracing::info;

mod service;

struct ReplaceNode;

pub const SERVICE_NAME: &str = "Replace";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	ServiceBuilder::new(SERVICE_NAME)
		.await?
		.with_node_service(ReplaceNode)
		.await
		.run()
		.await
}
