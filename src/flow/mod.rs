use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

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

use crate::websub::WebSub;
use node::{Data, DataKind, Node, NodeTrait, IO};

#[inline]
fn feed_io() -> Arc<IO> {
	Arc::new(IO::new(DataKind::Feed))
}

#[inline]
fn feed_arr<const N: usize>() -> [Arc<IO>; N] {
	std::array::from_fn(|_| feed_io())
}

pub struct Flow {
	nodes: Mutex<Vec<Node>>,

	web_sub: parking_lot::Mutex<Option<WebSub>>,
	inputs: Box<[Arc<IO>]>,
	outputs: Box<[Arc<IO>]>,
}

impl Flow {
	pub fn result(&self) -> Option<Data> {
		self.outputs.first()?.get()
	}
}

#[async_trait]
impl NodeTrait for Flow {
	fn inputs(&self) -> &[Arc<IO>] {
		&self.inputs
	}

	fn outputs(&self) -> &[Arc<IO>] {
		&self.outputs
	}

	fn input_types(&self) -> &[DataKind] {
		&[]
	}

	fn output_types(&self) -> &[DataKind] {
		&[]
	}

	async fn run(&self) -> anyhow::Result<()> {
		let nodes = self.nodes.lock().await;
		for node in nodes.iter() {
			if node.is_dirty() {
				tracing::info!("Running node: {node}");
				node.run().await?;

				let inputs = node.inputs();
				for io in inputs.iter().filter(|i| i.is_dirty()) {
					io.clear();
				}
			}
		}

		if let Some(web_sub) = nodes.first().and_then(NodeTrait::web_sub) {
			self.web_sub.lock().replace(web_sub);
		}

		Ok(())
	}

	fn set_input(&mut self, _index: usize, _input: Arc<IO>) {
		unimplemented!()
	}
	fn set_output(&mut self, _index: usize, _output: Arc<IO>) {
		unimplemented!()
	}

	fn web_sub(&self) -> Option<WebSub> {
		self.web_sub.lock().clone()
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

	pub fn simple(mut self) -> Flow {
		let inputs: Box<[_]> = self
			.nodes
			.first()
			.iter()
			.flat_map(|n| n.input_types())
			.map(|d| Arc::new(IO::new(*d)))
			.collect();

		let mut outputs: Vec<Arc<IO>> = Vec::new();

		//
		// let outputs: Box<[Arc<IO>]> = self
		// 	.nodes
		// 	.last()
		// 	.iter()
		// 	.flat_map(|n| n.output_types())
		// 	.map(|d| Arc::new(IO::new(*d)))
		// 	.collect();

		if !self.nodes.is_empty() {
			for (i, node) in self.nodes.iter_mut().enumerate() {
				if i == 0 {
					for (j, arc) in inputs.iter().enumerate() {
						node.set_input(j, arc.clone());
					}
				} else {
					for (j, arc) in outputs.iter().enumerate() {
						node.set_input(j, arc.clone());
					}
				}

				let o: Vec<_> = node
					.output_types()
					.iter()
					.map(|d| Arc::new(IO::new(*d)))
					.collect();

				for (j, arc) in o.iter().enumerate() {
					node.set_output(j, arc.clone());
				}
				outputs = o;
			}
		}

		Flow {
			nodes: Mutex::new(self.nodes),
			inputs,
			outputs: outputs.into(),
			web_sub: parking_lot::Mutex::default(),
		}
	}
}

impl From<Vec<Node>> for FlowBuilder {
	fn from(nodes: Vec<Node>) -> Self {
		FlowBuilder { nodes }
	}
}

#[cfg(test)]
mod test {
	use super::node::Field;
	use crate::flow::{
		feed::Feed,
		filter::{Filter, Kind},
		retrieve::Retrieve,
		sanitise::Sanitise,
		FlowBuilder,
	};
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

		// let flow = builder.simple(DataKind::Feed);
		// let Some(Data::Feed(atom)) = flow.run().await? else {
		// 	return Err(anyhow!(""));
		// };
		//
		// println!("{}", atom.to_string());
		//
		// let Some(Data::Feed(atom)) = flow.run().await? else {
		// 	return Err(anyhow!(""));
		// };
		//
		// println!("Wow");

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
