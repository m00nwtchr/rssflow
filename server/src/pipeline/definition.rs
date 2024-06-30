use std::{fmt::Debug, time::Duration};

use rss::Channel;
use scraper::Selector;
use serde::{Deserialize, Serialize};
use url::Url;

use super::retrieve::serde_selector;
use crate::pipeline::{
	cache::Cache,
	feed::Feed,
	filter::{Field, Filter, Kind},
	retrieve::Retrieve,
	NodeTrait,
};

#[derive(Serialize, Deserialize, Debug)]
pub enum Node {
	Cache {
		ttl: Duration,

		child: Box<Node>,
	},
	Feed {
		url: Url,
	},
	Filter {
		field: Field,
		filter: Kind,
		invert: bool,

		child: Box<Node>,
	},
	Retrieve {
		#[serde(with = "serde_selector")]
		content: Selector,
		child: Box<Node>,
	},
}

impl Node {
	fn filter(self, field: Field, filter: Kind, invert: bool) -> Self {
		Self::Filter {
			field,
			filter,
			invert,
			child: Box::new(self),
		}
	}

	fn retrieve(self, content: Selector) -> Self {
		Self::Retrieve {
			content,
			child: Box::new(self),
		}
	}

	fn cache(self, ttl: Duration) -> Self {
		Self::Cache {
			ttl,
			child: Box::new(self),
		}
	}
}

impl From<Node> for Box<dyn NodeTrait<Item = Channel>> {
	fn from(node: Node) -> Self {
		match node {
			Node::Cache { ttl, child } => {
				let int: Box<dyn NodeTrait<Item = Channel>> = (*child).into();
				Box::new(Cache::new(int, ttl))
			}

			Node::Feed { url } => Box::new(Feed::new(url)),
			Node::Filter {
				field,
				filter,
				invert,
				child,
			} => {
				let int: Box<dyn NodeTrait<Item = Channel>> = (*child).into();
				Box::new(Filter::new(int, field, filter, invert))
			}
			Node::Retrieve { content, child } => {
				let int: Box<dyn NodeTrait<Item = Channel>> = (*child).into();
				Box::new(Retrieve::new(int, content))
			}
		}
	}
}

type RSSNode = dyn NodeTrait<Item = Channel>;
type Pipeline = Box<RSSNode>;

#[cfg(test)]
mod test {
	use crate::pipeline::{
		definition::{Node, Pipeline},
		filter::{Field, Kind},
		NodeTrait,
	};
	use ron::ser::PrettyConfig;
	use scraper::Selector;
	use std::time::Duration;
	use tokio::time::sleep;

	#[tokio::test]
	pub async fn ser_de() -> anyhow::Result<()> {
		tracing_subscriber::fmt::init();

		let perfectly_serialisable = Node::Feed {
			url: "https://www.azaleaellis.com/tag/pgts/feed".parse()?,
		}
		.filter(
			Field::Description,
			Kind::Contains("BELOW IS A SNEAK PEEK OF THIS CONTENT!".to_string()),
			true,
		)
		.retrieve(Selector::parse(".entry-content").unwrap())
		.cache(Duration::from_secs(60 * 60));

		let ron = ron::ser::to_string_pretty(&perfectly_serialisable, PrettyConfig::default())?;

		tracing::info!("Serialise: {}", ron);

		let de: Node = ron::de::from_str(&ron)?;

		let pipeline: Pipeline = de.into();

		let channel = &pipeline.run().await?;
		tracing::info!("{}", channel.to_string());

		Ok(())
	}
}
