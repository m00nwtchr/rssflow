use std::{cmp::min, sync::Arc};

use super::node::{Data, DataKind, NodeTrait, IO};
use anyhow::anyhow;
use async_trait::async_trait;
use atom_syndication::ContentBuilder;
use futures::stream::{self, StreamExt};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Retrieve {
	#[serde(with = "serde_selector")]
	content: Selector,

	#[serde(skip, default = "super::feed_io")]
	input: Arc<IO>,
	#[serde(skip, default = "super::feed_io")]
	output: Arc<IO>,
}

impl Retrieve {
	pub fn new(content: Selector) -> Self {
		Self {
			content,
			input: Arc::new(IO::new(DataKind::Feed)),
			output: Arc::new(IO::new(DataKind::Feed)),
		}
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
impl NodeTrait for Retrieve {
	fn inputs(&self) -> Box<[Arc<IO>]> {
		Box::new([self.input.clone()])
	}

	fn outputs(&self) -> Box<[DataKind]> {
		Box::new([DataKind::Feed])
	}

	#[tracing::instrument(name = "retrieve_node")]
	async fn run(&self) -> anyhow::Result<()> {
		let Some(Data::Feed(mut atom)) = self.input.get() else {
			return Err(anyhow!(""));
		};

		let n = min(atom.entries.len(), 6); // Avoiding too high values to prevent spamming the target site.
		let items: Vec<anyhow::Result<atom_syndication::Entry>> =
			stream::iter(atom.entries.into_iter())
				.map(|item| get_content(item, &self.content))
				.buffered(n)
				.collect()
				.await;
		atom.entries = items.into_iter().collect::<anyhow::Result<_>>()?;

		self.output.accept(atom)
	}

	fn output(&mut self, output: Arc<IO>) {
		self.output = output;
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
