pub mod feed;
#[cfg(feature = "filter")]
pub mod filter;
pub mod node;
#[cfg(feature = "retrieve")]
pub mod retrieve;
#[cfg(feature = "sanitise")]
pub mod sanitise;
#[cfg(feature = "wasm")]
pub mod wasm;

use crate::flow::node::{Data, DataKind, NodeTrait, IO};
use node::Node;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[inline]
fn feed_io() -> Arc<IO> {
	Arc::new(IO::new(DataKind::Feed))
}

pub struct Flow {
	nodes: Mutex<Vec<Node>>,

	output: Arc<IO>,
}

impl Flow {
	pub async fn run(&self) -> anyhow::Result<Option<Data>> {
		let nodes = self.nodes.lock().await;
		for node in nodes.iter() {
			node.run().await?
		}
		Ok(self.output.get())
	}
}

#[derive(Serialize, Deserialize)]
pub struct FlowBuilder {
	nodes: Vec<Node>,
}

impl FlowBuilder {
	pub fn new() -> Self {
		Self { nodes: Vec::new() }
	}

	pub fn node(mut self, node: impl Into<Node>) -> Self {
		self.nodes.push(node.into());
		self
	}

	pub fn simple(self, output: DataKind) -> Flow {
		let mut nodes = self.nodes;
		let output = Arc::new(IO::new(output));

		let mut io = Some(output.clone());
		for node in nodes.iter_mut().rev() {
			if let Some(ioi) = io {
				node.output(ioi);
				io = None;
			}

			if let Some(input) = node.inputs().get(0) {
				io.replace(input.clone());
			}
		}

		Flow { nodes: Mutex::new(nodes), output }
	}
}

impl From<Vec<Node>> for FlowBuilder {
	fn from(nodes: Vec<Node>) -> Self {
		FlowBuilder { nodes }
	}
}

#[cfg(test)]
mod test {
	use super::node::{Data, DataKind, Field};
	use crate::flow::{
		feed::Feed,
		filter::{Filter, Kind},
		retrieve::Retrieve,
		sanitise::Sanitise,
		FlowBuilder,
	};
	use anyhow::anyhow;
	use scraper::Selector;
	use std::time::Duration;

	#[tokio::test]
	pub async fn test() -> anyhow::Result<()> {
		let builder = FlowBuilder::new()
			.node(Feed::new(
				"https://www.azaleaellis.com/tag/pgts/feed/atom".parse()?,
				Duration::from_secs(60 * 60),
			))
			.node(Filter::new(
				Field::Summary,
				Kind::Contains("BELOW IS A SNEAK PEEK OF THIS CONTENT!".parse()?),
				true,
			))
			.node(Retrieve::new(Selector::parse(".entry-content").unwrap()))
			.node(Sanitise::new(Field::Content));

		println!("{}", serde_json::to_string_pretty(&builder)?);

		let flow = builder.simple(DataKind::Feed);
		let Some(Data::Feed(atom)) = flow.run().await? else {
			return Err(anyhow!(""));
		};

		println!("{}", atom.to_string());

		let Some(Data::Feed(atom)) = flow.run().await? else {
			return Err(anyhow!(""));
		};

		println!("Wow");

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
