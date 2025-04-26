#![warn(clippy::pedantic)]

use std::{error::Error, fmt::Debug, str::FromStr, time::Duration};

use ::tracing::error;
use anyhow::Context;
use proto::{
	FILE_DESCRIPTOR_SET,
	registry::{Node, RegisterRequest, node_registry_client::NodeRegistryClient},
};
pub use rssflow_proto as proto;
use rssflow_proto::node::{
	ProcessRequest, ProcessResponse, node_service_client::NodeServiceClient,
};
use tokio::time::sleep;
use tonic::{
	Request, Response, Status,
	codegen::InterceptedService,
	service::{Interceptor, RoutesBuilder},
	transport::{Channel, Endpoint},
};
use tonic_health::pb::health_client::HealthClient;
use url::Url;
use uuid::{Uuid, uuid};

use crate::config::ServiceConfig;

#[cfg(feature = "cache")]
pub mod cache;
pub mod config;
mod error;
pub mod service;
#[cfg(feature = "telemetry")]
pub mod telemetry;

const NAMESPACE: Uuid = uuid!("466b8727-8f7f-4596-b59d-92b2252b2c4b");

pub struct ServiceInfo {
	pub name: &'static str,
	pub pkg: &'static str,
	pub version: &'static str,
}

impl ServiceInfo {
	pub fn uuid(&self) -> Uuid {
		Uuid::new_v5(&NAMESPACE, self.pkg.as_bytes())
	}
}

#[macro_export]
macro_rules! service_info {
	($name:literal) => {
		pub const SERVICE_INFO: ::rssflow_service::ServiceInfo = ::rssflow_service::ServiceInfo {
			name: $name,
			pkg: env!("CARGO_PKG_NAME"),
			version: env!("CARGO_PKG_VERSION"),
		};
	};
}

pub trait NodeExt {
	fn endpoint(&self) -> anyhow::Result<Endpoint>;
	async fn channel(&self) -> anyhow::Result<Channel>;

	#[cfg(not(feature = "telemetry"))]
	async fn client(&self) -> anyhow::Result<NodeServiceClient<Channel>>;
	#[cfg(feature = "telemetry")]
	async fn client(
		&self,
	) -> anyhow::Result<NodeServiceClient<InterceptedService<Channel, impl Interceptor>>>;

	#[allow(async_fn_in_trait)]
	async fn health(&self) -> anyhow::Result<HealthClient<Channel>>;

	async fn process(&self, req: ProcessRequest) -> anyhow::Result<Response<ProcessResponse>>;
}

impl NodeExt for Node {
	fn endpoint(&self) -> anyhow::Result<Endpoint> {
		Endpoint::from_str(&self.address)
			.with_context(|| format!("create endpoint {}", self.address))
	}

	async fn channel(&self) -> anyhow::Result<Channel> {
		self.endpoint()?
			.connect()
			.await
			.with_context(|| format!("connect to endpoint {}", self.address))
	}

	#[cfg(not(feature = "telemetry"))]
	async fn client(&self) -> anyhow::Result<NodeServiceClient<Channel>> {
		Ok(NodeServiceClient::new(self.channel().await?))
	}

	#[cfg(feature = "telemetry")]
	async fn client(
		&self,
	) -> anyhow::Result<NodeServiceClient<InterceptedService<Channel, impl Interceptor>>> {
		Ok(NodeServiceClient::with_interceptor(
			self.channel().await?,
			interceptor(telemetry::send_trace),
		))
	}

	async fn health(&self) -> anyhow::Result<HealthClient<Channel>> {
		Ok(HealthClient::new(self.channel().await?))
	}

	async fn process(&self, req: ProcessRequest) -> anyhow::Result<Response<ProcessResponse>> {
		let mut req = Request::new(req);
		req.metadata_mut().insert("x-node", self.node_name.parse()?);
		Ok(self.client().await?.process(req).await?)
	}
}

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

pub mod tracing {
	#[cfg(feature = "telemetry")]
	use opentelemetry::trace::TracerProvider;
	use tracing::level_filters::LevelFilter;
	use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

	use crate::ServiceInfo;

	#[allow(private_interfaces)]
	pub fn init(info: &ServiceInfo) {
		if tracing::dispatcher::has_been_set() {
			return;
		}

		let subscriber = tracing_subscriber::registry()
			.with(
				tracing_subscriber::EnvFilter::builder()
					.with_default_directive(LevelFilter::INFO.into())
					.from_env_lossy(),
			)
			.with(tracing_subscriber::fmt::layer());

		#[cfg(feature = "telemetry")]
		let subscriber = subscriber
			.with(tracing_opentelemetry::OpenTelemetryLayer::new(
				crate::telemetry::init_tracer_provider(&info).tracer(info.pkg),
			))
			.with(tracing_opentelemetry::MetricsLayer::new(
				crate::telemetry::init_meter_provider(&info),
			));

		subscriber.init();
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
				error!("Operation failed: {err:?}. Retries left: {retries}",);
				sleep(delay).await;
			}
			Err(err) => return Err(err),
		}
	}
}

pub async fn report(
	name: &str,
	config: &ServiceConfig,
) -> Result<(), Box<dyn Error + Send + Sync>> {
	retry_async(
		|| async {
			let endpoint = Endpoint::from_str(config.registry_url.as_str())
				.with_context(|| format!("create endpoint {}", config.registry_url))?;
			let channel = endpoint
				.connect()
				.await
				.with_context(|| format!("connect to endpoint {}", config.registry_url))?;

			#[cfg(not(feature = "telemetry"))]
			let mut client = NodeRegistryClient::new(channel);
			#[cfg(feature = "telemetry")]
			let mut client =
				NodeRegistryClient::with_interceptor(channel, interceptor(telemetry::send_trace));

			client
				.register(RegisterRequest {
					node: Some(Node {
						address: config
							.service_url
							.as_ref()
							.map(Url::to_string)
							.unwrap_or_default(),
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

pub fn check_node(request: &Request<ProcessRequest>, info: &ServiceInfo) -> Result<(), Status> {
	if let Some(node) = request.metadata().get("x-node") {
		if node != info.name {
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

pub fn interceptor<T>(mutator: impl Fn(&mut T)) -> impl FnMut(T) -> Result<T, Status> {
	move |mut value: T| {
		mutator(&mut value);
		Ok(value)
	}
}
