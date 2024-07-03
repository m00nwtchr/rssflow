use std::{fmt::Write, marker::PhantomData};

use anyhow::anyhow;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::Mutex;
use wasmtime::{Config, Engine, Linker, Module, Store, TypedFunc};
use wasmtime_wasi::{preview1, preview1::WasiP1Ctx, WasiCtxBuilder};

use super::{dummy::Dummy, node::NodeTrait};
use pipe::{MyInputPipe, MyOutputPipe};

pub struct Wasm<O, T: NodeTrait = Dummy> {
	store: Mutex<Store<WasiP1Ctx>>,

	stdin: MyInputPipe,
	stdout: MyOutputPipe,

	func: TypedFunc<(), ()>,

	child: Option<T>,
	_phantom: PhantomData<O>,
}

impl<O, T: NodeTrait> Wasm<O, T> {
	pub async fn new(wat: impl AsRef<[u8]>) -> anyhow::Result<Self> {
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
			child: None,
			_phantom: PhantomData,
		})
	}

	pub fn child(mut self, child: T) -> Self {
		self.child = Some(child);
		self
	}
}

#[async_trait]
impl<O, T: NodeTrait> NodeTrait for Wasm<O, T>
where
	T::Item: Serialize,
	O: Sync + Send + DeserializeOwned,
{
	type Item = O;

	async fn run(&self) -> anyhow::Result<Self::Item> {
		if let Some(child) = &self.child {
			let json = serde_json::to_string(&child.run().await?)?;
			self.stdin.buffer.lock().unwrap().write_str(&json)?;
		}

		self.func
			.call_async(&mut *self.store.lock().await, ())
			.await?;

		let out = serde_json::from_slice(&self.stdout.buffer.lock().unwrap())?;
		self.stdout.clear();

		Ok(out)
	}
}

mod pipe {
	#![allow(clippy::module_name_repetitions)]
	use bytes::{Bytes, BytesMut};
	use std::sync::{Arc, Mutex};
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
			self.buffer.lock().unwrap().clear();
		}
	}

	impl HostOutputStream for MyOutputPipe {
		fn write(&mut self, bytes: Bytes) -> Result<(), StreamError> {
			let mut buf = self.buffer.lock().unwrap();
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
			let mut buffer = self.buffer.lock().unwrap();
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
	use crate::flow::feed::Feed;
	use rss::Channel;

	#[tokio::test]
	pub async fn wasm() -> anyhow::Result<()> {
		let flow = Feed::new("https://www.azaleaellis.com/tag/pgts/feed".parse()?)
			.wasm::<Channel>(include_bytes!("../../wasm_node_test.wasm"))
			.await?;

		let rss = flow.run().await?;
		println!("{}", serde_json::to_string_pretty(&rss)?);
		let rss = flow.run().await?;
		Ok(())
	}
}
