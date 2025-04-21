#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use std::{
	collections::{HashMap, HashSet},
	net::SocketAddr,
	ops::Deref,
	str::FromStr,
	sync::{Arc, Mutex},
};

use axum::extract::FromRef;
use proto::{
	add_reflection_service,
	node::{ProcessRequest, node_service_client::NodeServiceClient},
	registry::{
		Empty, GetNodeRequest, GetNodeResponse, HeartbeatRequest, ListNodesResponse, Node,
		RegisterRequest,
		node_registry_server::{NodeRegistry, NodeRegistryServer},
	},
};
use sqlx::{
	SqlitePool,
	sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use tokio::net::TcpListener;
use tonic::{
	Request, Response, Status,
	codegen::http,
	transport::{Endpoint, Server},
};
use tonic_health::pb::{
	HealthCheckRequest, health_check_response::ServingStatus, health_client::HealthClient,
};
use tracing::{info, instrument};

mod app;
mod config;
mod feed;
mod flow;
mod route;
mod subscriber;

use crate::{app::app, config::config};

// #[global_allocator]
// static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug)]
struct RSSFlowInner {
	pub pool: SqlitePool,
	pub nodes: Mutex<HashMap<String, Node>>,
}

#[derive(Debug, Clone)]
struct RSSFlow(Arc<RSSFlowInner>);

impl Deref for RSSFlow {
	type Target = RSSFlowInner;

	#[allow(clippy::explicit_deref_methods)]
	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl FromRef<RSSFlow> for SqlitePool {
	fn from_ref(input: &RSSFlow) -> Self {
		input.pool.clone()
	}
}

#[tonic::async_trait]
impl NodeRegistry for RSSFlow {
	#[instrument]
	async fn register(&self, request: Request<RegisterRequest>) -> Result<Response<Empty>, Status> {
		let Some(node) = request.into_inner().node else {
			return Err(Status::invalid_argument("Invalid node argument"));
		};

		if !node.node_name.is_empty() && !node.address.is_empty() {
			let end = node
				.endpoint()
				.map_err(|e| Status::invalid_argument("Invalid"))?
				.connect()
				.await
				.map_err(|e| Status::unavailable(e.to_string()))?;

			let resp = HealthClient::new(end)
				.check(HealthCheckRequest {
					service: proto::node::node_service_server::SERVICE_NAME.to_string(),
				})
				.await?
				.into_inner();

			if resp.status == ServingStatus::Serving as i32 {
				info!("Successfully added Node: {}", node.node_name);
				self.nodes
					.lock()
					.unwrap()
					.insert(node.node_name.clone(), node);
				Ok(Response::new(Default::default()))
			} else {
				Err(Status::unavailable(""))
			}
		} else {
			Err(Status::invalid_argument("Invalid node argument"))
		}
	}

	async fn heartbeat(
		&self,
		request: Request<HeartbeatRequest>,
	) -> Result<Response<Empty>, Status> {
		todo!()
	}

	async fn get_node(
		&self,
		request: Request<GetNodeRequest>,
	) -> Result<Response<GetNodeResponse>, Status> {
		let name = request.into_inner().name;
		if !name.is_empty() {
			let node = self.nodes.lock().unwrap().get(&name).cloned();
			Ok(Response::new(GetNodeResponse { node }))
		} else {
			Err(Status::invalid_argument("Missing name argument"))
		}
	}

	async fn list_nodes(
		&self,
		request: Request<Empty>,
	) -> Result<Response<ListNodesResponse>, Status> {
		Ok(Response::new(ListNodesResponse {
			nodes: self.nodes.lock().unwrap().values().cloned().collect(),
		}))
	}
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt::init();
	let config = config().await;

	let (health_reporter, health_service) = tonic_health::server::health_reporter();
	health_reporter
		.set_serving::<NodeRegistryServer<RSSFlow>>()
		.await;

	let http_addr = SocketAddr::new(config.address, config.port);
	let addr = SocketAddr::new(config.address, config.grpc_port);

	let pool = SqlitePoolOptions::new()
		.connect_with(
			SqliteConnectOptions::new()
				.filename(&config.database_file)
				.journal_mode(SqliteJournalMode::Wal)
				.create_if_missing(true),
		)
		.await?;
	sqlx::migrate!().run(&pool).await?;

	let svc = RSSFlow(Arc::new(RSSFlowInner {
		pool,
		nodes: Mutex::default(),
	}));

	tracing::info!("gRPC service at: {}", addr);
	tracing::info!("Listening at: {}", http_addr);
	let server = add_reflection_service(
		Server::builder(),
		proto::registry::node_registry_server::SERVICE_NAME,
	)?
	.add_service(health_service)
	.add_service(NodeRegistryServer::new(svc.clone()));

	let http_server = axum::serve(TcpListener::bind(http_addr).await?, app(svc).await?);

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
