use std::{collections::HashMap, ops::Deref, sync::Arc};

use axum::{extract::FromRef, routing::get, Router};
use futures::StreamExt;
use sqlx::{
	sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow},
	Executor, Row, SqlitePool,
};
use tokio::sync::Mutex;

use crate::{
	config::AppConfig,
	flow::{Flow, FlowBuilder},
	route,
};

#[allow(clippy::module_name_repetitions)]
pub struct AppStateInner {
	pub flows: Mutex<HashMap<String, Arc<Flow>>>,
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

fn load_flow(row: &SqliteRow) -> anyhow::Result<Flow> {
	let flow: FlowBuilder = serde_json::de::from_str(row.get("content"))?;

	Ok(flow.simple())
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

	let flows = conn
		.fetch(sqlx::query!("SELECT * FROM flows"))
		.filter_map(|f| async { f.ok() })
		.filter_map(|row| async move {
			let name: String = row.get("name");

			if let Ok(flow) = load_flow(&row).map(Arc::new) {
				tracing::info!("Loaded `{name}` flow");
				Some((name, flow))
			} else {
				tracing::error!("Failed loading `{name}` flow");
				None
			}
		})
		.collect()
		.await;

	let state = AppState(Arc::new(AppStateInner {
		flows: Mutex::new(flows),
		pool,
		config: Arc::new(config),
	}));

	Ok(
		Router::new()
			.nest("/api", route::api())
			.nest("/websub", route::websub())
			.nest("/flow", route::flow())
			.route("/", get(|| async { "Hello, World!".to_string() }))
			.with_state(state), // .with_state(config.toml)
	)
}
