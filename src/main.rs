#![warn(clippy::pedantic)]

use crate::app::app;

mod app;
mod flow;
mod route;
mod convert;
// mod rss;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt::init();

	let listener = tokio::net::TcpListener::bind("[::]:3434").await.unwrap();
	axum::serve(listener, app().await?).await.unwrap();

	Ok(())
}
