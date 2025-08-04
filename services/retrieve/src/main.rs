#![warn(clippy::pedantic)]

use rssflow_service::{ServiceExt, proto, proto::node::node_service_server::NodeServiceServer};
use runesys::{Service, config::config};

mod service;

#[derive(Service)]
#[service("Retrieve")]
#[server(NodeServiceServer)]
#[fd_set(proto::FILE_DESCRIPTOR_SET)]
struct RetrieveNode {
	conn: redis::aio::MultiplexedConnection,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	runesys::tracing::init(&RetrieveNode::INFO);
	let config = config();

	let redis = redis::Client::open(config.redis_url.as_str())?;
	let conn = redis.get_multiplexed_async_connection().await?;

	let node = RetrieveNode { conn };
	Ok(node.builder().run().await?)
}
