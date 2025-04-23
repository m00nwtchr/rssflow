#![warn(clippy::pedantic)]

use rssflow_service::{config::config, service::ServiceBuilder};
mod service;

struct FetchNode {
	conn: redis::aio::MultiplexedConnection,
}

pub const SERVICE_NAME: &str = "Fetch";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	rssflow_service::tracing::init();
	let config = config(SERVICE_NAME);

	let redis = redis::Client::open(config.redis_url.as_str())?;
	let conn = redis.get_multiplexed_async_connection().await?;
	let node = FetchNode { conn };

	ServiceBuilder::new(SERVICE_NAME)
		.await?
		.with_node_service(node)
		.await
		.run()
		.await
}
