use std::fmt::Debug;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{
	sync::Mutex,
	time::{Duration, Instant},
};

use crate::pipeline::node::NodeTrait;

#[derive(Serialize, Deserialize, Debug)]
pub struct Cache<I: NodeTrait> {
	ttl: Duration,
	#[serde(skip, default = "Instant::now")]
	last_run: Instant,

	child: I,
	#[serde(skip)]
	cached: Mutex<Option<I::Item>>,
}

impl<I> Cache<I>
where
	I: NodeTrait,
{
	pub fn new(child: I, ttl: Duration) -> Self {
		Self {
			ttl,
			last_run: Instant::now(),
			child,
			cached: Mutex::new(None),
		}
	}
}

#[async_trait]
impl<I> NodeTrait for Cache<I>
where
	I: NodeTrait,
	I::Item: Clone + Send + Sync,
{
	type Item = I::Item;

	async fn run(&self) -> anyhow::Result<Self::Item> {
		if Instant::now().duration_since(self.last_run) > self.ttl {
			Ok(self
				.cached
				.lock()
				.await
				.insert(self.child.run().await?)
				.clone())
		} else {
			let mut cached = self.cached.lock().await;
			if let Some(cached) = cached.as_ref() {
				Ok(cached.clone())
			} else {
				Ok(cached.insert(self.child.run().await?).clone())
			}
		}
	}
}
