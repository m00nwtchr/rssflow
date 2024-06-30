use crate::pipeline::Node;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tokio::time::Instant;

#[derive(Serialize, Deserialize, Debug)]
pub struct Cache<I, B> {
	ttl: Duration,
	#[serde(skip, default = "Instant::now")]
	last_run: Instant,

	child: I,
	#[serde(skip)]
	cached: Mutex<Option<B>>,
}

impl<I, B> Cache<I, B>
where
	I: Node<B> + Serialize + DeserializeOwned + Debug,
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
impl<I, B> Node<B> for Cache<I, B>
where
	I: Node<B> + Serialize + DeserializeOwned + Debug,
	B: Clone + Send + Sync,
{
	// type Item = Channel;

	async fn run(&self) -> anyhow::Result<B> {
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
