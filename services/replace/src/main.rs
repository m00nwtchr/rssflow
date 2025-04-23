#![warn(clippy::pedantic)]

use rssflow_service::service::ServiceBuilder;

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
