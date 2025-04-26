#![warn(clippy::pedantic)]

use rssflow_service::{config::config, service::ServiceBuilder, service_info};

mod service;

struct RetrieveNode {
	conn: redis::aio::MultiplexedConnection,
}

service_info!("Retrieve");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	rssflow_service::tracing::init(&SERVICE_INFO);
	let config = config(&SERVICE_INFO);

	let redis = redis::Client::open(config.redis_url.as_str())?;
	let conn = redis.get_multiplexed_async_connection().await?;

	let node = RetrieveNode { conn };
	ServiceBuilder::new(SERVICE_INFO)?
		.with_node_service(node)
		.await
		.run()
		.await
}
