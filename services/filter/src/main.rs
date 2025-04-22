#![warn(clippy::pedantic)]

use rssflow_service::service::ServiceBuilder;

mod service;

struct FilterNode;

pub const SERVICE_NAME: &str = "Filter";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	ServiceBuilder::new(SERVICE_NAME)
		.await?
		.with_node_service(FilterNode)
		.await
		.run()
		.await
}
