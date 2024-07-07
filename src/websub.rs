use anyhow::anyhow;
use rand::{
	distributions::{Alphanumeric, Uniform},
	Rng, RngCore,
};
use serde::{Deserialize, Serialize};
use sqlx::{pool::PoolConnection, Executor, Row, Sqlite, SqliteConnection};
use url::Url;
use uuid::{NoContext, Timestamp, Uuid};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSub {
	pub hub: String,
	pub this: String,
}

impl WebSub {
	pub async fn subscribe(
		&self,
		flow: &str,
		public_url: &str,
		conn: &mut PoolConnection<Sqlite>,
	) -> anyhow::Result<()> {
		let row = conn
			.fetch_optional(sqlx::query!(
				"SELECT uuid, secret FROM websub WHERE flow = ?",
				flow
			))
			.await?;

		let uuid = if let Some(row) = &row {
			Uuid::from_slice(row.get("uuid"))?
		} else {
			Uuid::new_v7(Timestamp::now(NoContext))
		};

		let secret: String = if let Some(row) = &row {
			row.get("secret")
		} else {
			rand::thread_rng()
				.sample_iter(Uniform::new(' ', '~'))
				.take(64)
				.map(char::from)
				.collect()
		};

		let callback = format!("{public_url}websub/{uuid}");
		let rb = reqwest::Client::new().post(&self.hub).form(&[
			("hub.callback", callback.as_str()),
			("hub.mode", "subscribe"),
			("hub.topic", &self.this),
			("hub.secret", &secret),
		]);

		let uuid = uuid.as_bytes().as_slice();
		if row.is_none() {
			conn.execute(sqlx::query!(
				"INSERT INTO websub (uuid, hub, topic, flow, secret) VALUES (?, ?, ?, ?, ?)",
				uuid,
				self.hub,
				self.this,
				flow,
				secret
			))
			.await?;
		}

		let resp = rb.send().await?;
		tracing::info!("Response: {}", resp.status());
		resp.error_for_status()?;
		Ok(())
	}
}
