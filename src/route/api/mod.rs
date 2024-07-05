use crate::{
	app::AppState,
	flow::{
		node::{DataKind, Node},
		FlowBuilder,
	},
	route::internal_error,
};
use axum::{
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
	routing::{delete, get, put},
	Json, Router,
};
use serde::Serialize;
use sqlx::{Executor, Row, SqlitePool};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::{NoContext, Timestamp, Uuid};

#[derive(Serialize)]
struct FlowResult {
	name: String,
	flow: serde_json::value::Value,
}

async fn get_flows(
	State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	let rows: anyhow::Result<Vec<_>> = conn
		.fetch_all(sqlx::query!("SELECT * FROM flows"))
		.await
		.map_err(internal_error)?
		.into_iter()
		.map(|s| -> anyhow::Result<FlowResult> {
			Ok(FlowResult {
				name: s.get::<String, _>(0),
				flow: serde_json::from_str(&s.get::<String, _>(1))?,
			})
		})
		.collect();

	Ok(Json(rows.map_err(|e| {
		(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
	})?))
}
async fn get_flow(
	Path(name): Path<String>,
	State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	let row = conn
		.fetch_one(sqlx::query!("SELECT * FROM flows WHERE name = ?", name))
		.await
		.map_err(internal_error)?;

	Ok(row.get::<String, _>(1))
}

async fn update_flow(
	Path(name): Path<String>,
	State(state): State<AppState>,
	State(pool): State<SqlitePool>,
	Json(flow): Json<FlowBuilder>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let json = serde_json::to_string(&flow).map_err(internal_error)?;

	let mut conn = pool.acquire().await.map_err(internal_error)?;

	if let Some(row) = conn
		.fetch_optional(sqlx::query!("SELECT * FROM flows WHERE name = ?", name))
		.await
		.map_err(internal_error)?
	{
		conn.execute(sqlx::query!(
			"UPDATE flows SET content = ? WHERE name = ?",
			json,
			name
		))
		.await
		.map_err(internal_error)?;

		let uuid = Uuid::from_slice(row.get("uuid")).map_err(internal_error)?;

		state
			.flows
			.lock()
			.insert(name.clone(), Arc::new(flow.simple(DataKind::Feed, uuid)));

		Ok(StatusCode::NO_CONTENT)
	} else {
		let uuid = Uuid::new_v7(Timestamp::now(NoContext));
		let blob = uuid.as_bytes().as_slice();

		conn.execute(sqlx::query!(
			"INSERT INTO flows (uuid, name, content) VALUES (?, ?, ?)",
			blob,
			name,
			json
		))
		.await
		.map_err(internal_error)?;

		state
			.flows
			.lock()
			.insert(name.clone(), Arc::new(flow.simple(DataKind::Feed, uuid)));

		Ok(StatusCode::CREATED)
	}
}

async fn delete_flow(
	Path(name): Path<String>,
	State(state): State<AppState>,
	State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	if state.flows.lock().remove(&name).is_some() {
		let mut conn = pool.acquire().await.map_err(internal_error)?;
		conn.execute(sqlx::query!("DELETE FROM flows WHERE name = ?", name))
			.await
			.map_err(internal_error)?;
	}

	Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
	Router::new()
		// .route("/flow", post(create_flow))
		.route("/flow", get(get_flows))
		.route("/flow/:name", get(get_flow))
		.route("/flow/:name", put(update_flow))
		.route("/flow/:name", delete(delete_flow))
}
