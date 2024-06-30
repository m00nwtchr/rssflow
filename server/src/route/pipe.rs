use crate::app::AppState;
use crate::route::Atom;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

pub async fn run(
	Path(path): Path<String>,
	State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	if let Some(pipe) = state.pipelines.get(&path) {
		let channel = pipe
			.run()
			.await
			.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
		Ok(Atom(channel))
	} else {
		Err((StatusCode::NOT_FOUND, String::from("Not found")))
	}
}

pub fn router() -> Router<AppState> {
	Router::new().route("/:pipeline", get(run))
}
