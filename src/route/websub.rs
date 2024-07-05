use crate::{app::AppState, route::Atom};
use axum::{
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
	routing::get,
	Router,
};
use uuid::Uuid;

pub async fn receive(
	Path(uuid): Path<Uuid>,
	State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let flow = state
		.flows
		.lock()
		.iter()
		.find(|(_, v)| v.uuid == uuid)
		.map(|(_, v)| v.clone());

	if let Some(flow) = flow {
		Ok("")
	} else {
		Err((StatusCode::NOT_FOUND, String::from("Not found")))
	}
}

pub fn router() -> Router<AppState> {
	Router::new().route("/:id", get(receive))
}
