#![warn(clippy::pedantic)]

use std::{net::SocketAddr, time::Duration};

use proto::{
	add_reflection_service,
	node::node_service_server::NodeServiceServer,
	registry::{Node, RegisterRequest, node_registry_client::NodeRegistryClient},
	retry_async,
};
use tonic::transport::Server;
use tracing::info;

mod service;

pub fn default_ammonia() -> ammonia::Builder<'static> {
	let mut ammonia = ammonia::Builder::new();
	ammonia.add_generic_attributes(["style"]);
	ammonia
}

struct SanitiseNode {
	ammonia: ammonia::Builder<'static>,
}

impl Default for SanitiseNode {
	fn default() -> Self {
		Self {
			ammonia: default_ammonia(),
		}
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt::init();
	let (health_reporter, health_service) = tonic_health::server::health_reporter();
	health_reporter
		.set_serving::<NodeServiceServer<SanitiseNode>>()
		.await;

	let port = std::env::var("GRPC_PORT")
		.ok()
		.and_then(|v| v.parse::<u16>().ok())
		.unwrap_or(50051);

	let registry_url = std::env::var("REGISTRY")
		.ok()
		.unwrap_or("http://rssflow:50051".to_string());
	let service_url = std::env::var("SERVICE_URL")
		.ok()
		.unwrap_or(format!("http://sanitise:{port}"));

	let ip = "::".parse().unwrap();
	let addr = SocketAddr::new(ip, port);

	info!("Sanitise service at: {}", addr);

	let server = add_reflection_service(
		Server::builder(),
		proto::node::node_service_server::SERVICE_NAME,
	)?
	.add_service(health_service)
	.add_service(NodeServiceServer::new(SanitiseNode::default()));

	let report = retry_async(
		|| async {
			let mut client = NodeRegistryClient::connect(registry_url.clone()).await?;

			client
				.register(RegisterRequest {
					node: Some(Node {
						address: service_url.clone(),
						node_name: "Sanitise".into(),
					}),
				})
				.await?;

			Ok::<(), Box<dyn std::error::Error>>(())
		},
		3,
		Duration::from_secs(2),
	);

	tokio::join!(server.serve(addr), report).0?;

	Ok(())
}
