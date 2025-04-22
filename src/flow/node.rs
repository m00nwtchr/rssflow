#![allow(clippy::module_name_repetitions)]

// #[async_trait]
// #[enum_dispatch]
// pub trait NodeTrait: Sync + Send {
// 	// fn inputs(&self) -> &[Arc<IO>];
// 	// fn outputs(&self) -> &[Arc<IO>];
//
// 	// fn input_types(&self) -> &[DataKind];
// 	// fn output_types(&self) -> &[DataKind];
//
// 	// fn is_dirty(&self) -> bool {
// 	// 	let inputs = self.inputs();
// 	//
// 	// 	inputs.is_empty() || inputs.iter().any(|i| i.is_dirty())
// 	// }
//
// 	async fn run(&self) -> anyhow::Result<()>;
//
// 	// fn set_input(&mut self, index: usize, input: Arc<IO>);
// 	// fn set_output(&mut self, index: usize, output: Arc<IO>);
// 	//
// 	// fn connect(&mut self, io: Arc<IO>, port: usize) {
// 	// 	if let Some(kind) = self.input_types().get(port) {
// 	// 		if io.kind.eq(kind) || DataKind::Any.eq(kind) {
// 	// 			self.set_input(port, io);
// 	// 		}
// 	// 	}
// 	// }
//
// 	fn web_sub(&self) -> Option<WebSub> {
// 		None
// 	}
// }

// #[derive(Serialize, Deserialize, Display)]
// #[serde(tag = "type")]
// #[enum_dispatch(NodeTrait)]
// pub enum Node {
// 	AI(AI),
// 	Feed(super::feed::Feed),
// 	#[cfg(feature = "filter")]
// 	Filter(super::filter::Filter),
// 	#[cfg(feature = "html")]
// 	Html(super::html::Html),
// 	Replace(super::replace::Replace),
// 	#[cfg(feature = "retrieve")]
// 	Retrieve(super::retrieve::Retrieve),
// 	#[cfg(feature = "sanitise")]
// 	Sanitise(super::sanitise::Sanitise),
// 	Seen(Seen),
// 	#[cfg(feature = "wasm")]
// 	#[serde(skip)]
// 	Wasm(super::wasm::Wasm),
// 	#[serde(skip)]
// 	Other(Box<dyn NodeTrait>),
// }

// #[async_trait]
// impl NodeTrait for Box<dyn NodeTrait> {
// 	fn inputs(&self) -> &[Arc<IO>] {
// 		(**self).inputs()
// 	}
//
// 	fn outputs(&self) -> &[Arc<IO>] {
// 		(**self).outputs()
// 	}
//
// 	fn input_types(&self) -> &[DataKind] {
// 		(**self).input_types()
// 	}
//
// 	fn output_types(&self) -> &[DataKind] {
// 		(**self).output_types()
// 	}
//
// 	fn is_dirty(&self) -> bool {
// 		(**self).is_dirty()
// 	}
//
// 	async fn run(&self) -> anyhow::Result<()> {
// 		(**self).run().await
// 	}
//
// 	fn set_input(&mut self, index: usize, input: Arc<IO>) {
// 		(**self).set_input(index, input);
// 	}
//
// 	fn set_output(&mut self, index: usize, output: Arc<IO>) {
// 		(**self).set_output(index, output);
// 	}
//
// 	fn connect(&mut self, io: Arc<IO>, port: usize) {
// 		(**self).connect(io, port);
// 	}
//
// 	fn web_sub(&self) -> Option<WebSub> {
// 		(**self).web_sub()
// 	}
// }

// #[derive(EnumDiscriminants, Serialize, Deserialize, Debug, From, Clone, PartialEq)]
// #[strum_discriminants(name(DataKind), derive(Serialize, Deserialize))]
// #[serde(untagged)]
// pub enum Data {
// 	Feed(atom_syndication::Feed),
// 	Entry(atom_syndication::Entry),
// 	WebSub(Bytes),
// 	Vec(Vec<Data>),
// 	Any(Box<Data>),
// }

// impl Data {
// 	pub fn is_kind(&self, kind: DataKind) -> bool {
// 		match kind {
// 			DataKind::Feed => matches!(self, Data::Feed(_)),
// 			DataKind::Entry => matches!(self, Data::Entry(_)),
// 			DataKind::WebSub => matches!(self, Data::WebSub(_)),
// 			DataKind::Vec => matches!(self, Data::Vec(_)),
// 			DataKind::Any => true,
// 		}
// 	}
//
// 	pub fn kind(&self) -> DataKind {
// 		match self {
// 			Self::Feed(_) => DataKind::Feed,
// 			Self::Entry(_) => DataKind::Entry,
// 			Self::WebSub(_) => DataKind::WebSub,
// 			Self::Vec(_) => DataKind::Vec,
// 			Self::Any(data) => data.kind(),
// 		}
// 	}
// }

// #[derive(Default, Debug)]
// pub struct IOInner {
// 	data: Option<Data>,
// 	dirty: bool,
// }

// #[derive(Debug)]
// pub struct IO {
// 	inner: Arc<RwLock<IOInner>>,
// 	kind: DataKind,
// }
//
// impl IO {
// 	pub fn new(kind: DataKind) -> Self {
// 		Self {
// 			inner: Arc::default(),
// 			kind,
// 		}
// 	}
//
// 	pub fn kind(&self) -> &DataKind {
// 		&self.kind
// 	}
//
// 	pub fn accept(&self, data: impl Into<Data>) -> anyhow::Result<()> {
// 		let data = data.into();
// 		if data.is_kind(self.kind) {
// 			let mut inner = self.inner.write();
// 			inner.dirty = inner.data.replace(data) != inner.data;
// 			Ok(())
// 		} else {
// 			Err(anyhow!("Wrong data type"))
// 		}
// 	}
//
// 	pub fn get(&self) -> Option<Data> {
// 		self.inner.read().data.clone()
// 	}
//
// 	pub fn is_some(&self) -> bool {
// 		self.inner.read().data.is_some()
// 	}
//
// 	pub fn is_dirty(&self) -> bool {
// 		let read = self.inner.read();
// 		read.dirty
// 	}
//
// 	pub fn clear(&self) {
// 		self.inner.write().dirty = false;
// 	}
// }
//
// impl Default for IO {
// 	fn default() -> Self {
// 		Self {
// 			inner: Arc::default(),
// 			kind: DataKind::Any,
// 		}
// 	}
// }

// pub fn collect_inputs(inputs: &Vec<Arc<IO>>) -> Option<Vec<Data>> {
// 	let mut data = Vec::with_capacity(inputs.len());
// 	for input in inputs {
// 		if !input.is_some() {
// 			return None;
// 		}
//
// 		data.push(input.get().unwrap());
// 	}
//
// 	Some(data)
// }

// #[derive(Serialize, Deserialize, Debug)]
// pub enum Field {
// 	Author,
// 	Summary,
// 	Content,
// 	Title,
// 	// Uri
// }
