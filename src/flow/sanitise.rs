use std::{sync::Arc, thread::available_parallelism};

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

#[derive(Serialize, Deserialize, Debug)]
pub struct Sanitise {
	field: Field,

	#[serde(skip, default = "default_ammonia")]
	ammonia: ammonia::Builder<'static>,

	#[serde(skip, default = "super::feed_io")]
	input: Arc<IO>,
	#[serde(skip, default = "super::feed_io")]
	output: Arc<IO>,
}

impl Sanitise {
	pub fn new(field: Field) -> Self {
		Self {
			field,
			ammonia: default_ammonia(),
			input: Arc::new(IO::new(DataKind::Feed)),
			output: Arc::new(IO::new(DataKind::Feed)),
		}
	}
}

#[async_trait]
impl NodeTrait for Sanitise {
	fn inputs(&self) -> Box<[Arc<IO>]> {
		Box::new([self.input.clone()])
	}

	fn outputs(&self) -> Box<[DataKind]> {
		Box::new([])
	}

	#[tracing::instrument(name = "sanitise_node", skip(self))]
	async fn run(&self) -> anyhow::Result<()> {
		let Some(Data::Feed(mut atom)) = self.input.get() else {
			return Err(anyhow!(""));
		};

		atom.entries = stream::iter(atom.entries.into_iter())
			.map(|mut item| async {
				let Some(value) = (match self.field {
					Field::Summary => item.summary().map(|s| &s.value),
					Field::Content => item.content().and_then(|c| c.value.as_ref()),
					_ => unimplemented!(),
				}) else {
					return item;
				};

				let value = self.ammonia.clean(value).to_string();

				match self.field {
					Field::Summary => {
						let mut summary = item.summary.unwrap();
						summary.value = value;
						item.summary = Some(summary);
					}
					Field::Content => {
						let mut content = item.content.unwrap();
						content.value = Some(value);
						item.content = Some(content);
					}
					_ => unimplemented!(),
				}

				item
			})
			.buffered(available_parallelism()?.get())
			.collect()
			.await;

		self.output.accept(atom)
	}

	fn output(&mut self, output: Arc<IO>) {
		self.output = output;
	}
}
