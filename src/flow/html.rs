use std::{
	slice,
	sync::Arc,
	time::{Duration, Instant},
};

use anyhow::anyhow;
use async_trait::async_trait;
use parking_lot::Mutex;
use reqwest::header;
use scraper::Selector;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationSeconds};
use url::Url;

use super::node::{Data, DataKind, NodeTrait, IO};
use crate::websub::WebSub;

fn mutex_now() -> Mutex<Instant> {
	Mutex::new(Instant::now())
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
pub struct Html {
	url: Url,

	#[serde_as(as = "DurationSeconds")]
	ttl: Duration,
	#[serde(skip, default = "mutex_now")]
	last_fetch: Mutex<Instant>,

	#[serde(skip)]
	web_sub: Mutex<Option<WebSub>>,

	#[serde(skip)]
	input: Arc<IO>,
	#[serde(skip)]
	output: Arc<IO>,
}

impl Html {
	pub fn new(url: Url, ttl: Duration) -> Self {
		Self {
			url,
			ttl,
			last_fetch: mutex_now(),
			web_sub: Mutex::default(),

			input: Arc::default(),
			output: Arc::default(),
		}
	}
}

#[async_trait]
impl NodeTrait for Html {
	fn inputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.input)
	}

	fn outputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.output)
	}

	fn input_types(&self) -> &[DataKind] {
		&[DataKind::WebSub]
	}

	fn output_types(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	fn is_dirty(&self) -> bool {
		!self.output.is_some()
			|| self.input.is_dirty()
			|| self.last_fetch.lock().elapsed() > self.ttl
	}

	#[tracing::instrument(name = "html_node")]
	async fn run(&self) -> anyhow::Result<()> {
		let sub = self.input.is_dirty();
		let content = if sub {
			let Some(Data::WebSub(websub)) = self.input.get() else {
				return Err(anyhow!(""));
			};

			websub
		} else {
			let response = reqwest::get(self.url.clone()).await?;

			response.bytes().await?
		};
		let html = scraper::Html::parse_document(std::str::from_utf8(&content)?);

		let mut links = html.select(&Selector::parse("link").map_err(|e| anyhow!(e.to_string()))?);

		if !sub {
			let hub = links
				.clone()
				.find(|l| l.attr("rel").map(|a| a.eq("hub")).unwrap_or_default());
			let this = links.find(|l| l.attr("rel").map(|a| a.eq("self")).unwrap_or_default());

			if let (Some(hub), Some(this)) = (hub, this) {
				self.web_sub.lock().replace(WebSub {
					hub: hub.attr("href").unwrap_or_default().to_string(),
					topic: this.attr("href").unwrap_or_default().to_string(),
				});
			}
		}

		todo!();
		*self.last_fetch.lock() = Instant::now();
		self.output.accept(todo!())
	}

	fn set_input(&mut self, _index: usize, input: Arc<IO>) {
		self.input = input;
	}
	fn set_output(&mut self, _index: usize, output: Arc<IO>) {
		self.output = output;
	}

	fn web_sub(&self) -> Option<WebSub> {
		self.web_sub.lock().clone()
	}
}
