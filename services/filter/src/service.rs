use std::{
	str::FromStr,
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use proto::{
	feed::Feed,
	node::{Field, ProcessRequest, ProcessResponse, node_service_server::NodeService},
	registry::Node,
	websub::{SubscribeRequest, WebSub, WebSubEvent, web_sub_service_client::WebSubServiceClient},
};
use regex::Regex;
use tonic::{Request, Response, Status};
use tracing::{info, instrument};

use crate::FilterNode;

enum Filter {
	Regex(Regex),
	Contains(String),
}

#[tonic::async_trait]
impl NodeService for FilterNode {
	#[instrument(skip(self))]
	async fn process(
		&self,
		request: Request<ProcessRequest>,
	) -> Result<Response<ProcessResponse>, Status> {
		if let Some(node) = request.metadata().get("x-node") {
			if node != "Filter" {
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

		let invert = match request
			.options
			.as_ref()
			.and_then(|o| o.fields.get("invert"))
		{
			Some(v) => match &v.kind {
				Some(prost_types::value::Kind::BoolValue(b)) => *b,
				_ => Err(Status::invalid_argument("wrong type for invert"))?,
			},
			None => false,
		};

		let field = match request.options.as_ref().and_then(|o| o.fields.get("field")) {
			Some(v) => match &v.kind {
				Some(prost_types::value::Kind::NumberValue(i)) => Field::try_from(*i as i32)
					.map_err(|e| Status::invalid_argument("not a valid field enum value"))?,
				_ => Err(Status::invalid_argument("wrong type for field"))?,
			},
			None => Err(Status::invalid_argument("field option is missing"))?,
		};

		let filter = match request
			.options
			.as_ref()
			.and_then(|o| o.fields.get("contains"))
		{
			Some(v) => match &v.kind {
				Some(prost_types::value::Kind::StringValue(i)) => Filter::Contains(i.clone()),
				_ => Err(Status::invalid_argument("wrong type for contains"))?,
			},
			None => match request.options.as_ref().and_then(|o| o.fields.get("regex")) {
				Some(v) => match &v.kind {
					Some(prost_types::value::Kind::StringValue(i)) => {
						Filter::Regex(Regex::from_str(i).map_err(|e| {
							Status::invalid_argument(format!("invalid regex: {}", e.to_string()))
						})?)
					}
					_ => Err(Status::invalid_argument("wrong type for regex"))?,
				},
				None => Err(Status::invalid_argument(
					"no filter option: oneof [contains, regex]",
				))?,
			},
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
}
