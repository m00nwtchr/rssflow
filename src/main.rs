#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use std::{
	collections::HashMap,
	ops::Deref,
	sync::{Arc, Mutex},
};

use rssflow_service::{
	NodeExt, proto,
	proto::registry::{
		Empty, GetNodeRequest, GetNodeResponse, HeartbeatRequest, ListNodesResponse, Node,
		RegisterRequest,
		node_registry_server::{NodeRegistry, NodeRegistryServer},
	},
};
use runesys::Service;
use tonic::{Request, Response, Status};
use tonic_health::{ServingStatus, pb::HealthCheckRequest};
use tracing::{info, instrument};

mod app;
mod flow;
mod route;

use crate::app::app;

// #[global_allocator]
// static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug)]
struct RSSFlowInner {
	pub nodes: Mutex<HashMap<String, Node>>,
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
impl NodeRegistry for RSSFlow {
	#[instrument(skip_all)]
	async fn register(&self, request: Request<RegisterRequest>) -> Result<Response<Empty>, Status> {
		runesys::telemetry::propagation::accept_trace(&request);
		let Some(node) = request.into_inner().node else {
			return Err(Status::invalid_argument("Invalid node argument"));
		};

		if !node.node_name.is_empty() && !node.address.is_empty() {
			let response = node
				.health()
				.await
				.map_err(|e| Status::invalid_argument(e.to_string()))?
				.check(HealthCheckRequest {
					service: rssflow_service::proto::node::node_service_server::SERVICE_NAME
						.to_string(),
				})
				.await?
				.into_inner();

			if response.status == ServingStatus::Serving as i32 {
				info!("Successfully added Node: {}", node.node_name);
				self.nodes
					.lock()
					.unwrap()
					.insert(node.node_name.clone(), node);
				Ok(Response::new(Empty::default()))
			} else {
				Err(Status::unavailable(""))
			}
		} else {
			Err(Status::invalid_argument("Invalid node argument"))
		}
	}

	#[instrument(skip_all)]
	async fn heartbeat(
		&self,
		request: Request<HeartbeatRequest>,
	) -> Result<Response<Empty>, Status> {
		runesys::telemetry::propagation::accept_trace(&request);
		todo!()
	}

	#[instrument(skip_all)]
	async fn get_node(
		&self,
		request: Request<GetNodeRequest>,
	) -> Result<Response<GetNodeResponse>, Status> {
		runesys::telemetry::propagation::accept_trace(&request);
		let name = request.into_inner().name;
		if name.is_empty() {
			Err(Status::invalid_argument("Missing name argument"))
		} else {
			let node = self.nodes.lock().unwrap().get(&name).cloned();
			Ok(Response::new(GetNodeResponse { node }))
		}
	}

	#[instrument(skip_all)]
	async fn list_nodes(
		&self,
		request: Request<Empty>,
	) -> Result<Response<ListNodesResponse>, Status> {
		runesys::telemetry::propagation::accept_trace(&request);
		Ok(Response::new(ListNodesResponse {
			nodes: self.nodes.lock().unwrap().values().cloned().collect(),
		}))
	}
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	runesys::tracing::init(&RSSFlow::INFO);

	let svc = RSSFlow(Arc::new(RSSFlowInner {
		nodes: Mutex::default(),
	}));

	let app = app(svc.clone());
	let _ = svc
		.builder()
		.with_pg(|pool| async move { sqlx::migrate!().run(&pool).await })
		.await?
		.with_http(app.await?)
		.run()
		.await;
	Ok(())
}
