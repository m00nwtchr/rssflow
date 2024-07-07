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
			&record.uuid
		} else {
			&Uuid::new_v7(Timestamp::now(NoContext))
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
			("hub.topic", &self.topic),
			("hub.secret", secret),
		]);

		if record.is_none() {
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
}
