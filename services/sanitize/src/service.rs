use std::thread::available_parallelism;

use futures::{StreamExt, stream};
use rssflow_service::{
	ServiceExt2, check_node,
	proto::{
		feed::Feed,
		node::{
			Field, PingRequest, PingResponse, ProcessRequest, ProcessResponse,
			node_service_server::NodeService,
		},
	},
	try_from_request,
};
use runesys::Service;
use tonic::{Request, Response, Status};
use tracing::instrument;

use crate::SanitizeNode;

#[tonic::async_trait]
impl NodeService for SanitizeNode {
	#[instrument(skip_all)]
	async fn process(
		&self,
		request: Request<ProcessRequest>,
	) -> Result<Response<ProcessResponse>, Status> {
		runesys::telemetry::propagation::accept_trace(&request);
		check_node::<Self>(&request)?;
		let request = request.into_inner();

		let mut feed: Feed = try_from_request(&request)?;

		let field = request.get_option_required("field").and_then(|f: &f64| {
			Field::try_from(*f as i32).map_err(|e| Status::invalid_argument(e.to_string()))
		})?;

		feed.entries = stream::iter(feed.entries.into_iter())
			.map(|mut item| async {
				let Some(value) = item.value_mut(field) else {
					return item;
				};
				*value = self.ammonia.clean(value).to_string();

				item
			})
			.buffered(available_parallelism()?.get())
			.collect()
			.await;

		Ok(Response::new(ProcessResponse {
			payload: Some(feed.into()),
		}))
	}

	async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
		Self::respond_to_ping()
	}
}
