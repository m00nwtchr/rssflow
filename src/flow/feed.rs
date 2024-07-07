use anyhow::anyhow;
use async_trait::async_trait;
use parking_lot::Mutex;
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::{
	sync::Arc,
	time::{Duration, Instant},
};
use url::Url;

use super::node::{Data, DataKind, NodeTrait, IO};
use crate::websub::WebSub;

#[inline]
fn inputs() -> [Arc<IO>; 1] {
	[Arc::new(IO::new(DataKind::WebSub))]
}

#[inline]
fn mutex_now() -> Mutex<Instant> {
	Mutex::new(Instant::now())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Feed {
	url: Url,

	ttl: Duration,
	#[serde(skip, default = "mutex_now")]
	last_fetch: Mutex<Instant>,

	#[serde(skip)]
	web_sub: Mutex<Option<WebSub>>,

	#[serde(skip, default = "inputs")]
	inputs: [Arc<IO>; 1],
	#[serde(skip, default = "super::feed_io")]
	output: Arc<IO>,
}

impl Feed {
	pub fn new(url: Url, ttl: Duration) -> Self {
		Self {
			url,
			ttl,
			last_fetch: mutex_now(),
			web_sub: Mutex::default(),

			inputs: inputs(),
			output: Arc::new(IO::new(DataKind::Feed)),
		}
	}
}

#[async_trait]
impl NodeTrait for Feed {
	fn inputs(&self) -> &[Arc<IO>] {
		&self.inputs
	}

	fn outputs(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	fn is_dirty(&self) -> bool {
		!self.output.is_some()
			|| self.inputs[0].is_dirty()
			|| self.last_fetch.lock().elapsed() > self.ttl
	}

	#[tracing::instrument(name = "feed_node")]
	async fn run(&self) -> anyhow::Result<()> {
		let sub = self.inputs[0].is_dirty();
		let content = if sub {
			let Some(Data::WebSub(websub)) = self.inputs[0].get() else {
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

			response.bytes().await?
		};
		let feed = atom_syndication::Feed::read_from(&content[..])?;

		if !sub {
			let hub = feed.links.iter().find(|l| l.rel.eq("hub"));
			let this = feed.links.iter().find(|l| l.rel.eq("self"));

			if let (Some(hub), Some(this)) = (hub, this) {
				self.web_sub.lock().replace(WebSub {
					hub: hub.href.clone(),
					this: this.href.clone(),
				});
			}
		}

		*self.last_fetch.lock() = Instant::now();
		self.output.accept(feed)
	}

	fn set_output(&mut self, _index: usize, output: Arc<IO>) {
		self.output = output;
	}

	fn web_sub(&self) -> Option<WebSub> {
		self.web_sub.lock().clone()
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

		node.run().await?;

		let c = node.web_sub();

		println!("{c:?}");

		Ok(())
	}
}
