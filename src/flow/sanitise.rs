use std::thread::available_parallelism;

use async_trait::async_trait;
use atom_syndication::Feed;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tracing::Instrument;

use super::node::{Field, NodeTrait};

#[derive(Serialize, Deserialize, Debug)]
pub struct Sanitise<I> {
	field: Field,
	child: I,

	#[serde(skip)]
	ammonia: ammonia::Builder<'static>,
}

impl<I: NodeTrait> Sanitise<I> {
	pub fn new(child: I, field: Field) -> Self {
		let mut ammonia = ammonia::Builder::new();
		ammonia.add_generic_attributes(["style"]);

		Self {
			field,
			child,

			ammonia,
		}
	}
}

#[async_trait]
impl<I: NodeTrait<Item = Feed>> NodeTrait for Sanitise<I> {
	type Item = Feed;

	async fn run(&self) -> anyhow::Result<Feed> {
		let mut atom = self.child.run().await?;

		let span = tracing::info_span!("sanitise_node");
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
			.instrument(span)
			.await;

		Ok(atom)
	}
}
