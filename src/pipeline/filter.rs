use crate::pipeline::node::NodeTrait;
use async_trait::async_trait;
use regex::Regex;
use rss::Channel;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Filter<I> {
	field: Field,
	kind: Kind,
	invert: bool,

	child: I,
}

impl<I: NodeTrait> Filter<I> {
	pub fn new(child: I, field: Field, filter: Kind, invert: bool) -> Self {
		Self {
			field,
			kind: filter,
			invert,
			child,
		}
	}
}

#[async_trait]
impl<I: NodeTrait<Item = Channel>> NodeTrait for Filter<I> {
	type Item = Channel;

	async fn run(&self) -> anyhow::Result<Channel> {
		let mut rss = self.child.run().await?;

		rss.items.retain(|item| {
			let cmp = match self.field {
				Field::Author => &item.author,
				Field::Description => &item.description,
				Field::Content => &item.content,
				Field::Title => &item.title,
			};
			let cmp = if let Some(cmp) = cmp { cmp } else { "" };

			let value = match &self.kind {
				Kind::Regex(regex) => regex.is_match(cmp),
				Kind::Contains(str) => cmp.contains(str),
				// FilterSpec::ContainsCaseInsensitive(str) => {
				//     cmp.to_lowercase().contains(&str.to_lowercase())
				// }
			};

			if self.invert {
				!value
			} else {
				value
			}
		});

		Ok(rss)
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Field {
	Author,
	Description,
	Content,
	Title,
	// Uri
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Kind {
	Regex(#[serde(with = "serde_regex")] Regex),
	Contains(String),
}

use serde_regex;
