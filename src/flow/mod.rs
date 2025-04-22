use std::collections::BTreeMap;

use prost_types::Struct;
use proto::node::Field;
use serde::{Deserialize, Serialize};

pub mod node;

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Value {
	Bool(bool),
	Number(u32),
	Field(Field),
	String(String),
}

impl From<Value> for prost_types::Value {
	fn from(value: Value) -> Self {
		match value {
			Value::Bool(b) => prost_types::Value::from(b),
			Value::Number(n) => prost_types::Value::from(n),
			Value::Field(f) => prost_types::Value::from(f as i32),
			Value::String(s) => prost_types::Value::from(s),
		}
	}
}

pub fn to_struct(map: BTreeMap<String, Value>) -> prost_types::Struct {
	prost_types::Struct {
		fields: map.into_iter().map(|(k, v)| (k, v.into())).collect(),
	}
}

#[derive(Serialize, Deserialize)]
pub struct NodeOptions {
	#[serde(rename = "type")]
	pub r#type: String,
	#[serde(flatten)]
	pub options: BTreeMap<String, Value>,
}

impl NodeOptions {
	pub fn options(&self) -> Option<Struct> {
		if self.options.is_empty() {
			None
		} else {
			Some(to_struct(self.options.clone()))
		}
	}
}

#[derive(Serialize, Deserialize)]
pub struct Flow {
	pub nodes: Vec<NodeOptions>,
}

//
// pub struct Flow {
// 	nodes: Mutex<Vec<Node>>,
// }
//
// impl Flow {
// #[async_trait]
// impl NodeTrait for Flow {
// 	async fn run(&self) -> anyhow::Result<()> {
// 		// TODO: Run nodes in order based on input/output dependencies, run adjacent nodes concurrently.
//
// 		let mut subscriptions: Option<Vec<WebSub>> = if self.subscriptions.lock().is_empty() {
// 			Some(Vec::new())
// 		} else {
// 			None
// 		};
//
// 		let nodes = self.nodes.lock().await;
// 		for node in nodes.iter() {
// 			if node.is_dirty() {
// 				tracing::info!("Running node: {node}");
// 				node.run().await?;
//
// 				let inputs = node.inputs();
// 				for io in inputs.iter().filter(|i| i.is_dirty()) {
// 					io.clear();
// 				}
//
// 				if let Some(subscriptions) = &mut subscriptions {
// 					if let Some(sub) = node.web_sub() {
// 						subscriptions.push(sub);
// 					}
// 				}
// 			}
// 		}
//
// 		if let Some(subscriptions) = subscriptions {
// 			*self.subscriptions.lock() = subscriptions;
// 		}
//
// 		Ok(())
// 	}
//

// }

#[derive(Serialize, Deserialize, Default)]
pub struct FlowBuilder {
	// nodes: Vec<Node>,
	// #[serde(default, skip_serializing_if = "Vec::is_empty")]
	// connections: Vec<Connection>,
}
//
// impl FlowBuilder {
// 	pub fn node(mut self, node: impl Into<Node>) -> Self {
// 		self.nodes.push(node.into());
// 		self
// 	}
//
// 	pub fn simple(mut self) -> Self {
// 		self.connections.clear();
// 		for i in 0..self.nodes.len() {
// 			if i > 0 {
// 				self.connections
// 					.push(Connection(Port(i - 1, 0), Port(i, 0)));
// 			}
// 		}
//
// 		self
// 	}
//
// 	pub fn build(mut self) -> Flow {
// 		// TODO: Improve this code once multiple-input nodes actually exist.
// 		// TODO: Collect all unconnected inputs/outputs into flow inputs/outputs
// 		let inputs: Box<[Arc<IO>]> = self
// 			.nodes
// 			.first()
// 			.iter()
// 			.flat_map(|n| n.input_types())
// 			.map(|d| Arc::new(IO::new(*d)))
// 			.collect();
//
// 		let outputs: Box<[Arc<IO>]> = self
// 			.nodes
// 			.last()
// 			.iter()
// 			.flat_map(|n| n.output_types())
// 			.map(|d| Arc::new(IO::new(*d)))
// 			.collect();
//
// 		if !self.nodes.is_empty() {
// 			if let Some(first) = self.nodes.first_mut() {
// 				for (i, input) in inputs.iter().enumerate() {
// 					first.set_input(i, input.clone());
// 				}
// 			}
// 			if let Some(last) = self.nodes.last_mut() {
// 				for (i, output) in outputs.iter().enumerate() {
// 					last.set_output(i, output.clone());
// 				}
// 			}
//
// 			if self.connections.is_empty() {
// 				self = self.simple();
// 			}
// 			let mut port_map = HashMap::new();
// 			for Connection(from, to) in self.connections {
// 				if let Some(from_n) = self.nodes.get_mut(from.0) {
// 					if let Some(kind) = from_n.output_types().get(from.1).copied() {
// 						// NOTE: If multiple outputs are connected to the same input, this will only correctly assign the last one.
// 						let io = port_map.entry(from).or_insert_with(|| {
// 							let io = Arc::new(IO::new(kind));
// 							from_n.set_output(from.1, io.clone());
// 							io
// 						});
//
// 						if let Some(to_n) = self.nodes.get_mut(to.0) {
// 							to_n.connect(io.clone(), to.1);
// 						}
// 					}
// 				}
// 			}
// 		}
//
// 		Flow {
// 			nodes: Mutex::new(self.nodes),
// 			inputs,
// 			outputs,
// 			subscriptions: parking_lot::Mutex::default(),
// 		}
// 	}
// }

// fn get_value<'a>(field: &Field, item: &'a Entry) -> Option<&'a String> {
// 	match field {
// 		Field::Author => item.authors.first().map(|p| &p.name),
// 		Field::Summary => item.summary.as_ref().map(|t| &t.value),
// 		Field::Content => item.content.as_ref().and_then(|c| c.value.as_ref()),
// 		Field::Title => Some(&item.title.value),
// 	}
// }
//
// fn set_value(field: &Field, item: &mut Entry, value: String) {
// 	match field {
// 		Field::Summary => {
// 			item.summary.as_mut().unwrap().value = value;
// 		}
// 		Field::Content => {
// 			item.content.as_mut().unwrap().value = Some(value);
// 		}
// 		_ => unimplemented!(),
// 	}
// }
