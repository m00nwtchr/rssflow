use std::{slice, sync::Arc};

use super::node::{Data, DataKind, Field, NodeTrait, IO};
use crate::flow::feed_arr;
use anyhow::anyhow;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_regex;

#[derive(Serialize, Deserialize, Debug)]
pub struct Filter {
	field: Field,
	#[allow(clippy::struct_field_names)]
	filter: Kind,
	invert: bool,

	#[serde(skip)]
	input: Arc<IO>,
	#[serde(skip)]
	output: Arc<IO>,
}

impl Filter {
	pub fn new(field: Field, filter: Kind, invert: bool) -> Self {
		Self {
			field,
			filter,
			invert,

			output: Arc::default(),
			input: Arc::default(),
		}
	}
}

#[async_trait]
impl NodeTrait for Filter {
	fn inputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.input)
	}

	fn outputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.input)
	}

	fn input_types(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	fn output_types(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	#[tracing::instrument(name = "filter_node")]
	async fn run(&self) -> anyhow::Result<()> {
		let Some(Data::Feed(mut atom)) = self.input.get() else {
			return Err(anyhow!(""));
		};

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

	fn set_input(&mut self, _index: usize, input: Arc<IO>) {
		self.input = input;
	}
	fn set_output(&mut self, _index: usize, output: Arc<IO>) {
		self.output = output;
	}
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
	Regex(#[serde(with = "serde_regex")] Regex),
	Contains(String),
}
