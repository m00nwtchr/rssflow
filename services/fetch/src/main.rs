use std::{
	collections::HashMap, fmt::Debug, net::SocketAddr, str::FromStr, sync::Mutex, time::Duration,
};

#[warn(clippy::pedantic)]
use atom_syndication::Feed;
use proto::{
	add_reflection_service,
	node::{
		ProcessRequest, ProcessResponse,
		node_service_server::{NodeService, NodeServiceServer},
	},
	registry::{Node, RegisterRequest, node_registry_client::NodeRegistryClient},
};
use reqwest::{header, header::LINK};
use tokio::time::Instant;
use tonic::{Request, Response, Status, transport::Server};
use tracing::{info, instrument};
use url::Url;

struct Cached {
	time: Instant,
	content: Feed,
}

#[derive(Default)]
struct FetchNode {
	// TODO: Impl redis cache
	cache: Mutex<HashMap<Url, Cached>>,
}

#[tonic::async_trait]
impl NodeService for FetchNode {
	#[instrument(skip(self))]
	async fn process(
		&self,
		request: Request<ProcessRequest>,
	) -> Result<Response<ProcessResponse>, Status> {
		let request = request.into_inner();

		let url = match request.options.as_ref().and_then(|o| o.fields.get("url")) {
			Some(v) => match &v.kind {
				Some(prost_types::value::Kind::StringValue(s)) => Url::from_str(s)
					.map_err(|s| Status::invalid_argument("url is not a valid url"))?,
				_ => Err(Status::invalid_argument("wrong type for url"))?,
			},
			None => Err(Status::invalid_argument("url option missing"))?,
		};

		let payload = request.payload;

		let feed = if let Some(payload) = payload {
			let Ok(wse) = WebSubEvent::try_from(payload) else { todo!(); };

			Feed::read_from(&wse.body[..]).map_err(|e| Status::internal(e.to_string()))?
		} else {
			let ttl = match request.options.as_ref().and_then(|o| o.fields.get("ttl")) {
				Some(v) => match &v.kind {
					Some(prost_types::value::Kind::NumberValue(n)) => Duration::from_secs(*n as u64),
					_ => Err(Status::invalid_argument("wrong type for ttl"))?,
				},
				None => Duration::from_secs(60 * 60),
			};

			if let Some(cached) = self.cache.lock().unwrap().get(&url) {
				if cached.time.elapsed() <= ttl {
					let feed: proto::feed::Feed = (&cached.content).into();
					return Ok(Response::new(ProcessResponse {
						payload: Some(feed.into()),
					}));
				}
			}

			let response = reqwest::get(url.clone()).await.map_err(|e| {
				Status::unavailable(format!("Request to {} failed: {e}", e.url().unwrap()))
			})?;

			if let Some(ct) = response.headers().get(header::CONTENT_TYPE) {
				if ct.eq("application/rss+xml") {
					// TODO: Handle RSS channels (upgrade to atom)
				}
			}

			let websub = response
				.headers()
				.get(LINK)
				.and_then(|v| v.to_str().ok())
				.and_then(|v| WebSub::from_str(v).ok());

			let content = response
				.bytes()
				.await
				.map_err(|e| Status::internal(e.to_string()))?;
			let feed = Feed::read_from(&content[..]).map_err(|e| Status::internal(e.to_string()))?;

			let websub = websub.or_else(|| {
				let hub = feed.links.iter().find(|l| l.rel.eq("hub"));
				let this = feed.links.iter().find(|l| l.rel.eq("self"));

				if let (Some(hub), Some(this)) = (hub, this) {
					Some(WebSub {
						hub: hub.href.clone(),
						topic: this.href.clone(),
					})
				} else {
					None
				}
			});

			if let Some(websub) = websub {
				info!("{websub:?}");

				if let Ok(mut client) = WebSubServiceClient::connect("http://[::]:50052").await {
					let _ = client.subscribe(SubscribeRequest {
						sub: Some(websub),
						node: Some(Node {
							address: "http://[::]:50061".to_string(),
							node_name: "Fetch".into(),
						}),
					}).await;
				}
			}

			feed
		};

		info!("{}", feed.to_string());
		self.cache.lock().unwrap().insert(
			url,
			Cached {
				time: Instant::now(),
				content: feed.clone(),
			},
		);

		let feed: proto::feed::Feed = (&feed).into();
		Ok(Response::new(ProcessResponse {
			payload: Some(feed.into()),
		}))
	}
}

use std::future::Future;

use proto::websub::{SubscribeRequest, WebSub, WebSubEvent};
use tokio::time::sleep;
use proto::websub::web_sub_service_client::WebSubServiceClient;

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
	Fut: Future<Output=Result<T, E>>,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt::init();
	let (health_reporter, health_service) = tonic_health::server::health_reporter();
	health_reporter
		.set_serving::<NodeServiceServer<FetchNode>>()
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
		.unwrap_or(format!("http://fetch:{port}"));

	let ip = "::".parse().unwrap();
	let addr = SocketAddr::new(ip, port);

	let node = FetchNode::default();

	info!("Fetch service at: {}", addr);

	let server = add_reflection_service(
		Server::builder(),
		proto::node::node_service_server::SERVICE_NAME,
	)?
		.add_service(health_service)
		.add_service(NodeServiceServer::new(node));

	let report = retry_async(
		|| async {
			let mut client = NodeRegistryClient::connect(registry_url.clone()).await?;

			client
				.register(RegisterRequest {
					node: Some(Node {
						address: service_url.clone(),
						node_name: "Fetch".into(),
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

#[cfg(test)]
mod tests {
	use proto::{
		feed::Feed,
		node::{ProcessRequest, node_service_server::NodeService},
	};
	use tonic::Request;

	use crate::FetchNode;

	#[tokio::test]
	async fn test() {
		let node = FetchNode::default();

		let mut options = prost_types::Struct::default();
		options.fields.insert(
			"url".to_string(),
			prost_types::Value::from("http://push-tester.cweiske.de/feed.php"),
		);

		let resp = node
			.process(Request::new(ProcessRequest {
				payload: None,
				options: Some(options),
			}))
			.await
			.unwrap();

		let resp = resp.into_inner();

		let feed = Feed::try_from(resp.payload.unwrap()).unwrap();
	}
}
