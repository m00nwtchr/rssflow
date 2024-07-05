#![allow(clippy::module_name_repetitions)]
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use derive_more::From;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumDiscriminants};

use super::feed::Feed;
use crate::websub::WebSub;

#[cfg(feature = "filter")]
use super::filter::Filter;
#[cfg(feature = "retrieve")]
use super::retrieve::Retrieve;
#[cfg(feature = "sanitise")]
use super::sanitise::Sanitise;
#[cfg(feature = "wasm")]
use super::wasm::Wasm;

#[async_trait]
pub trait NodeTrait: Sync + Send {
	fn inputs(&self) -> Box<[Arc<IO>]> {
		Box::new([])
	}
	fn outputs(&self) -> Box<[DataKind]> {
		Box::new([])
	}

	fn is_dirty(&self) -> bool {
		let inputs = self.inputs();

		inputs.is_empty() || inputs.iter().any(|i| i.is_dirty())
	}

	async fn run(&self) -> anyhow::Result<()>;

	#[allow(unused_variables)]
	fn set_outputs(&mut self, outputs: Vec<Arc<IO>>) {
		unimplemented!();
	}
	fn output(&mut self, output: Arc<IO>);

	fn web_sub(&self) -> Option<WebSub> {
		None
	}
}

#[async_trait]
impl NodeTrait for Node {
	fn inputs(&self) -> Box<[Arc<IO>]> {
		match self {
			Self::Feed(n) => n.inputs(),
			Self::Filter(n) => n.inputs(),
			#[cfg(feature = "retrieve")]
			Self::Retrieve(n) => n.inputs(),
			#[cfg(feature = "sanitise")]
			Self::Sanitise(n) => n.inputs(),
			#[cfg(feature = "wasm")]
			Self::Wasm(n) => n.inputs(),
			Self::Other(n) => n.inputs(),
			_ => unimplemented!(),
		}
	}

	fn outputs(&self) -> Box<[DataKind]> {
		match self {
			Self::Feed(n) => n.outputs(),
			Self::Filter(n) => n.outputs(),
			#[cfg(feature = "retrieve")]
			Self::Retrieve(n) => n.outputs(),
			#[cfg(feature = "sanitise")]
			Self::Sanitise(n) => n.outputs(),
			#[cfg(feature = "wasm")]
			Self::Wasm(n) => n.outputs(),
			Self::Other(n) => n.outputs(),
			_ => unimplemented!(),
		}
	}

	fn is_dirty(&self) -> bool {
		match self {
			Self::Feed(n) => n.is_dirty(),
			Self::Filter(n) => n.is_dirty(),
			#[cfg(feature = "retrieve")]
			Self::Retrieve(n) => n.is_dirty(),
			#[cfg(feature = "sanitise")]
			Self::Sanitise(n) => n.is_dirty(),
			#[cfg(feature = "wasm")]
			Self::Wasm(n) => n.is_dirty(),
			Self::Other(n) => n.is_dirty(),
			_ => unimplemented!(),
		}
	}

	async fn run(&self) -> anyhow::Result<()> {
		match self {
			Self::Feed(n) => n.run().await,
			Self::Filter(n) => n.run().await,
			#[cfg(feature = "retrieve")]
			Self::Retrieve(n) => n.run().await,
			#[cfg(feature = "sanitise")]
			Self::Sanitise(n) => n.run().await,
			#[cfg(feature = "wasm")]
			Self::Wasm(n) => n.run().await,
			Self::Other(n) => n.run().await,
			_ => unimplemented!(),
		}
	}

	fn set_outputs(&mut self, outputs: Vec<Arc<IO>>) {
		match self {
			Self::Feed(n) => n.set_outputs(outputs),
			Self::Filter(n) => n.set_outputs(outputs),
			#[cfg(feature = "retrieve")]
			Self::Retrieve(n) => n.set_outputs(outputs),
			#[cfg(feature = "sanitise")]
			Self::Sanitise(n) => n.set_outputs(outputs),
			#[cfg(feature = "wasm")]
			Self::Wasm(n) => n.set_outputs(outputs),
			Self::Other(n) => n.set_outputs(outputs),
			_ => unimplemented!(),
		}
	}

