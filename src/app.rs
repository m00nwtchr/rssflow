use axum::{Router, http::StatusCode, routing::get};

use crate::{RSSFlow, route};

// #[derive(Clone)]
// pub struct FlowHandle(Arc<Flow>, broadcast::Sender<Data>);
// impl FlowHandle {
// 	pub fn new(arc: Arc<Flow>) -> Self {
// 		FlowHandle(arc, broadcast::channel(100).0)
// 	}
//
// 	pub fn tx(&self) -> &broadcast::Sender<Data> {
// 		&self.1
// 	}
//
// 	pub fn subscribe(&self) -> broadcast::Receiver<Data> {
// 		self.1.subscribe()
// 	}
// }
//
// impl Deref for FlowHandle {
// 	type Target = Arc<Flow>;
//
// 	fn deref(&self) -> &Self::Target {
// 		&self.0
// 	}
// }

// fn load_flow(content: &str) -> anyhow::Result<Flow> {
// 	let flow: FlowBuilder = serde_json::de::from_str(content)?;
//
// 	Ok(flow.build())
// }

pub async fn app(state: RSSFlow) -> anyhow::Result<Router> {
	// let mut conn = pool.acquire().await?;

	// let flows = sqlx::query!("SELECT * FROM flows")
	// 	.fetch(&mut *conn)
	// 	.filter_map(|f| async { f.ok() })
	// 	.filter_map(|record| async move {
	// 		if let Ok(flow) = load_flow(&record.content).map(Arc::new) {
	// 			tracing::info!("Loaded `{}` flow", record.name);
	// 			Some((record.name, flow))
	// 		} else {
	// 			tracing::error!("Failed loading `{}` flow", record.name);
	// 			None
	// 		}
	// 	})
	// 	.map(|(k, v)| (k, FlowHandle::new(v)))
	// 	.collect()
	// 	.await;

	// let web_sub_subscriber = WebSubSubscriber::new(pool.clone());
	// let state = AppState(Arc::new(AppStateInner {
	// 	// flows: Mutex::new(flows),
	// 	pool,
	// 	// web_sub_subscriber,
	// 	rssflow,
	// }));

	let router = Router::new()
		.nest("/api", route::api())
		.nest("/flow", route::flow())
		.route("/", get(|| async { StatusCode::OK }))
		.with_state(state);

	Ok(router)
}
