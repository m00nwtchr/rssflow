use crate::pipeline::Node;
use async_trait::async_trait;
use wasmtime::{Caller, Engine, Linker, Module, Store};

// struct StoreData {
// 	// channel: Channel
// }

pub struct Wasm<T: Node<B>, B = ()> {
	engine: Engine,
	linker: Linker<B>,
	// store: Store<()>,
	module: Module,
	// instance: Instance,
	child: Option<T>,
}

impl<T: Node<B>, B> Wasm<T, B> {
	pub fn new(module: Module) -> Self {
		let engine = Engine::default();
		let mut linker = Linker::new(&engine);

		linker
			.func_wrap("host", "host_func", |caller: Caller<'_, B>, param: i32| {
				println!("Got {param} from WebAssembly");
				// println!("my host state is: {}", caller.data());
			})
			.expect("");

		Self {
			engine,
			linker,
			module, // store,
			// instance,
			child: None,
		}
	}
}

#[async_trait]
impl<T, B> Node<B> for Wasm<T, B>
where
	T: Node<B>,
{
	async fn run(&self) -> anyhow::Result<B> {
		let data = if let Some(child) = &self.child {
			child.run().await?
		} else {
			todo!()
		};
		let mut store: Store<B> = Store::new(&self.engine, data);

		let instance = self.linker.instantiate(&mut store, &self.module)?;
		instance.get_typed_func::<(), ()>(&mut store, "run")?;

		todo!()
	}
}
