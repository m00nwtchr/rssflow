#![warn(clippy::pedantic)]

use std::{error::Error, str::FromStr, time::Duration};

use anyhow::Context;
use proto::registry::{Node, RegisterRequest, node_registry_client::NodeRegistryClient};
pub use rssflow_proto as proto;
use rssflow_proto::node::{
	ProcessRequest, ProcessResponse, node_service_client::NodeServiceClient,
};
use runesys::{
	telemetry,
	util::{retry_async, try_from_any},
};
use tonic::{
	Request, Response, Status,
	codegen::InterceptedService,
	server::NamedService,
	service::Interceptor,
	transport::{Channel, Endpoint},
};
use tonic_health::pb::health_client::HealthClient;
use url::Url;

use crate::config::ServiceConfig;

pub mod config;

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
			interceptor(telemetry::propagation::send_trace),
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

pub trait ServiceExt {
	fn with_reporter(self) -> Self;
}

impl<S> ServiceExt for runesys::service::ServiceBuilder<S>
where
	S: runesys::Service,
	S::Server: NamedService,
{
	fn with_reporter(self) -> Self {
		let config = config::config::<S>();

		self.with_task(async {
			tokio::time::sleep(Duration::from_secs(5)).await;
			report(S::INFO.name, config).await?;

			Ok(())
		})
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
			let mut client = NodeRegistryClient::with_interceptor(
				channel,
				interceptor(telemetry::propagation::send_trace),
			);

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

pub fn check_node<S: runesys::Service>(request: &Request<ProcessRequest>) -> Result<(), Status> {
	if let Some(node) = request.metadata().get("x-node") {
		if node != S::INFO.name {
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
) -> Result<T, tonic::Status> {
	let Some(payload) = &request.payload else {
		return Err(Status::invalid_argument("payload missing"));
	};

	try_from_any(payload)
}

pub fn interceptor<T>(mutator: impl Fn(&mut T)) -> impl FnMut(T) -> Result<T, Status> {
	move |mut value: T| {
		mutator(&mut value);
		Ok(value)
	}
}
