use std::thread::available_parallelism;

use futures::{StreamExt, stream};
use rssflow_service::{
	check_node,
	proto::{
		feed::Feed,
		node::{Field, ProcessRequest, ProcessResponse, node_service_server::NodeService},
	},
	try_from_request,
};
use tonic::{Request, Response, Status};
use tracing::instrument;

use crate::ReplaceNode;

#[tonic::async_trait]
impl NodeService for ReplaceNode {
	#[instrument(skip_all)]
	async fn process(
		&self,
		request: Request<ProcessRequest>,
	) -> Result<Response<ProcessResponse>, Status> {
		runesys::telemetry::propagation::accept_trace(&request);
		check_node::<Self>(&request)?;
		let request = request.into_inner();

		let mut feed: Feed = try_from_request(&request)?;

		let old: &String = request.get_option_required("old")?;
		let new: &String = request.get_option_required("new")?;

		let field = request.get_option_required("field").and_then(|f: &f64| {
			Field::try_from(*f as i32)
				.map_err(|e| Status::invalid_argument("not a valid field enum value"))
		})?;

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
