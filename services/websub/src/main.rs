#![warn(clippy::pedantic)]

use std::{
	collections::{HashMap, HashSet},
	ops::Deref,
	sync::{Arc, Mutex},
};

use rssflow_service::proto::{
	node::NodeMeta,
	websub::{WebSub, web_sub_service_server::WebSubServiceServer},
};
use runesys::Service;
use url::Url;
use uuid::Uuid;

use crate::router::app;

pub mod router;
mod service;
mod ws;

pub async fn websub_check(public_url: &Url) -> anyhow::Result<()> {
	let resp = reqwest::get(public_url.join("/websub/check")?).await?;

	resp.error_for_status()?;
	Ok(())
}

#[derive(Debug, Default, Clone)]
pub struct Subscription {
	web_sub: WebSub,
	nodes: HashSet<NodeMeta>,
}

#[derive(Debug, Default)]
pub struct WebSubInner {
	subscriptions: Mutex<HashMap<Uuid, Subscription>>,
	ws: Mutex<HashMap<WebSub, Uuid>>,
}

#[derive(Service, Debug, Clone, Default)]
#[service("WebSub")]
#[server(WebSubServiceServer)]
#[fd_set(rssflow_service::proto::FILE_DESCRIPTOR_SET)]
pub struct WebSubSVC(Arc<WebSubInner>);

impl Deref for WebSubSVC {
	type Target = WebSubInner;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[tokio::main]
async fn main() -> Result<(), runesys::error::Error> {
	runesys::tracing::init(&WebSubSVC::INFO);
	let svc = WebSubSVC::default();
	let app = app(svc.clone());

	svc.builder()
		.with_pg(|pool| async move { sqlx::migrate!().run(&pool).await })?
		.with_http(app)
		.run()
		.await
}
