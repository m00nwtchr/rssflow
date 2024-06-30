use crate::pipeline::cache::Cache;
use crate::pipeline::filter::{Filter, Field, Kind};
use crate::pipeline::retrieve::Retrieve;
use async_trait::async_trait;
use rss::Channel;
use scraper::Selector;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::time::Duration;

pub mod cache;
pub mod feed;
pub mod filter;
pub mod retrieve;
// mod wasm;

// #[derive(Default)]
// pub struct Pipeline<I> {
// 	out: Box<dyn Node<I> + Sync + Send>,
// }

// impl<I> Pipeline<I>
// where
// 	I: Sync + Send + Default,
// {
// 	// pub fn new() -> Self {
// 	// 	Pipeline::default()
// 	// }
//
// 	pub fn add_node(&mut self, node: Box<dyn Node<I> + Sync + Send>) {
// 		self.nodes.push(node);
// 	}
//
// 	pub async fn pipeline(&self) -> anyhow::Result<Box<[I]>> {
// 		Node::run(self, Box::new([])).await
// 	}
// }

// #[async_trait]
// impl<I> Node<I> for Pipeline<I>
// where
// 	I: Sync + Send,
// {
// 	async fn run(&self, input: Box<[I]>) -> anyhow::Result<Box<[I]>> {
// 		let mut input: Box<[I]> = input;
// 		for node in &self.nodes {
// 			input = node.run(input).await?;
// 		}
// 		Ok(input)
// 	}
// }

#[async_trait]
pub trait Node<T> {
	// type Item = T;

	async fn run(&self) -> anyhow::Result<T>;

	fn filter(self, field: Field, filter: Kind, invert: bool) -> Filter<Self>
	where
		Self: Sized + Node<Channel>,
	{
		Filter::new(self, field, filter, invert)
	}

	fn retreive(self, content: Selector) -> Retrieve<Self>
	where
		Self: Sized + Node<Channel> + Serialize + DeserializeOwned + Debug,
	{
		Retrieve::new(self, content)
	}

	fn cache<C>(self, ttl: Duration) -> Cache<Self, C>
	where
		Self: Sized + Node<C> + Serialize + DeserializeOwned + Debug,
	{
		Cache::new(self, ttl)
	}
}

#[cfg(test)]
mod test {
	use crate::pipeline::feed::Feed;
	use crate::pipeline::filter::{Field, Kind};
	use crate::pipeline::Node;
	use ron::ser::PrettyConfig;
	use std::time::Duration;
	use tokio::time::sleep;

	#[tokio::test]
	pub async fn test() -> anyhow::Result<()> {
		tracing_subscriber::fmt::init();

		let pipe = Feed::new("https://www.azaleaellis.com/tag/pgts/feed".parse()?)
			.filter(
				Field::Description,
				Kind::Contains("BELOW IS A SNEAK PEEK OF THIS CONTENT!".parse()?),
				true,
			)
			.cache(Duration::from_secs(60 * 60));

		tracing::debug!(
			"{}",
			ron::ser::to_string_pretty(&pipe, PrettyConfig::default())?
		);

		let channel = &pipe.run().await?;
		tracing::info!("{}", channel.to_string());

		let channel = &pipe.run().await?;
		tracing::info!("{}", channel.to_string());

		sleep(Duration::from_secs(11)).await;
		let channel = &pipe.run().await?;
		tracing::info!("{}", channel.to_string());

		Ok(())
	}
}
