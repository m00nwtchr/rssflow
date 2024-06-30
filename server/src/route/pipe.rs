use axum::{
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
	routing::get,
	Router,
};

use crate::{app::AppState, route::Atom};

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
