use std::{borrow::Cow, str::FromStr};

use anyhow::anyhow;
use rand::{distributions::Uniform, Rng};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use uuid::{NoContext, Timestamp, Uuid};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSub {
	pub topic: String,
	pub hub: String,
}

impl WebSub {
	/// Subscribe to the given topic, returns true if this is a new subscription, false if the subscription is renewed.
	pub async fn subscribe(
		&self,
		public_url: &str,
		conn: &mut SqliteConnection,
	) -> anyhow::Result<bool> {
		let record = sqlx::query!(
			r#"SELECT uuid as "uuid!: Uuid", secret FROM websub WHERE topic = ?"#,
			self.topic
		)
		.fetch_optional(&mut *conn)
		.await?;

		let uuid = if let Some(record) = &record {
			Cow::Borrowed(&record.uuid)
		} else {
			Cow::Owned(Uuid::new_v7(Timestamp::now(NoContext)))
		};

		let secret = if let Some(record) = &record {
			Cow::Borrowed(&record.secret)
		} else {
			Cow::Owned(
				rand::thread_rng()
					.sample_iter(Uniform::new(' ', '~'))
					.take(64)
					.map(char::from)
					.collect(),
			)
		};

		let callback = format!("{public_url}websub/{uuid}");
		let rb = reqwest::Client::new().post(&self.hub).form(&[
			("hub.callback", callback.as_str()),
			("hub.mode", "subscribe"),
			("hub.topic", &self.topic),
			("hub.secret", secret.as_str()),
		]);

		if record.is_none() {
			let uuid = uuid.as_ref();
			let secret = secret.as_str();
			sqlx::query!(
				"INSERT INTO websub (uuid, topic, hub, secret) VALUES (?, ?, ?, ?)",
				uuid,
				self.topic,
				self.hub,
				secret,
			)
			.execute(&mut *conn)
			.await?;
		}

		let resp = rb.send().await?;
		tracing::info!("Response: {}", resp.status());
		resp.error_for_status()?;
		Ok(record.is_none())
	}

	pub async fn unsubscribe(
		&self,
		public_url: &str,
		conn: &mut SqliteConnection,
	) -> anyhow::Result<()> {
		let uuid = sqlx::query_scalar!(
			r#"SELECT uuid as "uuid!: Uuid" FROM websub WHERE topic = ?"#,
			self.topic
		)
		.fetch_one(&mut *conn)
		.await?;

		let callback = format!("{public_url}websub/{uuid}");
		let rb = reqwest::Client::new().post(&self.hub).form(&[
			("hub.callback", callback.as_str()),
			("hub.mode", "unsubscribe"),
			("hub.topic", &self.topic),
		]);

		sqlx::query!("UPDATE websub SET subscribed = 0 WHERE uuid = ?", uuid)
			.execute(&mut *conn)
			.await?;

		let resp = rb.send().await?;
		tracing::info!("Response: {}", resp.status());
		resp.error_for_status()?;
		Ok(())
	}
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
