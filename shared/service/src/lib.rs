#![warn(clippy::pedantic)]

use std::{fmt::Debug, time::Duration};

use proto::{
	FILE_DESCRIPTOR_SET,
	registry::{Node, RegisterRequest, node_registry_client::NodeRegistryClient},
};
pub use rssflow_proto as proto;
use rssflow_proto::node::{
	ProcessRequest, ProcessResponse,
	node_service_server::{NodeService, NodeServiceServer},
};
use tokio::time::sleep;
use tonic::{
	Request, Response, Status,
	server::NamedService,
	service::{Routes, RoutesBuilder},
	transport::{Server, server::Router},
};

use crate::config::ServiceConfig;

pub mod config;
pub mod service;

#[cfg(debug_assertions)]
pub fn add_reflection_service(
	s: &mut RoutesBuilder,
	name: impl Into<String>,
) -> anyhow::Result<()> {
	let reflection = tonic_reflection::server::Builder::configure()
		.register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
		.with_service_name(name)
		.build_v1()?;

	s.add_service(reflection);
	Ok(())
}

#[cfg(not(debug_assertions))]
pub fn add_reflection_service(s: Server, _name: impl Into<String>) -> anyhow::Result<Server> {
	Ok(s)
}

pub fn build_server<T: NodeService>(svc: T) -> anyhow::Result<Router> {
	let mut routes = Routes::builder();
	add_reflection_service(&mut routes, NodeServiceServer::<T>::NAME)?;

	Ok(Server::builder()
		.add_routes(routes.routes())
		.add_service(NodeServiceServer::new(svc)))
}

pub mod tracing {
	pub fn init() {
		let _ = tracing_subscriber::fmt::try_init();
	}
}

/// Retry an async operation up to `retries` times with a fixed `delay` between attempts.
///
/// - `operation`: a closure returning a `Future` that yields `Result<T, E>`.
/// - `retries`: how many times to retry on failure.
/// - `delay`: how long to wait between retries.
///
/// Returns `Ok(T)` on the first successful attempt, or the last `Err(E)` if all retries fail.
pub async fn retry_async<Op, Fut, T, E>(
	mut operation: Op,
	mut retries: usize,
	delay: Duration,
) -> Result<T, E>
where
	E: Debug,
	Op: FnMut() -> Fut,
	Fut: Future<Output = Result<T, E>>,
{
	loop {
		match operation().await {
			Ok(v) => return Ok(v),
			Err(err) if retries > 0 => {
				retries -= 1;
				eprintln!("Operation failed: {:?}. Retries left: {}", err, retries);
				sleep(delay).await;
			}
			Err(err) => return Err(err),
		}
	}
}

pub async fn report(name: &str, config: &ServiceConfig) -> anyhow::Result<()> {
	retry_async(
		|| async {
			let mut client = NodeRegistryClient::connect(config.registry_url.to_string()).await?;

			client
				.register(RegisterRequest {
					node: Some(Node {
						address: config.service_url.clone(),
						node_name: name.to_string(),
					}),
				})
				.await?;

			Ok(())
		},
		3,
		Duration::from_secs(2),
	)
	.await
}

pub fn check_node(request: &Request<ProcessRequest>, name: &str) -> Result<(), Status> {
	if let Some(node) = request.metadata().get("x-node") {
		if node != name {
			return Err(Status::not_found(format!(
				"node {} not found",
				node.to_str().unwrap()
			)));
		}
	}
	Ok(())
}
pub fn try_from_request<'a, T: TryFrom<&'a prost_types::Any> + prost::Name>(
	request: &'a ProcessRequest,
) -> Result<T, Status> {
	let Some(payload) = &request.payload else {
		return Err(Status::invalid_argument("payload missing"));
	};

	T::try_from(payload).map_err(|_| {
		Status::invalid_argument(format!(
			"payload is of wrong type, {} expected",
			T::type_url()
		))
	})
}
