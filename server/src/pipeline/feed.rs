use crate::pipeline::Node;
use async_trait::async_trait;
use rss::Channel;
use serde::{Deserialize, Serialize};
use url::Url;

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
impl Node for Feed {
	type Item = Channel;

	async fn run(&self) -> anyhow::Result<Channel> {
		let content = reqwest::get(self.url.clone()).await?.bytes().await?;
		let channel = Channel::read_from(&content[..])?;

		tracing::info!("Get {}", &self.url);

		Ok(channel)
	}
}
