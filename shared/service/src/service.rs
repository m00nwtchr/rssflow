use std::{
	convert::Infallible,
	net::SocketAddr,
	ops::{Deref, DerefMut},
	sync::Arc,
	time::Duration,
};

use axum::middleware::AddExtension;
use futures::future::{self, Either};
use rssflow_proto::node::node_service_server::{NodeService, NodeServiceServer};
#[cfg(feature = "db")]
use sqlx::{PgPool, migrate::MigrateError};
use tokio::net::TcpListener;
use tonic::{
	body::Body,
	codegen::{Service, http::Request},
	server::NamedService,
	service::{Routes, RoutesBuilder},
	transport::Server,
};
use tonic_health::server::health_reporter;
use tower::{layer::util::Identity, util::option_layer};
use tower_http::{add_extension::AddExtensionLayer, trace::TraceLayer};
use tracing::info;

use crate::{ServiceInfo, add_reflection_service, report};

/// A generic microservice builder for gRPC + optional HTTP
pub struct ServiceBuilder {
	info: ServiceInfo,
	routes: RoutesBuilder,
	#[cfg(feature = "http")]
	http: Option<axum::Router>,
	#[cfg(feature = "db")]
	pg_pool: Option<sqlx::PgPool>,
	health_reporter: tonic_health::server::HealthReporter,
	node_service: bool,
}

pub struct ServiceState {
	pub health_reporter: tonic_health::server::HealthReporter,
}

impl ServiceBuilder {
	/// Initialize tracing, load config, setup health + gRPC address
	pub fn new(info: ServiceInfo) -> anyhow::Result<Self> {
		crate::tracing::init(&info);
		crate::config::config(&info);

		let (hr, hs) = health_reporter();

		let mut routes = Routes::builder();
		routes.add_service(hs);

		Ok(Self {
			info,
			health_reporter: hr,
			routes,
			#[cfg(feature = "http")]
			http: None,
			node_service: false,
			#[cfg(feature = "db")]
			pg_pool: None,
		})
	}

	/// Register a tonic gRPC service
	pub fn with_service<S>(mut self, svc: S) -> Self
	where
		S: Service<Request<Body>, Error = Infallible>
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
		if !self.node_service {
			self.node_service = true;
			self.health_reporter
				.set_serving::<NodeServiceServer<S>>()
				.await;
			add_reflection_service(&mut self.routes, NodeServiceServer::<S>::NAME).unwrap();
			self.routes.add_service(NodeServiceServer::new(svc));
		}
		self
	}

	/// add an HTTP endpoint alongside gRPC
	#[cfg(feature = "http")]
	pub fn with_http<R>(mut self, router: R) -> Self
	where
		R: Send + 'static,
		axum::Router: From<R>,
	{
		self.http = Some(axum::Router::from(router));
		self
	}

	/// add an HTTP endpoint alongside gRPC
	#[cfg(feature = "db")]
	pub async fn with_pg<F, Fut>(mut self, init: F) -> anyhow::Result<Self>
	where
		F: FnOnce(PgPool) -> Fut,
		Fut: Future<Output = Result<(), MigrateError>>,
	{
		let config = crate::config::config(&self.info);
		let pg_pool = sqlx::postgres::PgPoolOptions::new()
			.max_connections(5)
			.connect(config.postgres_url.as_str())
			.await?;
		init(pg_pool.clone()).await?;
		self.pg_pool = Some(pg_pool);
		Ok(self)
	}

	/// Build and run gRPC + optional HTTP + report
	pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
		let config = crate::config::config(&self.info);

		// gRPC builder
		let grpc_builder = Server::builder()
			.layer({
				let mut sb = tower::ServiceBuilder::new()
					.layer(TraceLayer::new_for_grpc())
					.layer(AddExtensionLayer::new(self.health_reporter.clone()));

				#[cfg(feature = "db")]
				let sb = sb.layer(option_layer(
					self.pg_pool
						.as_ref()
						.map(|pg| AddExtensionLayer::new(pg.clone())),
				));

				sb
			})
			.add_routes(self.routes.routes());

		let grpc_addr = SocketAddr::new(config.address, config.grpc_port);
		let grpc = grpc_builder.serve(grpc_addr);

		// combine with HTTP if present
		#[cfg(feature = "http")]
		let main = if let Some(router) = self.http {
			let router = router.layer({
				let sb = tower::ServiceBuilder::new()
					.layer(TraceLayer::new_for_http())
					.layer(AddExtensionLayer::new(self.health_reporter));

				#[cfg(feature = "db")]
				let sb = sb.layer(option_layer(
					self.pg_pool
						.as_ref()
						.map(|pg| AddExtensionLayer::new(pg.clone())),
				));

				sb
			});

			let http_addr = SocketAddr::new(config.address, config.http_port);
			let http = axum::serve(TcpListener::bind(http_addr).await?, router).into_future();

			info!("{} HTTP at {}", self.info.name, http_addr);
			Either::Left(async {
				Either::Left(future::select(Box::pin(http), Box::pin(grpc)).await)
			})
		} else {
			Either::Right(async { Either::Right(grpc.await) })
		};
		#[cfg(not(feature = "http"))]
		let main = Either::Right(async { Either::Right(grpc.await) });

		info!("{} gRPC at {}", self.info.name, grpc_addr);
		if self.node_service {
			tokio::spawn(async {
				tokio::time::sleep(Duration::from_secs(5)).await;
				report(self.info.name, config).await
			});
		}

		match main.await {
			Either::Left(Either::Left((r, _))) => r?,
			Either::Left(Either::Right((r, _))) | Either::Right(r) => r?,
		}
		Ok(())
	}
}
