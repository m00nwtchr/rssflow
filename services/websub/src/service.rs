use std::array::IntoIter;

use rssflow_service::proto::websub::{
	SubscribeRequest, SubscribeResponse, WebSubEvent, WebSubRequest,
	web_sub_service_server::WebSubService,
};
use runesys::{Service, config::config};
use sqlx::PgPool;
use tonic::{Request, Response, Status};
use tracing::instrument;
use uuid::Uuid;

use crate::{WebSubSVC, ws::generate_hmac_secret};

#[tonic::async_trait]
impl WebSubService for WebSubSVC {
	#[instrument(skip_all)]
	async fn subscribe(
		&self,
		request: Request<SubscribeRequest>,
	) -> Result<Response<SubscribeResponse>, Status> {
		runesys::telemetry::propagation::accept_trace(&request);
		let mut conn = request
			.extensions()
			.get::<PgPool>()
			.expect("pg pool")
			.acquire()
			.await
			.expect("");

		let request = request.into_inner();
		let Some(sub) = request.sub else {
			return Err(Status::invalid_argument("missing sub"));
		};
		let Some(node) = request.node else { todo!() };

		let public_url = config(&WebSubSVC::INFO).public_url.as_ref();
		let Some(public_url) = public_url else {
			return Err(Status::internal("Public url unset"));
		};

		let record = sqlx::query!(
			r#"SELECT uuid, secret, lease_end FROM subscription WHERE topic = $1 AND hub = $2"#,
			sub.topic,
			sub.hub
		)
		.fetch_optional(&mut *conn)
		.await
		.map_err(|e| Status::internal(e.to_string()))?;
		let new_subscription = record.is_none();

		let secret: &String = if let Some(r) = &record {
			&r.secret
		} else {
			&generate_hmac_secret()
		};

		let uuid: &Uuid = if let Some(record) = &record {
			&record.uuid
		} else {
			tracing::info!("Subscribed to `{}` at `{}`", sub.topic, sub.hub);

			&sqlx::query_scalar!(
				"INSERT INTO subscription (topic, hub, secret) VALUES ($1, $2, $3) RETURNING uuid",
				sub.topic,
				sub.hub,
				secret,
			)
			.fetch_one(&mut *conn)
			.await
			.map_err(|e| Status::internal(e.to_string()))?
		};

		let callback = format!("{public_url}websub/{uuid}");
		let rb = reqwest::Client::new().post(&sub.hub).form(&[
			("hub.callback", callback.as_str()),
			("hub.mode", "subscribe"),
			("hub.topic", &sub.topic),
			("hub.secret", secret.as_str()),
		]);

		let resp = rb
			.send()
			.await
			.map_err(|e| Status::unavailable(e.to_string()))?;
		tracing::info!("Response: {}", resp.status());
		resp.error_for_status()
			.map_err(|e| Status::unavailable(e.to_string()))?;

		Ok(SubscribeResponse { new_subscription }.into())
	}

	#[instrument(skip_all)]
	async fn unsubscribe(
		&self,
		request: Request<SubscribeRequest>,
	) -> Result<Response<SubscribeResponse>, Status> {
		runesys::telemetry::propagation::accept_trace(&request);
		let request = request.into_inner();
		let Some(sub) = request.sub else {
			return Err(Status::invalid_argument("missing sub"));
		};
		let Some(node) = request.node else { todo!() };

		let uuid = self.ws.lock().unwrap().get(&sub).copied();
		if let Some(uuid) = uuid {
			self.subscriptions
				.lock()
				.unwrap()
				.get_mut(&uuid)
				.unwrap()
				.nodes
				.retain(|e| !e.eq(&node));
		}

		Ok(SubscribeResponse {
			new_subscription: false,
		}
		.into())
	}

	type ReceiveStream = tokio_stream::Iter<IntoIter<Result<WebSubEvent, Status>, 1>>;

	#[instrument(skip_all)]
	async fn receive(
		&self,
		request: Request<WebSubRequest>,
	) -> Result<Response<Self::ReceiveStream>, Status> {
		runesys::telemetry::propagation::accept_trace(&request);
		let stream = tokio_stream::iter([Ok(WebSubEvent { body: Vec::new() })]);

		Ok(stream.into())
	}
}
