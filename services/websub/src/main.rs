#![warn(clippy::pedantic)]

use std::{
	collections::HashMap,
	ops::Deref,
	sync::{Arc, Mutex},
};

use rssflow_service::{
	proto::{
		registry::Node,
		websub::{WebSub, web_sub_service_server::WebSubServiceServer},
	},
	service::ServiceBuilder,
};
use url::Url;
use uuid::Uuid;

use crate::router::app;

pub mod router;
mod service;
mod ws;

pub async fn websub_check(public_url: &Url) {
	// let resp = reqwest::get(public_url.join("/websub/check")?).await?;

	// resp.error_for_status()?;
	// Ok(())
}

#[derive(Debug, Default, Clone)]
pub struct Subscription {
	web_sub: WebSub,
	nodes: Vec<Node>,
}

#[derive(Debug, Default)]
pub struct WebSubInner {
	subscriptions: Mutex<HashMap<Uuid, Subscription>>,
	ws: Mutex<HashMap<WebSub, Uuid>>,
}

#[derive(Debug, Clone, Default)]
pub struct WebSubSVC(Arc<WebSubInner>);

impl Deref for WebSubSVC {
	type Target = WebSubInner;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

pub const SERVICE_NAME: &str = "WebSub";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	rssflow_service::tracing::init();
	let svc = WebSubSVC::default();
	ServiceBuilder::new(SERVICE_NAME)
		.await?
		.with_service(WebSubServiceServer::new(svc.clone()))
		.with_http(app(svc))
		.run()
		.await
}
