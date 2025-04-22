use std::{str::FromStr, time::Duration};

use atom_syndication::Feed;
use proto::{
	cache::Cached,
	node::{ProcessRequest, ProcessResponse, node_service_server::NodeService},
	registry::Node,
	websub::{SubscribeRequest, WebSub, WebSubEvent, web_sub_service_client::WebSubServiceClient},
};
use redis::AsyncCommands;
use reqwest::{header, header::LINK};
use tonic::{Request, Response, Status};
use tracing::{info, instrument};
use url::Url;

use crate::FetchNode;

#[tonic::async_trait]
impl NodeService for FetchNode {
	#[instrument(skip(self))]
	async fn process(
		&self,
		request: Request<ProcessRequest>,
	) -> Result<Response<ProcessResponse>, Status> {
		if let Some(node) = request.metadata().get("x-node") {
			if node != "Fetch" {
				return Err(Status::not_found(format!(
					"node {} not found",
					node.to_str().unwrap()
				)));
			}
		}

		let request = request.into_inner();
		let mut conn = self.conn.clone();

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
			let Ok(wse) = WebSubEvent::try_from(payload) else {
				todo!();
			};

			Feed::read_from(&wse.body[..]).map_err(|e| Status::internal(e.to_string()))?
		} else {
			let ttl = match request.options.as_ref().and_then(|o| o.fields.get("ttl")) {
				Some(v) => match &v.kind {
					Some(prost_types::value::Kind::NumberValue(n)) => {
						Duration::from_secs(*n as u64)
					}
					_ => Err(Status::invalid_argument("wrong type for ttl"))?,
				},
				None => Duration::from_secs(60 * 60),
			};

			let cached: Option<Cached<Feed>> = conn.get(format!("cache:{url}")).await.ok();
			if let Some(cached) = cached {
				if cached.elapsed() <= ttl {
					info!("Cache hit");
					let feed: proto::feed::Feed = (&cached.value).into();
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

				if let Ok(mut client) = WebSubServiceClient::connect("http://[::]:50052").await {
					let _ = client
						.subscribe(SubscribeRequest {
							sub: Some(websub),
							node: Some(Node {
								address: "http://[::]:50061".to_string(),
								node_name: "Fetch".into(),
							}),
						})
						.await;
				}
			}

			feed
		};

		let cached = Cached::new(feed.clone());
		let _: () = conn
			.set_ex(format!("cache:{url}"), cached, 86400)
			.await
			.map_err(|e| Status::internal(e.to_string()))?;

		let feed: proto::feed::Feed = (&feed).into();
		Ok(Response::new(ProcessResponse {
			payload: Some(feed.into()),
		}))
	}
}
