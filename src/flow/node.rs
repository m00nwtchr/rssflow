#![allow(clippy::module_name_repetitions)]
use std::time::Duration;

use super::{
	cache::Cache,
	feed::Feed,
	filter::{Field, Filter, Kind},
	retrieve::{serde_selector, Retrieve},
};
use crate::flow::sanitise::Sanitise;
use async_trait::async_trait;
use rss::Channel;
use scraper::Selector;
use serde::{Deserialize, Serialize};
use url::Url;

pub type NodeObject<T> = Box<dyn NodeTrait<Item = T>>;
pub type RSSNode = NodeObject<Channel>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
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
	Sanitise {
		field: Field,

		child: Box<Node>,
	},
}

#[async_trait]
pub trait NodeTrait: Sync + Send {
	type Item;

	async fn run(&self) -> anyhow::Result<Self::Item>;

	fn cache(self, ttl: Duration) -> Cache<Self>
	where
		Self: Sized,
	{
		Cache::new(self, ttl)
	}

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

	fn sanitise(self, field: Field) -> Sanitise<Self>
	where
		Self: Sized,
	{
		Sanitise::new(self, field)
	}
}

impl Node {
	pub fn cache(self, ttl: Duration) -> Self {
		Self::Cache {
			child: Box::new(self),
			ttl,
		}
	}

	pub fn filter(self, field: Field, filter: Kind, invert: bool) -> Self {
		Self::Filter {
			child: Box::new(self),
			field,
			filter,
			invert,
		}
	}

	pub fn retrieve(self, content: Selector) -> Self {
		Self::Retrieve {
			child: Box::new(self),
			content,
		}
	}

	pub fn sanitise(self, field: Field) -> Self {
		Self::Sanitise {
			child: Box::new(self),
			field,
		}
	}
}

impl From<Node> for RSSNode {
	fn from(node: Node) -> Self {
		match node {
			Node::Cache { ttl, child } => {
				let int: RSSNode = (*child).into();
				Box::new(Cache::new(int, ttl))
			}
			Node::Feed { url } => Box::new(Feed::new(url)),
			Node::Filter {
				field,
				filter,
				invert,
				child,
			} => {
				let int: RSSNode = (*child).into();
				Box::new(Filter::new(int, field, filter, invert))
			}
			Node::Retrieve { content, child } => {
				let int: RSSNode = (*child).into();
				Box::new(Retrieve::new(int, content))
			}
			Node::Sanitise { child, field } => {
				let int: RSSNode = (*child).into();
				Box::new(Sanitise::new(int, field))
			}
			_ => unimplemented!(),
		}
	}
}

#[async_trait]
impl<T> NodeTrait for NodeObject<T> {
	type Item = T;

	async fn run(&self) -> anyhow::Result<T> {
		(**self).run().await
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[tokio::test]
	pub async fn serde() -> anyhow::Result<()> {
		tracing_subscriber::fmt::init();

		let node = Node::Feed {
			url: "https://www.azaleaellis.com/tag/pgts/feed".parse()?,
		}
		.filter(
			Field::Description,
			Kind::Contains("BELOW IS A SNEAK PEEK OF THIS CONTENT!".to_string()),
			true,
		)
		.retrieve(Selector::parse(".entry-content").unwrap())
		.sanitise(Field::Content)
		.cache(Duration::from_secs(60 * 60));

		tracing::info!("{}", serde_json::to_string_pretty(&node)?);

		Ok(())
	}
}
