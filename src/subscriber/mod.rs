use async_trait::async_trait;

use crate::config::AppConfig;

pub mod websub;

#[async_trait]
pub trait Subscriber<S, T> {
	async fn subscribe(&self, subscription: &S, config: &AppConfig) -> anyhow::Result<bool>;

	async fn handle(&self, subscription: &S, data: T) -> anyhow::Result<()>;
}
