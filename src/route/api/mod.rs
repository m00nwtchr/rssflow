use std::sync::Arc;

use axum::{
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
	routing::{delete, get, put},
	Json, Router,
};
use serde::Serialize;
use sqlx::SqlitePool;

use super::internal_error;
use crate::{
	app::AppState,
	flow::{node::NodeTrait, FlowBuilder},
};

#[derive(Serialize)]
struct FlowResult {
	name: String,
	content: serde_json::value::Value,
}

async fn get_flows(
	State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	let results = sqlx::query_as!(FlowResult, "SELECT name, content FROM flows")
		.fetch_all(&mut *conn)
		.await
		.map_err(internal_error)?;

	Ok(Json(results))
}
async fn get_flow(
	Path(name): Path<String>,
	State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	let content = sqlx::query_scalar!("SELECT content FROM flows WHERE name = ?", name)
		.fetch_one(&mut *conn)
		.await
		.map_err(internal_error)?;

	Ok(content)
}

async fn update_flow(
	Path(name): Path<String>,
	State(state): State<AppState>,
	State(pool): State<SqlitePool>,
	Json(flow): Json<FlowBuilder>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let json = serde_json::to_string(&flow).map_err(internal_error)?;

	let flow = flow.simple();
	flow.run()
		.await
		.map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;

	let mut conn = pool.acquire().await.map_err(internal_error)?;
	let out = if sqlx::query_scalar!("SELECT 1 FROM flows WHERE name = ?", name)
		.fetch_optional(&mut *conn)
		.await
		.map_err(internal_error)?
		.is_some()
	{
		sqlx::query!("UPDATE flows SET content = ? WHERE name = ?", json, name)
			.execute(&mut *conn)
			.await
			.map_err(internal_error)?;

		Ok(StatusCode::NO_CONTENT)
	} else {
		sqlx::query!(
			"INSERT INTO flows (name, content) VALUES (?, ?)",
			name,
			json
		)
		.execute(&mut *conn)
		.await
		.map_err(internal_error)?;

		Ok(StatusCode::CREATED)
	};

	if let Some(websub) = flow.web_sub() {
		if let Some(public_url) = &state.config.public_url {
			tracing::info!("Subscribe to {} at {}", websub.this, websub.hub);
			websub
				.subscribe(&name, public_url.as_str(), &mut conn)
				.await
				.map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
		}
	}

	state
		.flows
		.lock()
		.await
		.insert(name.clone(), Arc::new(flow));

	out
}

async fn delete_flow(
	Path(name): Path<String>,
	State(state): State<AppState>,
	State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	if state.flows.lock().await.remove(&name).is_some() {
		let mut conn = pool.acquire().await.map_err(internal_error)?;
		sqlx::query!("DELETE FROM flows WHERE name = ?", name)
			.execute(&mut *conn)
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
