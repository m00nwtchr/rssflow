use std::{
	sync::Arc,
	time::{Duration, Instant},
};

use async_trait::async_trait;
use reqwest::header;
use serde::{Deserialize, Serialize};
use url::Url;

use super::node::{DataKind, NodeTrait, IO};
use crate::websub::WebSub;

#[derive(Serialize, Deserialize, Debug)]
pub struct Feed {
	url: Url,

	ttl: Duration,
	#[serde(skip, default = "Instant::now")]
	last_fetch: Instant,

	#[serde(skip, default = "super::feed_io")]
	output: Arc<IO>,
}

impl Feed {
	pub fn new(url: Url, ttl: Duration) -> Self {
		Self {
			url,
			output: Arc::new(IO::new(DataKind::Feed)),
			ttl,
			last_fetch: Instant::now(),
		}
	}
}

#[async_trait]
impl NodeTrait for Feed {
	fn outputs(&self) -> Box<[DataKind]> {
		Box::new([DataKind::Feed])
	}

	fn is_dirty(&self) -> bool {
		self.last_fetch.elapsed() > self.ttl || !self.output.is_some()
	}

	#[tracing::instrument(name = "feed_node")]
	async fn run(&self) -> anyhow::Result<()> {
		let response = reqwest::get(self.url.clone()).await?;

		if let Some(ct) = response.headers().get(header::CONTENT_TYPE) {
			if ct.eq("application/rss+xml") {
				// TODO: Handle RSS channels (upgrade to atom)
			}
		}

		let content = response.bytes().await?;
		let feed = atom_syndication::Feed::read_from(&content[..])?;

		self.output.accept(feed)
	}

	fn output(&mut self, output: Arc<IO>) {
		self.output = output;
	}

	async fn websub(&self) -> anyhow::Result<Option<WebSub>> {
		let response = reqwest::get(self.url.clone()).await?;

		if let Some(ct) = response.headers().get(header::CONTENT_TYPE) {
			if ct.eq("application/rss+xml") {
				// TODO: Handle RSS channels (upgrade to atom)
			}
		}

		let content = response.bytes().await?;
		let feed = atom_syndication::Feed::read_from(&content[..])?;

		let this = feed.links.iter().find(|l| l.rel.eq("self"));
		let hub = feed.links.iter().find(|l| l.rel.eq("hub"));

		if let (Some(this), Some(hub)) = (this, hub) {
			let this = this.href.parse()?;
			let hub = hub.href.parse()?;

			Ok(Some(WebSub { hub, this }))
		} else {
			Ok(None)
		}
	}
}

#[cfg(test)]
mod test {
	use crate::flow::{feed::Feed, node::NodeTrait};
	use std::time::Duration;

	#[tokio::test]
	pub async fn websub() -> anyhow::Result<()> {
		let node = Feed::new(
			"http://push-tester.cweiske.de/feed.php".parse().unwrap(),
			Duration::from_secs(60 * 60),
		);

		let c = node.websub().await?;

		println!("{:?}", c);

		Ok(())
	}
}
