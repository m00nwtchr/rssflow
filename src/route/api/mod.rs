use axum::{
	Json, Router,
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
	routing::{delete, get, put},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::error;

use super::internal_error;
use crate::{RSSFlow, flow::Flow};

#[derive(Serialize, Deserialize)]
struct FlowResult {
	name: String,
	content: Flow,
}

async fn get_flows(
	State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	let results: Vec<_> = sqlx::query!("SELECT name, content FROM flows")
		.fetch_all(&mut *conn)
		.await
		.map_err(internal_error)?
		.into_iter()
		.filter_map(|r| {
			Some(FlowResult {
				name: r.name,
				content: serde_json::from_str(&r.content)
					.inspect_err(|err| error!("{err}"))
					.ok()?,
			})
		})
		.collect();

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

	let flow: Flow = serde_json::from_str(&content).map_err(internal_error)?;
	Ok(Json(flow))
}

async fn update_flow(
	Path(name): Path<String>,
	State(state): State<RSSFlow>,
	State(pool): State<SqlitePool>,
	Json(flow): Json<Flow>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let json = serde_json::to_string(&flow).map_err(internal_error)?;

	// let flow = flow.build();
	// flow.run()
	// 	.await
	// 	.map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;

	let mut conn = pool.acquire().await.map_err(internal_error)?;
	let update: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM flows WHERE name = ?)")
		.bind(&name)
		.fetch_one(&mut *conn)
		.await
		.map_err(internal_error)?;

	// if flow.has_subscriptions() {
	// 	state
	// 		.web_sub_subscriber
	// 		.register_flow(&name, &flow)
	// 		.await
	// 		.map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
	// }

	// state
	// 	.flows
	// 	.lock()
	// 	.await
	// 	.insert(name.clone(), FlowHandle::new(Arc::new(flow)));

	if update {
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
	}
}

async fn delete_flow(
	Path(name): Path<String>,
	State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	sqlx::query!("DELETE FROM flows WHERE name = ?", name)
		.execute(&mut *conn)
		.await
		.map_err(internal_error)?;

	// 	state
	// 		.web_sub_subscriber
	// 		.unregister_flow(flow)
	// 		.await
	// 		.map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

	Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<RSSFlow> {
	Router::new()
		// .route("/flow", post(create_flow))
		.route("/flow", get(get_flows))
		.route("/flow/{name}", get(get_flow))
		.route("/flow/{name}", put(update_flow))
		.route("/flow/{name}", delete(delete_flow))
}
