use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_regex;

use super::node::{Data, DataKind, Field, NodeTrait, IO};

#[derive(Serialize, Deserialize, Debug)]
pub struct Filter {
	field: Field,
	filter: Kind,
	invert: bool,

	#[serde(skip, default = "super::feed_io")]
	input: Arc<IO>,
	#[serde(skip, default = "super::feed_io")]
	output: Arc<IO>,
}

impl Filter {
	pub fn new(field: Field, filter: Kind, invert: bool) -> Self {
		Self {
			field,
			filter,
			invert,

			input: Arc::new(IO::new(DataKind::Feed)),
			output: Arc::new(IO::new(DataKind::Feed)),
		}
	}
}

#[async_trait]
impl NodeTrait for Filter {
	fn inputs(&self) -> Box<[Arc<IO>]> {
		Box::new([self.input.clone()])
	}

	fn outputs(&self) -> Box<[DataKind]> {
		Box::new([DataKind::Feed])
	}

	async fn run(&self) -> anyhow::Result<()> {
		let Some(Data::Feed(mut atom)) = self.input.get() else {
			return Err(anyhow!(""));
		};

		let _span = tracing::info_span!("filter_node").entered();
		atom.entries.retain(|item| {
			let cmp = match self.field {
				Field::Author => item.authors().first().map(|p| &p.name),
				Field::Summary => item.summary().map(|s| &s.value),
				Field::Content => item.content().and_then(|c| c.value.as_ref()),
				Field::Title => Some(&item.title().value),
			};
			let cmp = if let Some(cmp) = cmp { cmp } else { "" };

			let value = match &self.filter {
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

		self.output.accept(atom)
	}

	fn output(&mut self, output: Arc<IO>) {
		self.output = output;
	}
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
	Regex(#[serde(with = "serde_regex")] Regex),
	Contains(String),
}
