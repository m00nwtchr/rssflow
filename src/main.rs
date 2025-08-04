#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use std::{
	collections::HashMap,
	net::ToSocketAddrs,
	ops::Deref,
	str::FromStr,
	sync::{Arc, Mutex},
	time::Duration,
};

use anyhow::Context;
use rssflow_service::{
	NodeExt, proto,
	proto::{
		node::{NodeMeta, PingRequest, node_service_client::NodeServiceClient},
		registry::node_registry_server::{NodeRegistry, NodeRegistryServer},
	},
};
use runesys::Service;
use tonic::transport::Endpoint;

mod app;
mod flow;
mod route;

use crate::app::app;

// #[global_allocator]
// static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug)]
struct RSSFlowInner {
	pub nodes: Mutex<HashMap<String, NodeMeta>>,
}

#[derive(Service, Debug, Clone)]
#[server(NodeRegistryServer)]
#[fd_set(proto::FILE_DESCRIPTOR_SET)]
struct RSSFlow(Arc<RSSFlowInner>);

impl Deref for RSSFlow {
	type Target = RSSFlowInner;

	#[allow(clippy::explicit_deref_methods)]
	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

#[tonic::async_trait]
impl NodeRegistry for RSSFlow {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	runesys::tracing::init(&RSSFlow::INFO);

	let svc = RSSFlow(Arc::new(RSSFlowInner {
		nodes: Mutex::default(),
	}));

	let sd_task = {
		let svc = svc.clone();
		async move {
			loop {
				let resolve = "rssflow-headless:50051".to_socket_addrs();
				let Ok(addrs) = resolve else {
					tracing::error!("resolve failed: {resolve:?}");
					tokio::time::sleep(Duration::from_secs(5)).await;
					continue;
				};

				for addr in addrs {
					let url = format!("http://{addr}");
					tracing::debug!("trying to connect to {}", url);

					let Ok(endpoint) =
						Endpoint::from_str(&url).with_context(|| format!("create endpoint {url}"))
					else {
						continue;
					};
					let Ok(channel) = endpoint
						.connect()
						.await
						.with_context(|| format!("connect to endpoint {url}"))
					else {
						continue;
					};

					#[cfg(not(feature = "telemetry"))]
					let mut client = NodeServiceClient::new(channel);
					#[cfg(feature = "telemetry")]
					let mut client = NodeServiceClient::with_interceptor(
						channel,
						interceptor(telemetry::propagation::send_trace),
					);

					let Ok(response) = client.ping(PingRequest {}).await else {
						continue;
					};

					if let Some(node) = response.into_inner().node {
						let mut nodes = svc.nodes.lock().map_err(|_| {
							runesys::error::Error::Config("poison lock".to_string())
						})?;

						tracing::info!("node {} is alive", node.node_name);
						nodes.insert(node.node_name.clone(), node);
					}
				}

				tokio::time::sleep(Duration::from_secs(5)).await;
			}
		}
	};

	let app = app(svc.clone());
	let _ = svc
		.builder()
		.with_pg(|pool| async move { sqlx::migrate!().run(&pool).await })?
		.with_http(app.await?)
		.with_task(sd_task)
		.run()
		.await;
	Ok(())
}
