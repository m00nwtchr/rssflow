use async_trait::async_trait;
use reqwest::header;
use serde::{Deserialize, Serialize};
use url::Url;

use super::node::NodeTrait;
use crate::websub::WebSub;

#[derive(Serialize, Deserialize, Debug)]
pub struct Feed {
	url: Url,
}

impl Feed {
	pub fn new(url: Url) -> Self {
		Self { url }
	}
}

#[async_trait]
impl NodeTrait for Feed {
	type Item = atom_syndication::Feed;

	#[tracing::instrument(name = "feed_node")]
	async fn run(&self) -> anyhow::Result<atom_syndication::Feed> {
		let response = reqwest::get(self.url.clone()).await?;

		if let Some(ct) = response.headers().get(header::CONTENT_TYPE) {
			if ct.eq("application/rss+xml") {
				// TODO: Handle RSS channels (upgrade to atom)
			}
		}

		let content = response.bytes().await?;
		let feed = atom_syndication::Feed::read_from(&content[..])?;

		Ok(feed)
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

	#[tokio::test]
	pub async fn websub() -> anyhow::Result<()> {
		let node = Feed::new("http://push-tester.cweiske.de/feed.php".parse().unwrap());

		let c = node.websub().await?;

		println!("{:?}", c);

		Ok(())
	}
}
