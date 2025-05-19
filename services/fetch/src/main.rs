#![warn(clippy::pedantic)]

use rssflow_service::{proto, proto::node::node_service_server::NodeServiceServer};
use runesys::{Service, config::config};

mod service;

#[derive(Service)]
#[service("Fetch")]
#[server(NodeServiceServer)]
#[fd_set(proto::FILE_DESCRIPTOR_SET)]
struct FetchNode {
	conn: redis::aio::MultiplexedConnection,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	runesys::tracing::init(&FetchNode::INFO);
	let config = config(&FetchNode::INFO);

	let redis = redis::Client::open(config.redis_url.as_str())?;
	let conn = redis.get_multiplexed_async_connection().await?;
	let node = FetchNode { conn };

	Ok(node.builder().run().await?)
}
