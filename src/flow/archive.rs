use std::fmt::Debug;

use async_trait::async_trait;
use rss::{Channel, Item};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::{
	sync::Mutex,
	time::{Duration, Instant},
};

use super::node::NodeTrait;

#[derive(Debug, Default)]
struct KVStore;

impl KVStore {
	pub async fn insert(k: String, v: Item) {
		todo!()
	}

	pub async fn get(k: String) -> Item {
		todo!()
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Archive<I: NodeTrait<Item = Channel>> {
	max_items: Option<usize>,
	max_age: Option<Duration>,

	child: I,
	#[serde(skip)]
	store: KVStore,
}

impl<I> Archive<I>
where
	I: NodeTrait<Item = Channel>,
{
	pub fn new(child: I, store: KVStore) -> Self {
		Self {
			max_items: Some(200),
			max_age: Some(Duration::from_secs(3 * 30 * 24 * 60 * 60)), // 3 months

			child,
			store: KVStore,
		}
	}
}

#[async_trait]
impl<I> NodeTrait for Archive<I>
where
	I: NodeTrait<Item = Channel>,
{
	type Item = Channel;

	async fn run(&self) -> anyhow::Result<Self::Item> {
		let mut input = self.child.run().await?;

		for item in &input.items {
		}

		Ok(todo!())
	}
}
