use std::{
	slice,
	str::FromStr,
	sync::Arc,
	time::{Duration, Instant},
};

use anyhow::anyhow;
use async_trait::async_trait;
use parking_lot::Mutex;
use reqwest::{header, header::LINK};
use serde::{Deserialize, Serialize};
use serde_with::{DurationSeconds, serde_as};
use url::Url;

use super::node::{Data, DataKind, IO, NodeTrait};
use crate::subscriber::websub::WebSub;

fn mutex_now() -> Mutex<Instant> {
	Mutex::new(Instant::now())
}

/// HTTP GET an Atom feed, and subscribe via WebSub if available.
#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
pub struct Feed {
	url: Url,

	#[serde_as(as = "DurationSeconds")]
	ttl: Duration,
	#[serde(skip, default = "mutex_now")]
	last_fetch: Mutex<Instant>,

	#[serde(skip)]
	web_sub: Mutex<Option<WebSub>>,
}

impl Feed {
	pub fn new(url: Url, ttl: Duration) -> Self {
		Self {
			url,
			ttl,
			last_fetch: mutex_now(),
			web_sub: Mutex::default(),
		}
	}
}

#[async_trait]
impl NodeTrait for Feed {
	fn inputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.input)
	}

	fn outputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.output)
	}

	fn input_types(&self) -> &[DataKind] {
		&[DataKind::WebSub]
	}

	fn output_types(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	fn is_dirty(&self) -> bool {
		!self.output.is_some()
			|| self.input.is_dirty()
			|| self.last_fetch.lock().elapsed() > self.ttl
	}

	#[tracing::instrument(name = "feed_node")]
	async fn run(&self) -> anyhow::Result<()> {
		let mut ws = self.input.is_dirty();

		let content = if ws {
			let Some(Data::WebSub(websub)) = self.input.get() else {
				return Err(anyhow!(""));
			};

			websub
		} else {
			let response = reqwest::get(self.url.clone()).await?;

			if let Some(ct) = response.headers().get(header::CONTENT_TYPE) {
				if ct.eq("application/rss+xml") {
					// TODO: Handle RSS channels (upgrade to atom)
				}
			}

			if let Some(websub) = response
				.headers()
				.get(LINK)
				.and_then(|v| v.to_str().ok())
				.and_then(|v| WebSub::from_str(v).ok())
			{
				self.web_sub.lock().replace(websub);
				ws = true;
			}

			response.bytes().await?
		};
		let feed = atom_syndication::Feed::read_from(&content[..])?;

		if !ws {
			let hub = feed.links.iter().find(|l| l.rel.eq("hub"));
			let this = feed.links.iter().find(|l| l.rel.eq("self"));

			if let (Some(hub), Some(this)) = (hub, this) {
				self.web_sub.lock().replace(WebSub {
					hub: hub.href.clone(),
					topic: this.href.clone(),
				});
			}
		}

		*self.last_fetch.lock() = Instant::now();
		self.output.accept(feed)
	}

	fn web_sub(&self) -> Option<WebSub> {
		self.web_sub.lock().clone()
	}
}

#[cfg(test)]
mod test {
	use std::time::Duration;

	use crate::flow::{feed::Feed, node::NodeTrait};

	#[tokio::test]
	pub async fn websub() -> anyhow::Result<()> {
		let node = Feed::new(
			"http://push-tester.cweiske.de/feed.php".parse().unwrap(),
			Duration::from_secs(60 * 60),
		);

		node.run().await?;

		let c = node.web_sub();

		println!("{c:?}");

		Ok(())
	}
}
