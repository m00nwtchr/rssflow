use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	fmt::{Display, Formatter},
	sync::Arc,
};
use tokio::sync::Mutex;

pub mod feed;
#[cfg(feature = "filter")]
pub mod filter;
#[cfg(feature = "html")]
pub mod html;
pub mod node;
#[cfg(feature = "retrieve")]
pub mod retrieve;
#[cfg(feature = "sanitise")]
pub mod sanitise;
pub mod seen;
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
		// TODO: Run nodes in order based on input/output dependencies, run adjacent nodes concurrently.

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

#[derive(Serialize, Deserialize, Eq, PartialEq, Hash, Clone, Copy)]
pub struct Port(usize, usize);

impl Display for Port {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("N{}P{}", self.0, self.1))
	}
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
pub struct Connection(Port, Port);

#[derive(Serialize, Deserialize, Default)]
pub struct FlowBuilder {
	nodes: Vec<Node>,
	#[serde(default)]
	connections: Vec<Connection>,
}

impl FlowBuilder {
	pub fn node(mut self, node: impl Into<Node>) -> Self {
		self.nodes.push(node.into());
		self
	}

	pub fn simple(mut self) -> Self {
		self.connections.clear();
		for i in 0..self.nodes.len() {
			if i > 0 {
				self.connections
					.push(Connection(Port(i - 1, 0), Port(i, 0)));
			}
		}

		self
	}

	pub fn build(mut self) -> Flow {
		// TODO: Improve this code once multiple-input nodes actually exist.
		// TODO: Collect all unconnected inputs/outputs into flow inputs/outputs
		let inputs: Box<[Arc<IO>]> = self
			.nodes
			.first()
			.iter()
			.flat_map(|n| n.input_types())
			.map(|d| Arc::new(IO::new(*d)))
			.collect();

		let outputs: Box<[Arc<IO>]> = self
			.nodes
			.last()
			.iter()
			.flat_map(|n| n.output_types())
			.map(|d| Arc::new(IO::new(*d)))
			.collect();

		if !self.nodes.is_empty() {
			if let Some(first) = self.nodes.first_mut() {
				for (i, input) in inputs.iter().enumerate() {
					first.set_input(i, input.clone());
				}
			}
			if let Some(last) = self.nodes.last_mut() {
				for (i, output) in outputs.iter().enumerate() {
					last.set_output(i, output.clone());
				}
			}

			if self.connections.is_empty() {
				self = self.simple();
			}
			let mut port_map = HashMap::new();
			for Connection(from, to) in self.connections {
				if let Some(from_n) = self.nodes.get_mut(from.0) {
					if let Some(kind) = from_n.output_types().get(from.1).copied() {
						// NOTE: If multiple outputs are connected to the same input, this will only correctly assign the last one.
						let io = port_map.entry(from).or_insert_with(|| {
							let io = Arc::new(IO::new(kind));
							from_n.set_output(from.1, io.clone());
							io
						});

						if let Some(to_n) = self.nodes.get_mut(to.0) {
							to_n.connect(io.clone(), to.1);
						}
					}
				}
			}
		}

		Flow {
			nodes: Mutex::new(self.nodes),
			inputs,
			outputs,
			web_sub: parking_lot::Mutex::default(),
		}
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
		let builder = FlowBuilder::default()
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
			.node(Sanitise::new(Field::Content))
			.simple();

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
