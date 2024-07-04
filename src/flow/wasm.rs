use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fmt::Write, sync::Arc};
use tokio::sync::Mutex;
use wasmtime::{Config, Engine, Linker, Module, Store, TypedFunc};
use wasmtime_wasi::{preview1, preview1::WasiP1Ctx, WasiCtxBuilder};

use super::{node, node::NodeTrait};
use crate::flow::node::{Data, DataKind, IO};
use pipe::{MyInputPipe, MyOutputPipe};

pub struct Wasm {
	store: Mutex<Store<WasiP1Ctx>>,

	stdin: MyInputPipe,
	stdout: MyOutputPipe,

	func: TypedFunc<(), ()>,

	inputs: Vec<Arc<IO>>,
	outputs: Vec<Arc<IO>>,

	output_types: Box<[DataKind]>,
}

impl Wasm {
	#[tracing::instrument(name = "new_wasm_node", skip(wat))]
	pub async fn new(
		wat: impl AsRef<[u8]>,
		inputs: &[DataKind],
		outputs: &[DataKind],
	) -> anyhow::Result<Self> {
		let engine = Engine::new(Config::new().async_support(true))?;
		let module = Module::new(&engine, wat)?;

		let mut linker = Linker::new(&engine);
		preview1::add_to_linker_async(&mut linker, |t| t)?;

		let stdin = MyInputPipe::new();
		let stdout = MyOutputPipe::new();

		let wasi_ctx = WasiCtxBuilder::new()
			.stdin(stdin.clone())
			.stdout(stdout.clone())
			.inherit_stderr()
			.build_p1();

		let mut store = Store::new(&engine, wasi_ctx);

		linker.module_async(&mut store, "", &module).await?;
		let func = linker
			.get(&mut store, "", "run")
			.ok_or(anyhow!("fuck"))?
			.into_func()
			.ok_or(anyhow!("fuck"))?
			.typed(&store)?;

		Ok(Self {
			store: Mutex::new(store),
			stdin,
			stdout,
			func,

			inputs: inputs.iter().map(|d| Arc::new(IO::new(*d))).collect(),
			outputs: Vec::new(),
			output_types: outputs.iter().copied().collect(),
		})
	}
}

#[async_trait]
impl NodeTrait for Wasm {
	fn inputs(&self) -> Box<[Arc<IO>]> {
		self.inputs.iter().cloned().collect()
	}

	fn outputs(&self) -> Box<[DataKind]> {
		self.output_types.clone()
	}

	#[tracing::instrument(name = "wasm_node", skip(self))]
	async fn run(&self) -> anyhow::Result<()> {
		if let Some(input) = node::collect_inputs(&self.inputs) {
			let json = serde_json::to_string(&input)?;
			self.stdin.buffer.lock().write_str(&json)?;
		}

		self.func
			.call_async(&mut *self.store.lock().await, ())
			.await?;

		let out: Vec<Data> = serde_json::from_slice(&self.stdout.buffer.lock())?;
		self.stdout.clear();

		if out.len() != self.outputs.len() {
			return Err(anyhow!(
				"Wrong number of outputs from wasm module: {} / {}",
				out.len(),
				self.outputs.len()
			));
		}

		for (i, output) in out.into_iter().enumerate() {
			self.outputs.get(i).unwrap().accept(output)?;
		}

		Ok(())
	}

	fn set_outputs(&mut self, outputs: Vec<Arc<IO>>) {
		self.outputs = outputs;
	}
	fn output(&mut self, output: Arc<IO>) {
		self.outputs = vec![output];
	}
}

mod pipe {
	#![allow(clippy::module_name_repetitions)]
	use bytes::{Bytes, BytesMut};
	use parking_lot::Mutex;
	use std::sync::Arc;
	use wasmtime_wasi::{
		HostInputStream, HostOutputStream, StdinStream, StdoutStream, StreamError, Subscribe,
	};

	#[derive(Debug, Clone)]
	pub struct MyInputPipe {
		pub buffer: Arc<Mutex<BytesMut>>,
	}

	#[derive(Debug, Clone)]
	pub struct MyOutputPipe {
		pub buffer: Arc<Mutex<BytesMut>>,
	}

	impl MyInputPipe {
		pub fn new() -> Self {
			Self {
				buffer: Arc::new(Mutex::new(BytesMut::new())),
			}
		}
	}

	impl MyOutputPipe {
		pub fn new() -> Self {
			Self {
				buffer: Arc::new(Mutex::new(BytesMut::new())),
			}
		}

		pub fn clear(&self) {
			self.buffer.lock().clear();
		}
	}

	impl HostOutputStream for MyOutputPipe {
		fn write(&mut self, bytes: Bytes) -> Result<(), StreamError> {
			let mut buf = self.buffer.lock();
			buf.extend_from_slice(bytes.as_ref());
			// Always ready for writing
			Ok(())
		}
		fn flush(&mut self) -> Result<(), StreamError> {
			// This stream is always flushed
			Ok(())
		}
		fn check_write(&mut self) -> Result<usize, StreamError> {
			Ok(usize::MAX)
		}
	}

	#[async_trait::async_trait]
	impl Subscribe for MyOutputPipe {
		async fn ready(&mut self) {}
	}

	impl StdoutStream for MyOutputPipe {
		fn stream(&self) -> Box<dyn HostOutputStream> {
			Box::new(self.clone())
		}

		fn isatty(&self) -> bool {
			false
		}
	}

	#[async_trait::async_trait]
	impl HostInputStream for MyInputPipe {
		fn read(&mut self, size: usize) -> Result<Bytes, StreamError> {
			let mut buffer = self.buffer.lock();
			if buffer.is_empty() {
				return Err(StreamError::Closed);
			}

			let size = size.min(buffer.len());
			let read = buffer.split_to(size).freeze();
			Ok(read)
		}
	}

	#[async_trait::async_trait]
	impl Subscribe for MyInputPipe {
		async fn ready(&mut self) {}
	}

	impl StdinStream for MyInputPipe {
		fn stream(&self) -> Box<dyn HostInputStream> {
			Box::new(self.clone())
		}

		fn isatty(&self) -> bool {
			false
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	#[tokio::test]
	pub async fn wasm() -> anyhow::Result<()> {
		// let flow = Dummy::<Feed>::default()
		// 	.wasm::<Feed>(include_bytes!("../../wasm_node_test.wasm"))
		// 	.await?;

		// let atom = flow.run().await?;
		// println!("{}", serde_json::to_string_pretty(&atom)?);
		// let _atom = flow.run().await?;

		Ok(())
	}
}
