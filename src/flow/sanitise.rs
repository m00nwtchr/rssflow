use std::{future::Future, slice, sync::Arc, thread::available_parallelism};

use anyhow::anyhow;
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use super::node::{Data, DataKind, Field, NodeTrait, IO};

#[inline]
pub fn default_ammonia() -> ammonia::Builder<'static> {
	let mut ammonia = ammonia::Builder::new();
	ammonia.add_generic_attributes(["style"]);
	ammonia
}

/// Removes unnecessary elements/attributes from entry html.
#[derive(Serialize, Deserialize, Debug)]
pub struct Sanitise {
	field: Field,

	#[serde(skip, default = "default_ammonia")]
	ammonia: ammonia::Builder<'static>,

	#[serde(skip)]
	input: Arc<IO>,
	#[serde(skip)]
	output: Arc<IO>,
}

impl Sanitise {
	pub fn new(field: Field) -> Self {
		Self {
			field,
			ammonia: default_ammonia(),
			input: Arc::default(),
			output: Arc::default(),
		}
	}
}

#[async_trait]
impl NodeTrait for Sanitise {
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

	#[tracing::instrument(name = "sanitise_node", skip(self))]
	async fn run(&self) -> anyhow::Result<()> {
		let Some(Data::Feed(mut atom)) = self.input.get() else {
			return Err(anyhow!(""));
		};

		atom.entries = stream::iter(atom.entries.into_iter())
			.map(|mut item| async {
				let Some(value) = super::get_value(&self.field, &mut item) else {
					return item;
				};

				let value = self.ammonia.clean(value).to_string();

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
