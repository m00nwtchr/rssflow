use std::cmp::min;

use base64::{Engine, engine::general_purpose};
use futures::{StreamExt, stream};
use redis::{AsyncCommands, aio::MultiplexedConnection};
use rssflow_service::{
	check_node,
	proto::{
		feed::{Content, Entry, Feed},
		node::{ProcessRequest, ProcessResponse, node_service_server::NodeService},
	},
	try_from_request,
};
use scraper::{Html, Selector, selector::ToCss};
use sha2::{Digest, Sha256};
use tonic::{Request, Response, Status};
use tracing::instrument;

use crate::{RetrieveNode, SERVICE_INFO};

fn make_cache_key(url: &str, selector: &str) -> String {
	let mut hasher = Sha256::new();
	hasher.update(url);
	hasher.update(selector);
	let hash = hasher.finalize();
	format!(
		"rssflow:retrieve:snippet:{}",
		general_purpose::URL_SAFE_NO_PAD.encode(hash)
	)
}

async fn get_content(
	mut entry: Entry,
	selector: &Selector,
	mut conn: MultiplexedConnection,
) -> anyhow::Result<Entry> {
	let Some(link) = entry.links.iter().find(|l| l.rel.eq("alternate")) else {
		return Ok(entry);
	};
	let key = make_cache_key(&link.href, &selector.to_css_string());

	let content: String = if let Some(cached) = conn.get(&key).await? {
		cached
	} else {
		tracing::info!("HTTP GET {}", link.href);
		let content: String = {
			let content = reqwest::get(&link.href).await?.text().await?;
			let html = Html::parse_document(&content);
			html.select(selector).map(|s| s.inner_html()).collect()
		};
		let _: () = conn.set_ex(key, &content, 86400).await?;

		content
	};

	entry.content = Some(Content {
		value: content,
		content_type: "html".to_string(),
		..Content::default()
	});

	Ok(entry)
}

#[tonic::async_trait]
impl NodeService for RetrieveNode {
	#[instrument(skip_all)]
	async fn process(
		&self,
		request: Request<ProcessRequest>,
	) -> Result<Response<ProcessResponse>, Status> {
		rssflow_service::telemetry::accept_trace(&request);
		check_node(&request, &SERVICE_INFO)?;
		let request = request.into_inner();

		let mut feed: Feed = try_from_request(&request)?;

		let selector = request
			.get_option_required("selector")
			.and_then(|s: &String| {
				Selector::parse(s).map_err(|e| Status::invalid_argument(e.to_string()))
			})?;

		let n = min(feed.entries.len(), 6); // Avoiding too high values to prevent spamming the target site.
		let items: Vec<anyhow::Result<Entry>> = stream::iter(feed.entries.into_iter())
			.map(|item| get_content(item, &selector, self.conn.clone()))
			.buffered(n)
			.collect()
			.await;
		feed.entries = items
			.into_iter()
			.collect::<anyhow::Result<_>>()
			.map_err(|e| Status::internal(e.to_string()))?;

		Ok(Response::new(ProcessResponse {
			payload: Some(feed.into()),
		}))
	}
}
