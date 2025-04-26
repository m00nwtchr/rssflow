#![warn(clippy::pedantic)]

use rssflow_service::{service::ServiceBuilder, service_info};

mod service;

struct ReplaceNode;

service_info!("Replace");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	ServiceBuilder::new(SERVICE_INFO)?
		.with_node_service(ReplaceNode)
		.await
		.run()
		.await
}
