#![warn(clippy::pedantic)]

use redis::ClientTlsConfig;
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

	let cert_vecs = rssflow_service::load_tls("REDIS");
	let client_tls = if let Some((client_cert, client_key)) = cert_vecs.0 {
		Some(ClientTlsConfig {
			client_cert,
			client_key,
		})
	} else {
		None
	};

	let redis = redis::Client::build_with_tls(
		config.redis_url.as_str(),
		TlsCertificates {
			client_tls,
			root_cert: cert_vecs.1,
		},
	)?;
	let conn = redis.get_multiplexed_async_connection().await?;

	let node = RetrieveNode { conn };
	Ok(node.builder().run().await?)
}
