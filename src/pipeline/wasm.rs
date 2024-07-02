use crate::pipeline::node::NodeTrait;
use async_trait::async_trait;
use std::marker::PhantomData;
use wasmtime::{Caller, Engine, Linker, Module, Store};

// struct StoreData {
// 	// channel: Channel
// }

pub struct Wasm<T: NodeTrait, O = ()> {
	engine: Engine,
	linker: Linker<T::Item>,
	// store: Store<()>,
	module: Module,
	// instance: Instance,
	child: Option<T>,
	_phantom: PhantomData<O>,
}

impl<T: NodeTrait, O> Wasm<T, O> {
	pub fn new(module: Module) -> Self {
		let engine = Engine::default();
		let mut linker = Linker::new(&engine);

		linker
			.func_wrap(
				"host",
				"host_func",
				|caller: Caller<'_, T::Item>, param: i32| {
					println!("Got {param} from WebAssembly");
					// println!("my host state is: {}", caller.data());
				},
			)
			.expect("");

		Self {
			engine,
			linker,
			module, // store,
			// instance,
			child: None,
			_phantom: PhantomData,
		}
	}
}

#[async_trait]
impl<T: NodeTrait, O: Sync + Send + wasmtime::WasmResults> NodeTrait for Wasm<T, O>
where
	T::Item: Sync + Send,
{
	type Item = O;

	async fn run(&self) -> anyhow::Result<Self::Item> {
		let data = if let Some(child) = &self.child {
			child.run().await?
		} else {
			todo!()
		};
		let mut store: Store<T::Item> = Store::new(&self.engine, data);

		let instance = self.linker.instantiate(&mut store, &self.module)?;
		let func = instance.get_typed_func::<(), O>(&mut store, "run")?;

		Ok(func.call_async(&mut store, ()).await?)
	}
}
