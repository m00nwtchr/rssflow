#![warn(clippy::pedantic)]

use crate::app::app;

mod app;
mod pipeline;
mod route;
// mod rss;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
	tracing_subscriber::fmt::init();

	let listener = tokio::net::TcpListener::bind("[::]:3434").await.unwrap();
	axum::serve(listener, app().await).await.unwrap();
}
