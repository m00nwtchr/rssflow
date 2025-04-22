#![warn(clippy::pedantic)]

use std::{
	collections::HashMap,
	net::SocketAddr,
	ops::Deref,
	sync::{Arc, Mutex},
};

use proto::{
	registry::Node,
	websub::{WebSub, web_sub_service_server::WebSubServiceServer},
};
use tokio::net::TcpListener;
use tonic::transport::Server;
use url::Url;
use uuid::Uuid;

use crate::router::app;

pub mod router;
mod service;
mod ws;

pub async fn websub_check(public_url: &Url) {
	// let resp = reqwest::get(public_url.join("/websub/check")?).await?;

	// resp.error_for_status()?;
	// Ok(())
}

#[derive(Debug, Default, Clone)]
pub struct Subscription {
	web_sub: WebSub,
	nodes: Vec<Node>,
}

#[derive(Debug, Default)]
pub struct WebSubInner {
	subscriptions: Mutex<HashMap<Uuid, Subscription>>,
	ws: Mutex<HashMap<WebSub, Uuid>>,
}

#[derive(Debug, Clone, Default)]
pub struct WebSubSVC(Arc<WebSubInner>);

impl Deref for WebSubSVC {
	type Target = WebSubInner;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt::init();
	let (health_reporter, health_service) = tonic_health::server::health_reporter();
	health_reporter
		.set_serving::<WebSubServiceServer<WebSubSVC>>()
		.await;

	let port = std::env::var("GRPC_PORT")
		.ok()
		.and_then(|v| v.parse::<u16>().ok())
		.unwrap_or(50051);
	let http_port = std::env::var("HTTP_PORT")
		.ok()
		.and_then(|v| v.parse::<u16>().ok())
		.unwrap_or(3435);

	let ip = "::".parse().unwrap();

	let addr = SocketAddr::new(ip, port);
	let http_addr = SocketAddr::new(ip, http_port);

	let svc = WebSubSVC::default();

	tracing::info!("WebSub service at: {}", addr);
	tracing::info!("WebSub endpoint at: {}", http_addr);
	let server = Server::builder()
		.add_service(health_service)
		.add_service(WebSubServiceServer::new(svc.clone()));

	let http_server = axum::serve(TcpListener::bind(http_addr).await?, app(svc));

	tokio::select! {
		res = server.serve(addr) => {
			if let Err(err) = res {
				tracing::error!("Failed to start gRPC server: {err}");
			}
		}
		res = http_server => {
			if let Err(err) = res {
				tracing::error!("Failed to start HTTP server: {err}");
			}
		}
	}

	Ok(())
}
