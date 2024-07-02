pub mod cache;
pub mod feed;
pub mod filter;
pub mod node;
pub mod retrieve;
pub mod sanitise;
#[cfg(feature = "wasm")]
pub mod wasm;
mod dummy;

#[cfg(test)]
mod test {
	use super::node::NodeTrait;
	use crate::flow::{
		feed::Feed,
		filter::{Field, Kind},
	};
	use scraper::Selector;
	use std::time::Duration;

	#[tokio::test]
	pub async fn test() -> anyhow::Result<()> {
		tracing_subscriber::fmt::init();

		let pipe = Feed::new("https://www.azaleaellis.com/tag/pgts/feed".parse()?)
			.filter(
				Field::Description,
				Kind::Contains("BELOW IS A SNEAK PEEK OF THIS CONTENT!".parse()?),
				true,
			)
			.retrieve(Selector::parse(".entry-content").unwrap())
			.sanitise(Field::Content)
			.cache(Duration::from_secs(60 * 60));

		tracing::info!("{}", serde_json::to_string_pretty(&pipe)?);
		//
		// let channel = &pipe.run().await?;
		// tracing::info!("{}", channel.to_string());
		//
		// let channel = &pipe.run().await?;
		// tracing::info!("{}", channel.to_string());
		//
		// sleep(Duration::from_secs(11)).await;
		// let channel = &pipe.run().await?;
		// tracing::info!("{}", channel.to_string());

	

		Ok(())
	}
}
