use axum::{
	extract::{Path, State},
	http::StatusCode,
	response::{
		sse::{Event, KeepAlive},
		IntoResponse, Sse,
	},
	routing::get,
	Router,
};
use futures::Stream;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::{
	app::AppState,
	flow::node::{Data, NodeTrait},
	route::Atom,
};

async fn run(
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

async fn subscribe(
	Path(name): Path<String>,
	State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = anyhow::Result<Event>>>, StatusCode> {
	let Some((_flow, rx)) = state
		.flows
		.lock()
		.await
		.get(&name)
		.map(|h| ((*h).clone(), h.subscribe()))
	else {
		return Err(StatusCode::NOT_FOUND);
	};

	let stream = BroadcastStream::new(rx).map(|res| {
		// let entries = res.map(|d| {
		// 	if let Data::Feed(feed) = d {
		// 		Data::Vec(feed.entries.into_iter().map(Data::Entry).collect())
		// 	} else {
		// 		d
		// 	}
		// });

		Ok(Event::default().json_data(res?)?)
	});
	Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

pub fn router() -> Router<AppState> {
	Router::new()
		.route("/{name}", get(run))
		.route("/{name}/sse", get(subscribe))
}
