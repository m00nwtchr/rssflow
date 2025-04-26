use std::collections::HashMap;

use axum::{
	Extension, Router,
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
	routing::get,
};
use rssflow_service::{
	NodeExt,
	proto::{node::ProcessRequest, registry::Node},
};
use sqlx::PgPool;
use tracing::{info, instrument};

use crate::{
	RSSFlow,
	flow::Flow,
	route::{Atom, internal_error},
};

#[instrument(skip_all)]
async fn run(
	Path(name): Path<String>,
	State(state): State<RSSFlow>,
	Extension(pool): Extension<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	let mut conn = pool.acquire().await.map_err(internal_error)?;
	let content = sqlx::query_scalar!("SELECT content FROM flows WHERE name = $1", name)
		.fetch_one(&mut *conn)
		.await
		.map_err(|_| (StatusCode::NOT_FOUND, String::from("Not found")))?;

	let flow: Flow = serde_json::from_value(content).map_err(internal_error)?;

	let known_nodes: HashMap<String, Node> = state.nodes.lock().unwrap().clone();

	let mut payload = None;

	for node in flow.nodes {
		let service = known_nodes.get(&node.r#type).ok_or((
			StatusCode::UNPROCESSABLE_ENTITY,
			format!("No such node: {}", node.r#type),
		))?;

		info!("Sending request to {} node", node.r#type);
		let res = service
			.process(ProcessRequest {
				payload,
				options: node.options(),
			})
			.await
			.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
		payload = res.into_inner().payload;
	}

	if let Some(payload) = payload {
		let feed: rssflow_service::proto::feed::Feed =
			rssflow_service::proto::feed::Feed::try_from(payload).map_err(internal_error)?;
		Ok(Atom(feed.into()).into_response())
	} else {
		Ok(().into_response())
	}
}

#[instrument(skip_all)]
async fn subscribe(Path(name): Path<String>, State(state): State<RSSFlow>) -> StatusCode
//-> Result<Sse<impl Stream<Item = anyhow::Result<Event>>>, StatusCode>
{
	// let Some((_flow, rx)) = state
	// 	.flows
	// 	.lock()
	// 	.await
	// 	.get(&name)
	// 	.map(|h| ((*h).clone(), h.subscribe()))
	// else {
	// 	return Err(StatusCode::NOT_FOUND);
	// };

	// let stream = BroadcastStream::new(rx).map(|res| {
	// 	// let entries = res.map(|d| {
	// 	// 	if let Data::Feed(feed) = d {
	// 	// 		Data::Vec(feed.entries.into_iter().map(Data::Entry).collect())
	// 	// 	} else {
	// 	// 		d
	// 	// 	}
	// 	// });
	//
	// 	Ok(Event::default().json_data(res?)?)
	// });
	// Ok(Sse::new(stream).keep_alive(KeepAlive::default()))

	StatusCode::NOT_FOUND
}

pub fn router() -> Router<RSSFlow> {
	Router::new()
		.route("/{name}", get(run))
		.route("/{name}/sse", get(subscribe))
}
