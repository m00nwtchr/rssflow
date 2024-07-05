#![warn(clippy::pedantic)]

mod app;
mod convert;
mod feed;
mod flow;
mod route;
mod websub;

use crate::app::app;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt::init();

	let listener = tokio::net::TcpListener::bind("[::]:3434").await.unwrap();
	axum::serve(listener, app().await?).await.unwrap();

	Ok(())
}
