#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
use std::net::SocketAddr;

use tokio::net::TcpListener;

mod app;
mod config;
mod feed;
mod flow;
mod route;
mod websub;

use crate::{app::app, config::AppConfig};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt::init();

	let config = AppConfig::load()?;

	let listener = TcpListener::bind(SocketAddr::new(config.address, config.port)).await?;
	axum::serve(listener, app(config).await?).await?;

	Ok(())
}
