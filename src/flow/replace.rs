use std::{slice, sync::Arc, thread::available_parallelism};

use anyhow::anyhow;
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use super::node::{Data, DataKind, Field, NodeTrait, IO};

/// String replace
#[derive(Serialize, Deserialize, Debug)]
pub struct Replace {
	field: Field,

	old: String,
	new: String,

	#[serde(skip)]
	input: Arc<IO>,
	#[serde(skip)]
	output: Arc<IO>,
}

impl Replace {
	pub fn new(field: Field, old: String, new: String) -> Self {
		Self {
			field,
			old,
			new,
			input: Arc::default(),
			output: Arc::default(),
		}
	}
}

#[async_trait]
impl NodeTrait for Replace {
	fn inputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.input)
	}

	fn outputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.output)
	}

	fn input_types(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	fn output_types(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	#[tracing::instrument(name = "replace_node", skip(self))]
	async fn run(&self) -> anyhow::Result<()> {
		let Some(Data::Feed(mut atom)) = self.input.get() else {
			return Err(anyhow!(""));
		};

		atom.entries = stream::iter(atom.entries.into_iter())
			.map(|mut item| async {
				let Some(value) = super::get_value(&self.field, &mut item) else {
					return item;
				};

				let value = value.replace(&self.old, &self.new);

				super::set_value(&self.field, &mut item, value);

				item
			})
			.buffered(available_parallelism()?.get())
			.collect()
			.await;

		self.output.accept(atom)
	}

	fn set_input(&mut self, _index: usize, input: Arc<IO>) {
		self.input = input;
	}
	fn set_output(&mut self, _index: usize, output: Arc<IO>) {
		self.output = output;
	}
}
