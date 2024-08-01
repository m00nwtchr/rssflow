use std::{cmp::min, slice, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use url::Url;

use super::node::{Data, DataKind, NodeTrait, IO};

/// Generates a response using an AI assistant.
#[derive(Serialize, Deserialize, Debug)]
pub struct AI {
	url: Url, // Ollama endpoint URL.

	model: String,
	// prompt: String,
	system: String,

	#[serde(skip)]
	input: Arc<IO>,
	#[serde(skip)]
	output: Arc<IO>,
}

impl AI {
	pub fn new(url: Url, model: String, system: String) -> Self {
		Self {
			url,
			model,
			system,
			input: Arc::default(),
			output: Arc::default(),
		}
	}
}

#[async_trait]
impl NodeTrait for AI {
	fn inputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.input)
	}

	fn outputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.output)
	}

	fn input_types(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	fn output_types(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	#[tracing::instrument(name = "ai", skip(self))]
	async fn run(&self) -> anyhow::Result<()> {
		let Some(Data::Feed(mut atom)) = self.input.get() else {
			return Err(anyhow!(""));
		};

		let n = min(atom.entries.len(), 4); // Avoiding too high values to prevent spamming the target site.
		let items: Vec<anyhow::Result<atom_syndication::Entry>> =
			stream::iter(atom.entries.into_iter())
				.map(|mut item| async move {
					let mut content = item.content.unwrap();

					let resp = reqwest::Client::new()
						.post(self.url.clone())
						.json(&OllamaRequest {
							model: self.model.clone(),
							prompt: content.value.unwrap(),
							stream: Some(false),
							system: Some(self.system.clone()),
							..Default::default()
						})
						.send()
						.await?;

					resp.error_for_status_ref()?;
					let body: OllamaResponse = resp.json().await?;

					content.value = Some(body.response);
					item.content = Some(content);
					Ok(item)
				})
				.buffered(n)
				.collect()
				.await;
		atom.entries = items.into_iter().collect::<anyhow::Result<_>>()?;

		self.output.accept(atom)
	}

	fn set_input(&mut self, _index: usize, input: Arc<IO>) {
		self.input = input;
	}
	fn set_output(&mut self, _index: usize, output: Arc<IO>) {
		self.output = output;
	}
}

#[derive(Serialize, Default)]
pub struct OllamaRequest {
	model: String,
	prompt: String,
	format: Option<String>,
	system: Option<String>,
	stream: Option<bool>,
}

#[derive(Deserialize)]
pub struct OllamaResponse {
	response: String,
}
