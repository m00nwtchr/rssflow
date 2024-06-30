use crate::pipeline::Node;
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use rss::Channel;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::cmp::min;
use tracing::Instrument;
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub struct Retrieve<I> {
	#[serde(with = "serde_selector")]
	content: Selector,

	child: I,
}

impl<I: Node<Channel>> Retrieve<I> {
	pub fn new(child: I, content: Selector) -> Self {
		Self { content, child }
	}
}

#[async_trait]
impl<I: Node<Channel>> Node<Channel> for Retrieve<I> {
	// type Item = Channel;

	async fn run(&self) -> anyhow::Result<Channel> {
		let mut rss = self.child.run().await?;
		let n = min(rss.items.len(), 5);

		let span = tracing::info_span!("retrieve_node");
		rss.items = stream::iter(rss.items.into_iter())
			.map(|mut item| async {
				if let Some(link) = &item.link {
					tracing::info!("{link}");
					let content = reqwest::get(link.parse::<Url>().unwrap())
						.await
						.unwrap()
						.text()
						.await
						.unwrap();
					let html = Html::parse_document(&content);

					let content: String =
						html.select(&self.content).map(|s| s.inner_html()).collect();

					item.description = None;
					item.content = Some(content);
				}

				item
			})
			.buffered(n)
			.collect()
			.instrument(span)
			.await;

		Ok(rss)
	}
}

mod serde_selector {
	use scraper::selector::ToCss;
	use scraper::Selector;
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
