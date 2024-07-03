use std::cmp::min;

use async_trait::async_trait;
use atom_syndication::{ContentBuilder, Feed};
use futures::stream::{self, StreamExt};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tracing::Instrument;

use super::node::NodeTrait;

#[derive(Serialize, Deserialize, Debug)]
pub struct Retrieve<I> {
	#[serde(with = "serde_selector")]
	content: Selector,

	child: I,
}

impl<I: NodeTrait> Retrieve<I> {
	pub fn new(child: I, content: Selector) -> Self {
		Self { content, child }
	}
}

async fn get_content(
	mut entry: atom_syndication::Entry,
	selector: &Selector,
) -> anyhow::Result<atom_syndication::Entry> {
	let Some(link) = entry.links().iter().find(|l| l.rel().eq("alternate")) else {
		return Ok(entry);
	};

	let content = reqwest::get(link.href()).await?.text().await?;
	let html = Html::parse_document(&content);
	let content: String = html.select(selector).map(|s| s.inner_html()).collect();

	// item.description = None;
	entry.set_content(
		ContentBuilder::default()
			.value(content)
			.content_type("html".to_string())
			.build(),
	);

	Ok(entry)
}

#[async_trait]
impl<I: NodeTrait<Item = Feed>> NodeTrait for Retrieve<I> {
	type Item = Feed;

	async fn run(&self) -> anyhow::Result<Feed> {
		let span = tracing::info_span!("retrieve_node");
		let mut atom = self.child.run().await?;

		let n = min(atom.entries.len(), 6); // Avoiding too high values to prevent spamming the target site.
		let items: Vec<anyhow::Result<atom_syndication::Entry>> = stream::iter(atom.entries.into_iter())
			.map(|item| get_content(item, &self.content))
			.buffered(n)
			.collect()
			.instrument(span.clone())
			.await;
		let _enter = span.enter();
		atom.entries = items.into_iter().collect::<anyhow::Result<_>>()?;

		Ok(atom)
	}
}

pub(crate) mod serde_selector {
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