	fn output(&mut self, output: Arc<IO>) {
		match self {
			Self::Feed(n) => n.output(output),
			Self::Filter(n) => n.output(output),
			#[cfg(feature = "retrieve")]
			Self::Retrieve(n) => n.output(output),
			#[cfg(feature = "sanitise")]
			Self::Sanitise(n) => n.output(output),
			#[cfg(feature = "wasm")]
			Self::Wasm(n) => n.output(output),
			Self::Other(n) => n.output(output),
			_ => unimplemented!(),
		}
	}
}

#[derive(Serialize, Deserialize, From, Display)]
#[serde(tag = "type")]
pub enum Node {
	Feed(Feed),
	#[cfg(feature = "filter")]
	Filter(Filter),
	#[cfg(feature = "retrieve")]
	Retrieve(Retrieve),
	#[cfg(feature = "sanitise")]
	Sanitise(Sanitise),
	#[cfg(feature = "wasm")]
	#[serde(skip)]
	Wasm(Wasm),
	#[serde(skip)]
	Other(Box<dyn NodeTrait>),
}

#[derive(EnumDiscriminants, Serialize, Deserialize, Debug, From, Clone)]
#[strum_discriminants(name(DataKind), derive(Serialize, Deserialize))]
#[serde(untagged)]
pub enum Data {
	Feed(atom_syndication::Feed),
	Entry(atom_syndication::Entry),
	Vec(Vec<Data>),
	Any(Box<Data>),
}

impl Data {
	pub fn is_kind(&self, kind: &DataKind) -> bool {
		match kind {
			DataKind::Feed => matches!(self, Data::Feed(_)),
			DataKind::Entry => matches!(self, Data::Entry(_)),
			DataKind::Vec => matches!(self, Data::Vec(_)),
			DataKind::Any => true,
		}
	}

	pub fn kind(&self) -> DataKind {
		match self {
			Self::Feed(_) => DataKind::Feed,
			Self::Entry(_) => DataKind::Entry,
			Self::Vec(_) => DataKind::Vec,
			Self::Any(data) => data.kind(),
		}
	}
}

#[derive(Default, Debug)]
pub struct IOInner {
	data: Option<Data>,
	dirty: bool,
}

#[derive(Debug)]
pub struct IO {
	inner: Arc<RwLock<IOInner>>,
	kind: DataKind,
}

impl IO {
	pub fn new(kind: DataKind) -> Self {
		Self {
			inner: Arc::default(),
			kind,
		}
	}

	pub fn kind(&self) -> &DataKind {
		&self.kind
	}

	pub fn accept(&self, data: impl Into<Data>) -> anyhow::Result<()> {
		let data = data.into();
		if data.is_kind(&self.kind) {
			let mut inner = self.inner.write();
			inner.data.replace(data);
			inner.dirty = true;
			Ok(())
		} else {
			Err(anyhow!("Wrong data type"))
		}
	}

	pub fn get(&self) -> Option<Data> {
		self.inner.read().data.clone()
	}

	pub fn is_some(&self) -> bool {
		self.inner.read().data.is_some()
	}

	pub fn is_dirty(&self) -> bool {
		let read = self.inner.read();
		read.dirty || read.data.is_none()
	}

	pub fn clear(&self) {
		self.inner.write().dirty = false;
	}
}

pub fn collect_inputs(inputs: &Vec<Arc<IO>>) -> Option<Vec<Data>> {
	let mut data = Vec::with_capacity(inputs.len());
	for input in inputs {
		if !input.is_some() {
			return None;
		}

		data.push(input.get().unwrap());
	}

	Some(data)
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Field {
	Author,
	Summary,
	Content,
	Title,
	// Uri
}
