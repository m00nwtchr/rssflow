use async_trait::async_trait;
use atom_syndication::Feed;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_regex;

use super::node::{Field, NodeTrait};

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
impl<I: NodeTrait<Item = Feed>> NodeTrait for Filter<I> {
	type Item = Feed;

	async fn run(&self) -> anyhow::Result<Feed> {
		let mut atom = self.child.run().await?;

		let _span = tracing::info_span!("filter_node").entered();
		atom.entries.retain(|item| {
			let cmp = match self.field {
				Field::Author => item.authors().first().map(|p| &p.name),
				Field::Summary => item.summary().map(|s| &s.value),
				Field::Content => item.content().and_then(|c| c.value.as_ref()),
				Field::Title => Some(&item.title().value),
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

		Ok(atom)
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Kind {
	Regex(#[serde(with = "serde_regex")] Regex),
	Contains(String),
}
