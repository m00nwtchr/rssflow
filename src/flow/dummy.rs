use std::marker::PhantomData;

use async_trait::async_trait;

use super::node::NodeTrait;

#[derive(Default)]
pub struct Dummy<T: Default = ()>(PhantomData<T>);

#[async_trait]
impl<T: Default + Send + Sync> NodeTrait for Dummy<T> {
	type Item = T;

	async fn run(&self) -> anyhow::Result<Self::Item> {
		Ok(Default::default())
	}
}
