use std::collections::BTreeMap;

use prost_types::Struct;
use rssflow_service::proto::node::Field;
use serde::{Deserialize, Serialize};

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

pub fn to_struct(map: BTreeMap<String, Value>) -> Struct {
	Struct {
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

// TODO: Unused, might make this a thing again in the future
#[derive(Serialize, Deserialize, Default)]
struct FlowBuilder {
	// nodes: Vec<NodeOptions>,
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
