// use crate::pipeline::Node;
// use async_trait::async_trait;
// use reqwest::Url;
// use rss::Channel;
// use wasmtime::{Caller, Engine, Instance, Linker, Module, Store};
// 
// struct StoreData {
// 	// channel: Channel
// }
// 
// pub struct WasmNode {
// 	engine: Engine,
// 	linker: Linker<StoreData>,
// 	// store: Store<()>,
// 	module: Module,
// 	// instance: Instance,
// }
// 
// impl WasmNode {
// 	pub fn new(module: Module) -> Self {
// 		let engine = Engine::default();
// 		let mut linker = Linker::new(&engine);
// 
// 		linker
// 			.func_wrap(
// 				"host",
// 				"host_func",
// 				|caller: Caller<'_, StoreData>, param: i32| {
// 					println!("Got {} from WebAssembly", param);
// 					println!("my host state is: {}", caller.data());
// 				},
// 			)
// 			.expect("");
// 
// 		Self {
// 			engine,
// 			linker,
// 			module, // store,
// 			        // instance,
// 		}
// 	}
// }
// 
// #[async_trait]
// impl Node<Channel> for WasmNode {
// 	async fn run(&self, _: Box<[Channel]>) -> anyhow::Result<Box<[Channel]>> {
// 		let mut store: Store<StoreData> = Store::new(&self.engine, StoreData {
// 			// channel: 
// 		});
// 
// 		let instance = self.linker.instantiate(&mut store, &self.module)?;
// 		instance.get_typed_func::<(), ()>(&mut store, "run")?;
// 
// 		Ok(Box::new([]))
// 	}
// }
