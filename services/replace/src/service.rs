use std::thread::available_parallelism;

use futures::{StreamExt, stream};
use proto::{
	feed::Feed,
	node::{Field, ProcessRequest, ProcessResponse, node_service_server::NodeService},
};
use tonic::{Request, Response, Status};
use tracing::instrument;

use crate::ReplaceNode;

#[tonic::async_trait]
impl NodeService for ReplaceNode {
	#[instrument(skip(self))]
	async fn process(
		&self,
		request: Request<ProcessRequest>,
	) -> Result<Response<ProcessResponse>, Status> {
		if let Some(node) = request.metadata().get("x-node") {
			if node != "Replace" {
				return Err(Status::not_found(format!(
					"node {} not found",
					node.to_str().unwrap()
				)));
			}
		}

		let request = request.into_inner();
		let Some(payload) = request.payload else {
			return Err(Status::invalid_argument("payload missing"));
		};
		let mut feed = Feed::try_from(payload)
			.map_err(|e| Status::invalid_argument("payload is not a rssflow.feed.Feed"))?;

		let old = match request.options.as_ref().and_then(|o| o.fields.get("old")) {
			Some(v) => match &v.kind {
				Some(prost_types::value::Kind::StringValue(s)) => s,
				_ => Err(Status::invalid_argument("wrong type for old"))?,
			},
			None => Err(Status::invalid_argument("missing old option"))?,
		};

		let new = match request.options.as_ref().and_then(|o| o.fields.get("new")) {
			Some(v) => match &v.kind {
				Some(prost_types::value::Kind::StringValue(s)) => s,
				_ => Err(Status::invalid_argument("wrong type for new"))?,
			},
			None => Err(Status::invalid_argument("missing new option"))?,
		};

		let field = match request.options.as_ref().and_then(|o| o.fields.get("field")) {
			Some(v) => match &v.kind {
				Some(prost_types::value::Kind::NumberValue(i)) => Field::try_from(*i as i32)
					.map_err(|e| Status::invalid_argument("not a valid field enum value"))?,
				_ => Err(Status::invalid_argument("wrong type for field"))?,
			},
			None => Err(Status::invalid_argument("field option is missing"))?,
		};

		feed.entries = stream::iter(feed.entries.into_iter())
			.map(|mut item| async {
				let Some(value) = item.value_mut(field) else {
					return item;
				};
				*value = value.replace(old, new);

				item
			})
			.buffered(available_parallelism()?.get())
			.collect()
			.await;

		Ok(Response::new(ProcessResponse {
			payload: Some(feed.into()),
		}))
	}
}
