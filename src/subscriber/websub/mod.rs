use std::str::FromStr;

use anyhow::anyhow;
use bytes::Bytes;
use rand::{distributions::Uniform, Rng};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use tracing::Instrument;
use uuid::{NoContext, Timestamp, Uuid};

pub mod router;
pub use router::router;

use crate::{
	app::{AppState, FlowHandle},
	config::config,
	flow::{
		node::{Data, DataKind, NodeTrait, IO},
		Flow,
	},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSub {
	pub topic: String,
	pub hub: String,
}

impl FromStr for WebSub {
	type Err = anyhow::Error;

	fn from_str(header: &str) -> Result<Self, Self::Err> {
		let mut hub = None;
		let mut topic = None;

		// Split the header into individual link parts
		for part in header.split(',') {
			let segments: Vec<&str> = part.trim().split(';').collect();
			if segments.len() < 2 {
				continue;
			}

			let url_part = segments[0].trim();
			let rel_part = segments[1].trim();

			if !url_part.starts_with('<') || !url_part.ends_with('>') {
				continue;
			}

			// Extract the URL and rel values
			let url = &url_part[1..url_part.len() - 1];
			let rel = rel_part
				.split('=')
				.nth(1)
				.map_or("", |s| s.trim_matches('"'));

			match rel {
				"hub" => hub = Some(url.to_string()),
				"self" => topic = Some(url.to_string()),
				_ => (),
			}
		}

		Ok(WebSub {
			topic: topic.ok_or_else(|| anyhow!(""))?,
			hub: hub.ok_or_else(|| anyhow!(""))?,
		})
	}
}

pub struct WebSubSubscriber {
	pool: SqlitePool,
}

impl WebSubSubscriber {
	pub fn new(pool: SqlitePool) -> Self {
		Self { pool }
	}

	pub async fn register_flow(&self, name: &str, flow: &Flow) -> anyhow::Result<()> {
		let mut conn = self.pool.acquire().await?;
		// let mut tx = conn.begin().await?;

		sqlx::query!("DELETE FROM websub_flows WHERE flow = ?", name)
			.execute(&mut *conn)
			.await?;

		for websub in flow.subscriptions() {
			self.subscribe(&websub).await?;

			sqlx::query!(
				"INSERT OR IGNORE INTO websub_flows (topic, flow) VALUES (?, ?)",
				websub.topic,
				name
			)
			.execute(&mut *conn)
			.await?;
		}
		// tx.commit().await?;

		self.remove_unused_subscriptions(&mut conn).await?;

		Ok(())
	}

	pub async fn unregister_flow(&self, flow: FlowHandle) -> anyhow::Result<()> {
		if flow.has_subscriptions() {
			let mut conn = self.pool.acquire().await?;
			self.remove_unused_subscriptions(&mut conn).await?
		}

		Ok(())
	}

	async fn remove_unused_subscriptions(&self, conn: &mut SqliteConnection) -> anyhow::Result<()> {
		let res = sqlx::query_as!(
			WebSub,
			r#"
			SELECT topic, hub
			FROM websub
			WHERE NOT EXISTS (
				SELECT 1
				FROM websub_flows
				WHERE websub_flows.topic = websub.topic
			)
			"#
		)
		.fetch_all(&mut *conn)
		.await?;

		for websub in res {
			self.unsubscribe(&websub).await?;
		}

		Ok(())
	}

	pub async fn subscribe(&self, subscription: &WebSub) -> anyhow::Result<bool> {
		let config = config().await;
		let Some(public_url) = &config.public_url else {
			return Err(anyhow!(""));
		};

		let mut conn = self.pool.acquire().await?;
		let record = sqlx::query!(
			r#"SELECT uuid as "uuid!: Uuid", secret FROM websub WHERE topic = ?"#,
			subscription.topic
		)
		.fetch_optional(&mut *conn)
		.await?;
		let new_sub = record.is_none();

		let (uuid, secret) = if let Some(record) = record {
			(record.uuid, record.secret)
		} else {
			(
				Uuid::new_v7(Timestamp::now(NoContext)),
				rand::thread_rng()
					.sample_iter(Uniform::new(' ', '~'))
					.take(64)
					.collect(),
			)
		};

		let callback = format!("{public_url}websub/{uuid}");
		let rb = reqwest::Client::new().post(&subscription.hub).form(&[
			("hub.callback", callback.as_str()),
			("hub.mode", "subscribe"),
			("hub.topic", &subscription.topic),
			("hub.secret", secret.as_str()),
		]);

		if new_sub {
			tracing::info!(
				"Subscribed to `{}` at `{}`",
				subscription.topic,
				subscription.hub
			);

			sqlx::query!(
				"INSERT INTO websub (uuid, topic, hub, secret) VALUES (?, ?, ?, ?)",
				uuid,
				subscription.topic,
				subscription.hub,
				secret,
			)
			.execute(&mut *conn)
			.await?;
		}

		let resp = rb.send().await?;
		tracing::info!("Response: {}", resp.status());
		resp.error_for_status()?;
		Ok(new_sub)
	}

	pub async fn unsubscribe(&self, subscription: &WebSub) -> anyhow::Result<()> {
		let config = config().await;
		let Some(public_url) = &config.public_url else {
			return Err(anyhow!(""));
		};
		let mut conn = self.pool.acquire().await?;

		let uuid = sqlx::query_scalar!(
			r#"SELECT uuid as "uuid!: Uuid" FROM websub WHERE topic = ?"#,
			subscription.topic
		)
		.fetch_one(&mut *conn)
		.await?;

		let callback = format!("{public_url}websub/{uuid}");
		let rb = reqwest::Client::new().post(&subscription.hub).form(&[
			("hub.callback", callback.as_str()),
			("hub.mode", "unsubscribe"),
			("hub.topic", &subscription.topic),
		]);

		sqlx::query!("UPDATE websub SET subscribed = 0 WHERE uuid = ?", uuid)
			.execute(&mut *conn)
			.await?;

		let resp = rb.send().await?;
		tracing::info!("Response: {}", resp.status());
		resp.error_for_status()?;
		Ok(())
	}

	pub async fn handle(
		&self,
		state: &AppState,
		subscription: &WebSub,
		data: Bytes,
	) -> anyhow::Result<()> {
		let mut conn = self.pool.acquire().await?;

		let flows = sqlx::query_scalar!(
			"SELECT flow FROM websub_flows WHERE topic = ?",
			subscription.topic
		)
		.fetch_all(&mut *conn)
		.await?;
		if flows.is_empty() {
			return Ok(());
		}

		for name in flows {
			let Some(flow) = state.flows.lock().await.get(&name).cloned() else {
				return Ok(());
			};

			// TODO: Proper handling for multiple WebSub-type inputs in one flow
			if let Some(input) = flow
				.inputs()
				.iter()
				.find(|i| matches!(i.kind(), DataKind::WebSub))
			{
				let _ = input.accept(data.clone());

				let span = tracing::Span::current();
				tokio::spawn(async move {
					if let Ok(()) = flow.run().instrument(span.clone()).await {
						let _span = span.entered();
						if let Some(data) = flow.result() {
							match data {
								Data::Feed(feed) => {
									for entry in feed.entries.into_iter().rev() {
										let _ = flow.tx().send(Data::Entry(entry));
									}
								}
								_ => {
									let _ = flow.tx().send(data);
								}
							}
						}
					}
				});
			}
		}

		Ok(())
	}
}

// #[async_trait]
// impl Subscriber<WebSub, Bytes> for WebSubSubscriber {
//
// }
