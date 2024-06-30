use std::{fmt::Debug, time::Duration};

use async_trait::async_trait;
use scraper::Selector;
use serde::{de::DeserializeOwned, Serialize};

use crate::pipeline::{
	cache::Cache,
	filter::{Field, Filter, Kind},
	retrieve::Retrieve,
};

pub mod cache;
pub mod feed;
pub mod filter;
pub mod retrieve;

mod definition;
#[cfg(feature = "wasm")]
pub mod wasm;

#[async_trait]
pub trait NodeTrait: Sync + Send {
	type Item;

	async fn run(&self) -> anyhow::Result<Self::Item>;

	fn filter(self, field: Field, filter: Kind, invert: bool) -> Filter<Self>
	where
		Self: Sized,
	{
		Filter::new(self, field, filter, invert)
	}

	fn retrieve(self, content: Selector) -> Retrieve<Self>
	where
		Self: Sized,
	{
		Retrieve::new(self, content)
	}

	fn cache(self, ttl: Duration) -> Cache<Self>
	where
		Self: Sized,
	{
		Cache::new(self, ttl)
	}
}

#[async_trait]
impl<T> NodeTrait for Box<dyn NodeTrait<Item = T> + '_> {
	type Item = T;

	async fn run(&self) -> anyhow::Result<T> {
		(**self).run().await
	}
}

#[cfg(test)]
mod test {
	use crate::pipeline::{
		feed::Feed,
		filter::{Field, Kind},
		NodeTrait,
	};
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
