use std::{str::FromStr, time::Duration};

use atom_syndication::Feed;
use redis::AsyncCommands;
use reqwest::{header, header::LINK};
use rssflow_service::{
	ServiceExt2, check_node, interceptor,
	proto::{
		node::{
			NodeMeta, PingRequest, PingResponse, ProcessRequest, ProcessResponse,
			node_service_server::NodeService,
		},
		websub::{
			SubscribeRequest, WebSub, WebSubEvent, web_sub_service_client::WebSubServiceClient,
		},
	},
	try_from_request,
};
use runesys::{Service, cache::Cached, telemetry::propagation::send_trace};
use tonic::{Request, Response, Status, transport::Endpoint};
use tracing::{error, info, instrument};
use url::Url;

use crate::FetchNode;

#[tonic::async_trait]
impl NodeService for FetchNode {
	#[instrument(skip_all)]
	async fn process(
		&self,
		request: Request<ProcessRequest>,
	) -> Result<Response<ProcessResponse>, Status> {
		runesys::telemetry::propagation::accept_trace(&request);
		check_node::<Self>(&request)?;
		let request = request.into_inner();
		let mut conn = self.conn.clone();

		let url = request.get_option_required("url").and_then(|s: &String| {
			Url::from_str(s).map_err(|e| Status::invalid_argument(e.to_string()))
		})?;

		let feed = if let Ok(wse) = try_from_request::<WebSubEvent>(&request) {
			Feed::read_from(&wse.body[..]).map_err(|e| Status::internal(e.to_string()))?
		} else {
			let ttl = Duration::from_secs(match request.get_option::<&f64>("ttl") {
				Some(r) => r.map(|n| *n as u64)?,
				None => 60 * 60, // 1h
			});

			let cached: Option<Cached<Feed>> = conn.get(format!("cache:{url}")).await.ok();
			if let Some(cached) = cached {
				if cached.elapsed() <= ttl {
					info!("Cache hit");
					let feed: rssflow_service::proto::feed::Feed = (&cached.value).into();
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
			let feed =
				Feed::read_from(&content[..]).map_err(|e| Status::internal(e.to_string()))?;

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

				let websub_service = "http://[::]:50052";

				match async {
					let endpoint = Endpoint::from_str(websub_service)?;
					endpoint.connect().await
				}
				.await
				{
					Ok(channel) => {
						let mut client =
							WebSubServiceClient::with_interceptor(channel, interceptor(send_trace));

						let _ = client
							.subscribe(SubscribeRequest {
								sub: Some(websub),
								node: Some(Self::node_meta()),
							})
							.await;
					}
					Err(err) => error!("{err}"),
				}
			}

			feed
		};

		let cached = Cached::new(feed.clone());
		let _: () = conn
			.set_ex(format!("cache:{url}"), cached, 86400)
			.await
			.map_err(|e| Status::internal(e.to_string()))?;

		let feed: rssflow_service::proto::feed::Feed = (&feed).into();
		Ok(Response::new(ProcessResponse {
			payload: Some(feed.into()),
		}))
	}

	async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
		Self::respond_to_ping()
	}
}
