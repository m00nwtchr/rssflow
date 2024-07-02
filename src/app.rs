use crate::{
	flow,
	flow::{
		filter::{Field, Kind},
		node::{Node, NodeTrait, RSSNode},
	},
	route,
};
use axum::{extract::FromRef, routing::get, Router};
use rss::Channel;
use scraper::Selector;
use sqlx::{
	sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
	Executor, Row, SqlitePool,
};
use std::{collections::HashMap, env::var, ops::Deref, sync::Arc};
use tokio::sync::Mutex;

#[allow(clippy::module_name_repetitions)]
pub struct AppStateInner {
	pub flows: Mutex<HashMap<String, Arc<RSSNode>>>,
	pub pool: SqlitePool,
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

pub async fn app() -> anyhow::Result<Router> {
	let mut flows = HashMap::new();

	let pool = SqlitePoolOptions::new()
		.connect_with(
			SqliteConnectOptions::new()
				.filename(var("DATABASE_FILE").unwrap_or("rssflow.db".to_string()))
				.journal_mode(SqliteJournalMode::Wal)
				.create_if_missing(true),
		)
		.await?;
	sqlx::migrate!().run(&pool).await?;

	let mut conn = pool.acquire().await?;

	for row in conn.fetch_all(sqlx::query!("SELECT * FROM flows")).await? {
		let node: Node = serde_json::de::from_str(&row.get::<String, _>(1))?;
		flows.insert(row.get::<String, _>(0), Arc::new(node.into()));
	}

	let state = AppState(Arc::new(AppStateInner {
		flows: Mutex::new(flows),
		pool,
	}));

	Ok(
		Router::new()
			.nest("/api", route::api())
			.nest("/flow", route::flow())
			.route("/", get(|| async { "Hello, World!".to_string() }))
			.with_state(state), // .with_state(config)
	)
}
