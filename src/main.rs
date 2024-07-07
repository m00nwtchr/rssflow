#![warn(clippy::pedantic)]

mod app;
mod config;
mod convert;
mod feed;
mod flow;
mod route;
mod websub;

use std::net::SocketAddr;
use crate::{app::app, config::AppConfig};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt::init();

	let config = AppConfig::load()?;

	let listener = tokio::net::TcpListener::bind(SocketAddr::new(config.address, config.port)).await.unwrap();
	axum::serve(listener, app(config).await?).await.unwrap();

	Ok(())
}
