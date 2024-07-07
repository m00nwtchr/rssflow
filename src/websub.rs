use rand::{distributions::Uniform, Rng};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqliteConnection};
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
		conn: &mut SqliteConnection,
	) -> anyhow::Result<()> {
		let record = sqlx::query!("SELECT uuid, secret FROM websub WHERE flow = ?", flow)
			.fetch_optional(&mut *conn)
			.await?;

		let uuid = if let Some(record) = &record {
			Uuid::from_slice(&record.uuid)?
		} else {
			Uuid::new_v7(Timestamp::now(NoContext))
		};

		let secret = if let Some(record) = &record {
			&record.secret
		} else {
			&rand::thread_rng()
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
		if record.is_none() {
			sqlx::query!(
				"INSERT INTO websub (uuid, hub, topic, flow, secret) VALUES (?, ?, ?, ?, ?)",
				uuid,
				self.hub,
				self.this,
				flow,
				secret
			)
			.execute(&mut *conn)
			.await?;
		}

		let resp = rb.send().await?;
		tracing::info!("Response: {}", resp.status());
		resp.error_for_status()?;
		Ok(())
	}
}
