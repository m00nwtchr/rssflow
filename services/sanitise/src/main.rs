#![warn(clippy::pedantic)]

use rssflow_service::service::ServiceBuilder;

mod service;

#[must_use] pub fn default_ammonia() -> ammonia::Builder<'static> {
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

pub const SERVICE_NAME: &str = "Sanitise";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	ServiceBuilder::new(SERVICE_NAME)
		.await?
		.with_node_service(SanitizeNode::default())
		.await
		.run()
		.await
}
