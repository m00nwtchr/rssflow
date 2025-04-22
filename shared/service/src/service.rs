use std::{convert::Infallible, net::SocketAddr, time::Duration};

use futures::{
	FutureExt,
	future::{self, Either},
};
use rssflow_proto::node::node_service_server::{NodeService, NodeServiceServer};
use tokio::{net::TcpListener, task::JoinHandle};
use tonic::{
	body::Body,
	codegen::{Service, http::Request},
	server::NamedService,
	service::{Routes, RoutesBuilder},
	transport::Server,
};
use tonic_health::{
	pb::health_server::{Health, HealthServer},
	server::{HealthService, health_reporter},
};
use tracing::info;

use crate::{add_reflection_service, report};

/// A generic microservice builder for gRPC + optional HTTP
pub struct ServiceBuilder {
	name: &'static str,
	health_reporter: tonic_health::server::HealthReporter,
	routes: RoutesBuilder,
	http: Option<axum::Router>,
	flag: bool,
}

impl ServiceBuilder {
	/// Initialize tracing, load config, setup health + gRPC address
	pub async fn new(name: &'static str) -> anyhow::Result<Self> {
		crate::tracing::init();

		let config = crate::config::config(name);

		let (hr, hs) = health_reporter();

		let mut routes = Routes::builder();
		routes.add_service(hs);

		Ok(Self {
			name,
			health_reporter: hr,
			routes,
			http: None,
			flag: false,
		})
	}
}

impl ServiceBuilder {
	/// Register a tonic gRPC service and mark it healthy
	pub fn with_service<S>(mut self, svc: S) -> Self
	where
		S: Service<Request<Body>, Error=Infallible>
		+ NamedService
		+ Clone
		+ Send
		+ Sync
		+ 'static,
		S::Response: axum::response::IntoResponse,
		S::Future: Send + 'static,
	{
		self.routes.add_service(svc);
		self
	}

	/// Register a tonic gRPC service and mark it healthy
	pub async fn with_node_service<S>(mut self, svc: S) -> Self
	where
		S: NodeService,
	{
		if !self.flag {
			self.flag = true;
			self.health_reporter
				.set_serving::<NodeServiceServer<S>>()
				.await;
			add_reflection_service(&mut self.routes, NodeServiceServer::<S>::NAME).unwrap();
			self.routes.add_service(NodeServiceServer::new(svc));
		}
		self
	}

	/// add an HTTP endpoint alongside gRPC
	pub fn with_http<R>(mut self, router: R) -> Self
	where
		R: Send + 'static,
		axum::Router: From<R>,
	{
		self.http = Some(router.into());
		self
	}

	/// Build and run gRPC + optional HTTP + report
	pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
		let config = crate::config::config(self.name);

		// gRPC builder
		let grpc_builder = Server::builder().add_routes(self.routes.routes());

		let grpc_addr = SocketAddr::new(config.address, config.port);
		let grpc = grpc_builder.serve(grpc_addr);

		// combine with HTTP if present
		let main = if let Some(router) = self.http {
			let http_addr = SocketAddr::new(config.address, config.http_port);
			let http = axum::serve(TcpListener::bind(http_addr).await?, router).into_future();

			info!("{} HTTP at {}", self.name, http_addr);
			Either::Left(async {
				Either::Left(future::select(Box::pin(http), Box::pin(grpc)).await)
			})
		} else {
			Either::Right(async { Either::Right(grpc.await) })
		};

		info!("{} gRPC at {}", self.name, grpc_addr);
		if self.flag {
			tokio::spawn(async {
				tokio::time::sleep(Duration::from_secs(5)).await;
				let _ = report(self.name, config).await;
			});
		}

		match main.await {
			Either::Left(Either::Left((r, _))) => r?,
			Either::Left(Either::Right((r, _))) | Either::Right(r) => r?,
		}
		Ok(())
	}
}
