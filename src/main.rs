#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
use std::net::SocketAddr;

use tokio::net::TcpListener;

mod app;
mod config;
mod feed;
mod flow;
mod route;
mod subscriber;

use crate::{
	app::{app, websub_check},
	config::config,
};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt::init();

	let config = config().await;

	let listener = TcpListener::bind(SocketAddr::new(config.address, config.port)).await?;
	if let Some(public_url) = &config.public_url {
		let public_url = public_url.clone();
		tokio::spawn(async move {
			if let Err(e) = websub_check(&public_url).await {
				tracing::error!("WebSub check failed. The endpoints at `{}` must be publicly accessible to allow WebSub push reception.", public_url.join("/websub/").unwrap());
				tracing::error!("WebSub check error: {}", e.root_cause());
			}
		});
	}
	axum::serve(listener, app().await?).await?;

	Ok(())
}
