use std::cmp::min;

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use rss::Channel;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tracing::Instrument;

use crate::pipeline::Node;

#[derive(Serialize, Deserialize, Debug)]
pub struct Retrieve<I> {
	#[serde(with = "serde_selector")]
	content: Selector,

	child: I,
}

impl<I: Node> Retrieve<I> {
	pub fn new(child: I, content: Selector) -> Self {
		Self { content, child }
	}
}

async fn get_content(mut item: rss::Item, selector: &Selector) -> anyhow::Result<rss::Item> {
	let Some(link) = &item.link else {
		return Ok(item);
	};

	tracing::info!("{link}");
	let content = reqwest::get(link).await?.text().await?;
	let html = Html::parse_document(&content);
	let content: String = html.select(selector).map(|s| s.inner_html()).collect();

	item.description = None;
	item.content = Some(content);

	Ok(item)
}

#[async_trait]
impl<I: Node<Item = Channel>> Node for Retrieve<I> {
	type Item = Channel;

	async fn run(&self) -> anyhow::Result<Channel> {
		let mut rss = self.child.run().await?;
		let n = min(rss.items.len(), 6); // Avoiding too high values to prevent spamming the target site.

		let span = tracing::info_span!("retrieve_node");
		let items: Vec<anyhow::Result<rss::Item>> = stream::iter(rss.items.into_iter())
			.map(|item| get_content(item, &self.content))
			.buffered(n)
			.collect()
			.instrument(span)
			.await;
		rss.items = items.into_iter().collect::<anyhow::Result<_>>()?;

		Ok(rss)
	}
}

mod serde_selector {
	use scraper::{selector::ToCss, Selector};
	use serde::{Deserialize, Deserializer, Serializer};

	pub fn serialize<S>(selector: &Selector, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(&selector.to_css_string())
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Selector, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		Selector::parse(&s).map_err(serde::de::Error::custom)
	}
}
