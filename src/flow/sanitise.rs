use std::thread::available_parallelism;

use super::node::NodeTrait;
use crate::flow::node::Field;
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use rss::Channel;
use serde::{Deserialize, Serialize};
use tracing::Instrument;

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
impl<I: NodeTrait<Item = Channel>> NodeTrait for Sanitise<I> {
	type Item = Channel;

	async fn run(&self) -> anyhow::Result<Channel> {
		let mut rss = self.child.run().await?;

		let span = tracing::info_span!("sanitise_node");
		rss.items = stream::iter(rss.items.into_iter())
			.map(|mut item| async {
				let Some(value) = (match self.field {
					Field::Description => &item.description,
					Field::Content => &item.content,
					_ => unimplemented!(),
				}) else {
					return item;
				};

				let value = self.ammonia.clean(value).to_string();

				match self.field {
					Field::Description => item.description = Some(value),
					Field::Content => item.content = Some(value),
					_ => unimplemented!(),
				}

				item
			})
			.buffered(available_parallelism()?.get())
			.collect()
			.instrument(span)
			.await;

		Ok(rss)
	}
}
