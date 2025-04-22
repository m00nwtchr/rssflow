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

struct ReplaceNode;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt::init();
	let (health_reporter, health_service) = tonic_health::server::health_reporter();
	health_reporter
		.set_serving::<NodeServiceServer<ReplaceNode>>()
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
		.unwrap_or(format!("http://filter:{port}"));

	let ip = "::".parse().unwrap();
	let addr = SocketAddr::new(ip, port);

	info!("Replace service at: {}", addr);

	let server = add_reflection_service(
		Server::builder(),
		proto::node::node_service_server::SERVICE_NAME,
	)?
		.add_service(health_service)
		.add_service(NodeServiceServer::new(ReplaceNode));

	let report = retry_async(
		|| async {
			let mut client = NodeRegistryClient::connect(registry_url.clone()).await?;

			client
				.register(RegisterRequest {
					node: Some(Node {
						address: service_url.clone(),
						node_name: "Replace".into(),
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
