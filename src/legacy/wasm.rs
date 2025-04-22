use std::{fmt::Write, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;
use pipe::{MyInputPipe, MyOutputPipe};
use tokio::sync::Mutex;
use wasmtime::{Config, Engine, Linker, Module, Store, TypedFunc};
use wasmtime_wasi::{preview1, preview1::WasiP1Ctx, WasiCtxBuilder};

use super::node::{collect_inputs, Data, DataKind, NodeTrait, IO};

/// Run a WASI module as a [Node]
pub struct Wasm {
	store: Mutex<Store<WasiP1Ctx>>,
	func: TypedFunc<(), ()>,

	stdin: MyInputPipe,
	stdout: MyOutputPipe,

	inputs: Vec<Arc<IO>>,
	outputs: Vec<Arc<IO>>,

	input_types: Box<[DataKind]>,
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
			func,

			stdin,
			stdout,

			inputs: Vec::new(),
			outputs: Vec::new(),

			input_types: inputs.into(),
			output_types: outputs.into(),
		})
	}
}

#[async_trait]
impl NodeTrait for Wasm {
	fn inputs(&self) -> &[Arc<IO>] {
		&self.inputs
	}

	fn outputs(&self) -> &[Arc<IO>] {
		&self.outputs
	}

	fn input_types(&self) -> &[DataKind] {
		&self.input_types
	}

	fn output_types(&self) -> &[DataKind] {
		&self.output_types
	}

	#[tracing::instrument(name = "wasm_node", skip(self))]
	async fn run(&self) -> anyhow::Result<()> {
		if let Some(input) = collect_inputs(&self.inputs) {
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

	fn set_input(&mut self, index: usize, input: Arc<IO>) {
		self.inputs.insert(index, input);
	}
	fn set_output(&mut self, index: usize, output: Arc<IO>) {
		self.outputs.insert(index, output);
	}
}

mod pipe {
	#![allow(clippy::module_name_repetitions)]
	use std::sync::Arc;

	use bytes::{Bytes, BytesMut};
	use parking_lot::Mutex;
	use wasmtime_wasi::{
		InputStream, OutputStream, Pollable, StdinStream, StdoutStream, StreamError,
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

	#[async_trait::async_trait]
	impl OutputStream for MyOutputPipe {
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
	impl Pollable for MyOutputPipe {
		async fn ready(&mut self) {}
	}

	impl StdoutStream for MyOutputPipe {
		fn stream(&self) -> Box<dyn OutputStream> {
			Box::new(self.clone())
		}

		fn isatty(&self) -> bool {
			false
		}
	}

	#[async_trait::async_trait]
	impl InputStream for MyInputPipe {
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
	impl Pollable for MyInputPipe {
		async fn ready(&mut self) {}
	}

	impl StdinStream for MyInputPipe {
		fn stream(&self) -> Box<dyn InputStream> {
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
