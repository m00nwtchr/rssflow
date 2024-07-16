use std::{collections::HashMap, ops::Deref, sync::Arc};

use axum::{extract::FromRef, routing::get, Router};
use futures::StreamExt;
use sqlx::{
	sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
	SqlitePool,
};
use tokio::sync::{broadcast, Mutex};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use url::Url;

use crate::{
	config::AppConfig,
	flow::{node::Data, Flow, FlowBuilder},
	route,
};

#[derive(Clone)]
pub struct FlowHandle(Arc<Flow>, broadcast::Sender<Data>);
impl FlowHandle {
	pub fn new(arc: Arc<Flow>) -> Self {
		FlowHandle(arc, broadcast::channel(100).0)
	}

	pub fn tx(&self) -> &broadcast::Sender<Data> {
		&self.1
	}

	pub fn subscribe(&self) -> broadcast::Receiver<Data> {
		self.1.subscribe()
	}
}

impl Deref for FlowHandle {
	type Target = Arc<Flow>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[allow(clippy::module_name_repetitions)]
pub struct AppStateInner {
	pub flows: Mutex<HashMap<String, FlowHandle>>,
	pub pool: SqlitePool,
	pub config: Arc<AppConfig>,
}

#[derive(Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct AppState(Arc<AppStateInner>);

impl Deref for AppState {
	type Target = AppStateInner;

	#[allow(clippy::explicit_deref_methods)]
	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl FromRef<AppState> for SqlitePool {
	fn from_ref(input: &AppState) -> Self {
		input.pool.clone()
	}
}

impl FromRef<AppState> for Arc<AppConfig> {
	fn from_ref(input: &AppState) -> Self {
		input.config.clone()
	}
}

fn load_flow(content: &str) -> anyhow::Result<Flow> {
	let flow: FlowBuilder = serde_json::de::from_str(content)?;

	Ok(flow.build())
}

pub async fn websub_check(public_url: &Url) -> anyhow::Result<()> {
	let resp = reqwest::get(public_url.join("/websub/check")?).await?;

	resp.error_for_status()?;
	Ok(())
}

pub async fn app(config: AppConfig) -> anyhow::Result<Router> {
	let pool = SqlitePoolOptions::new()
		.connect_with(
			SqliteConnectOptions::new()
				.filename(&config.database_file)
				.journal_mode(SqliteJournalMode::Wal)
				.create_if_missing(true),
		)
		.await?;
	sqlx::migrate!().run(&pool).await?;

	let mut conn = pool.acquire().await?;

	let flows = sqlx::query!("SELECT * FROM flows")
		.fetch(&mut *conn)
		.filter_map(|f| async { f.ok() })
		.filter_map(|record| async move {
			if let Ok(flow) = load_flow(&record.content).map(Arc::new) {
				tracing::info!("Loaded `{}` flow", record.name);
				Some((record.name, flow))
			} else {
				tracing::error!("Failed loading `{}` flow", record.name);
				None
			}
		})
		.map(|(k, v)| (k, FlowHandle::new(v)))
		.collect()
		.await;

	let state = AppState(Arc::new(AppStateInner {
		flows: Mutex::new(flows),
		pool,
		config: Arc::new(config),
	}));

	Ok(Router::new()
		.nest("/api", route::api())
		.nest("/websub", route::websub())
		.nest("/flow", route::flow())
		.route("/", get(|| async { "Hello, World!".to_string() }))
		.with_state(state)
		.layer(ServiceBuilder::new().layer(TraceLayer::new_for_http())))
}
