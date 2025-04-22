use std::array::IntoIter;

use proto::websub::{
	SubscribeRequest, SubscribeResponse, WebSubEvent, WebSubRequest,
	web_sub_service_server::WebSubService,
};
use tonic::{Request, Response, Status};
use uuid::{NoContext, Timestamp, Uuid};

use crate::{Subscription, WebSubSVC};

#[tonic::async_trait]
impl WebSubService for WebSubSVC {
	async fn subscribe(
		&self,
		request: Request<SubscribeRequest>,
	) -> Result<Response<SubscribeResponse>, Status> {
		let request = request.into_inner();
		let Some(sub) = request.sub else {
			return Err(Status::invalid_argument("missing sub"));
		};
		let Some(node) = request.node else { todo!() };

		let uuid = self
			.ws
			.lock()
			.unwrap()
			.entry(sub.clone())
			.or_insert(Uuid::new_v7(Timestamp::now(NoContext)))
			.clone();
		self.subscriptions
			.lock()
			.unwrap()
			.entry(uuid.clone())
			.and_modify(|s| {
				s.nodes.push(node.clone());
			})
			.or_insert(Subscription {
				web_sub: sub.clone(),
				nodes: vec![node],
			});

		let public_url = "https://m00npc.tail096cd8.ts.net/";

		// 		let mut conn = self.pool.acquire().await?;
		// 		let record = sqlx::query!(
		// 			r#"SELECT uuid as "uuid!: Uuid", secret FROM websub WHERE topic = ?"#,
		// 			subscription.topic
		// 		)
		// 		.fetch_optional(&mut *conn)
		// 		.await?;
		// 		let new_sub = record.is_none();
		//
		// 		let (uuid, secret) = if let Some(record) = record {
		// 			(record.uuid, record.secret)
		// 		} else {
		// 			(
		// 				Uuid::new_v7(Timestamp::now(NoContext)),
		// 				Alphanumeric.sample_string(&mut rand::rng(), HMAC_SECRET_LENGTH),
		// 			)
		// 		};
		//
		let callback = format!("{public_url}websub/{uuid}");
		let rb = reqwest::Client::new().post(&sub.hub).form(&[
			("hub.callback", callback.as_str()),
			("hub.mode", "subscribe"),
			("hub.topic", &sub.topic),
			// ("hub.secret", secret.as_str()),
		]);
		//
		// 		if new_sub {
		// 			tracing::info!(
		// 				"Subscribed to `{}` at `{}`",
		// 				subscription.topic,
		// 				subscription.hub
		// 			);
		//
		// 			sqlx::query!(
		// 				"INSERT INTO websub (uuid, topic, hub, secret) VALUES (?, ?, ?, ?)",
		// 				uuid,
		// 				subscription.topic,
		// 				subscription.hub,
		// 				secret,
		// 			)
		// 			.execute(&mut *conn)
		// 			.await?;
		// 		}
		//
		let resp = rb
			.send()
			.await
			.map_err(|e| Status::unavailable(e.to_string()))?;
		tracing::info!("Response: {}", resp.status());
		resp.error_for_status()
			.map_err(|e| Status::unavailable(e.to_string()))?;
		// 		Ok(new_sub)

		Ok(SubscribeResponse {}.into())
	}

	async fn unsubscribe(
		&self,
		request: Request<SubscribeRequest>,
	) -> Result<Response<SubscribeResponse>, Status> {
		let request = request.into_inner();
		let Some(sub) = request.sub else {
			return Err(Status::invalid_argument("missing sub"));
		};
		let Some(node) = request.node else { todo!() };

		let uuid = self.ws.lock().unwrap().get(&sub).cloned();
		if let Some(uuid) = uuid {
			self.subscriptions
				.lock()
				.unwrap()
				.get_mut(&uuid)
				.unwrap()
				.nodes
				.retain(|e| !e.eq(&node));
		}

		Ok(SubscribeResponse {}.into())
	}

	type ReceiveStream = tokio_stream::Iter<IntoIter<Result<WebSubEvent, Status>, 1>>;

	async fn receive(
		&self,
		request: Request<WebSubRequest>,
	) -> Result<Response<Self::ReceiveStream>, Status> {
		let stream = tokio_stream::iter([Ok(WebSubEvent { body: Vec::new() })]);

		Ok(stream.into())
	}
}
