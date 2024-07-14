use std::{collections::HashSet, slice, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::node::{Data, DataKind, NodeTrait, IO};

#[derive(Serialize, Deserialize, Debug)]
pub struct Seen {
	#[serde(default)]
	store: Store,

	#[serde(skip)]
	input: Arc<IO>,
	#[serde(skip)]
	output: Arc<IO>,
}

impl Seen {
	pub fn new() -> Self {
		Self {
			store: Store::default(),

			output: Arc::default(),
			input: Arc::default(),
		}
	}
}

#[async_trait]
impl NodeTrait for Seen {
	fn inputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.input)
	}

	fn outputs(&self) -> &[Arc<IO>] {
		slice::from_ref(&self.input)
	}

	fn input_types(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	fn output_types(&self) -> &[DataKind] {
		&[DataKind::Feed]
	}

	#[tracing::instrument(name = "seen_node")]
	async fn run(&self) -> anyhow::Result<()> {
		let Some(Data::Feed(mut atom)) = self.input.get() else {
			return Err(anyhow!("Input data not available"));
		};

		if let Store::Internal(seen) = &self.store {
			let mut seen = seen.lock();

			// seen.retain(|id| atom.entries.iter().any(|i| i.id.eq(id)));
			atom.entries.retain(|item| seen.insert(item.id.clone()));
		}

		if !atom.entries.is_empty() {
			self.output.accept(atom)?;
		}
		Ok(())
	}

	fn set_input(&mut self, _index: usize, input: Arc<IO>) {
		self.input = input;
	}
	fn set_output(&mut self, _index: usize, output: Arc<IO>) {
		self.output = output;
	}
}

#[derive(Debug, Serialize, Deserialize)]
enum Store {
	Internal(#[serde(skip)] Mutex<HashSet<String>>),
	// External,
}

impl Default for Store {
	fn default() -> Self {
		Self::Internal(Mutex::default())
	}
}
