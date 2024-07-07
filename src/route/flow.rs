use axum::{
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
	routing::get,
	Router,
};

use crate::{
	app::AppState,
	flow::node::{Data, NodeTrait},
	route::Atom,
};

#[tracing::instrument(name = "run_flow_handler", skip(state))]
pub async fn run(
	Path(name): Path<String>,
	State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	if let Some(flow) = state.flows.lock().await.get(&name).cloned() {
		flow.run()
			.await
			.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

		let Some(Data::Feed(feed)) = flow.result() else {
			return Err((StatusCode::INTERNAL_SERVER_ERROR, ":(".to_string()));
		};

		Ok(Atom(feed))
	} else {
		Err((StatusCode::NOT_FOUND, String::from("Not found")))
	}
}

pub fn router() -> Router<AppState> {
	Router::new().route("/:name", get(run))
}
