#![warn(clippy::pedantic)]

use rssflow_service::{service::ServiceBuilder, service_info};
use tracing::instrument;

mod service;

#[must_use]
pub fn default_ammonia() -> ammonia::Builder<'static> {
	let mut ammonia = ammonia::Builder::new();
	ammonia.add_generic_attributes(["style"]);
	ammonia
}

struct SanitizeNode {
	ammonia: ammonia::Builder<'static>,
}

impl Default for SanitizeNode {
	fn default() -> Self {
		Self {
			ammonia: default_ammonia(),
		}
	}
}

service_info!("Sanitize");

#[tokio::main]
#[instrument]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	ServiceBuilder::new(SERVICE_INFO)?
		.with_node_service(SanitizeNode::default())
		.await
		.run()
		.await
}
