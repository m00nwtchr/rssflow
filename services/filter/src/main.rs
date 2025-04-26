#![warn(clippy::pedantic)]

use rssflow_service::{service::ServiceBuilder, service_info};

mod service;

struct FilterNode;

service_info!("Filter");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	ServiceBuilder::new(SERVICE_INFO)?
		.with_node_service(FilterNode)
		.await
		.run()
		.await
}
