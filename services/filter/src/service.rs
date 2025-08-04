use std::str::FromStr;

use regex::Regex;
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
use tonic::{Request, Response, Status};
use tracing::instrument;

use crate::FilterNode;

enum Filter {
	Regex(Regex),
	Contains(String),
}

#[tonic::async_trait]
impl NodeService for FilterNode {
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
			Field::try_from(*f as i32)
				.map_err(|e| Status::invalid_argument("not a valid field enum value"))
		})?;

		let filter = match request.get_option::<&String>("contains") {
			Some(cr) => cr.map(|s| Filter::Contains(s.clone()))?,
			None => match request.get_option::<&String>("regex") {
				Some(rr) => rr.and_then(|s| {
					Regex::from_str(s)
						.map(Filter::Regex)
						.map_err(|e| Status::invalid_argument(format!("invalid regex: {e}")))
				})?,
				None => Err(Status::invalid_argument(
					"no filter option: oneof [contains, regex]",
				))?,
			},
		};

		let invert = match request.get_option::<&bool>("invert") {
			Some(r) => r.copied()?,
			None => false,
		};

		feed.entries.retain(|item| {
			let cmp = item.value(field);
			let cmp = if let Some(cmp) = cmp { cmp } else { "" };

			let value = match &filter {
				Filter::Regex(regex) => regex.is_match(cmp),
				Filter::Contains(str) => cmp.contains(str),
			};

			if invert { !value } else { value }
		});

		Ok(Response::new(ProcessResponse {
			payload: Some(feed.into()),
		}))
	}

	async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
		Self::respond_to_ping()
	}
}
