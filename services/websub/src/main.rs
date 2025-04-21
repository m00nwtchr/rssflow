#[warn(clippy::pedantic)]
use std::array::IntoIter;
use std::{
	collections::HashMap,
	net::SocketAddr,
	ops::Deref,
	sync::{Arc, Mutex},
};

use proto::{
	registry::Node,
	websub::{
		SubscribeRequest, SubscribeResponse, WebSub, WebSubEvent, WebSubRequest,
		web_sub_service_server::{WebSubService, WebSubServiceServer},
	},
};
use tokio::net::TcpListener;
use tonic::{Request, Response, Status, transport::Server};
use url::Url;
use uuid::{NoContext, Timestamp, Uuid};

use crate::router::app;

pub mod router;
mod ws;

pub async fn websub_check(public_url: &Url) {
	// let resp = reqwest::get(public_url.join("/websub/check")?).await?;

	// resp.error_for_status()?;
	// Ok(())
}

#[derive(Debug, Default, Clone)]
pub struct Subscription {
	web_sub: WebSub,
	nodes: Vec<Node>,
}

#[derive(Debug, Default)]
pub struct WebSubInner {
	subscriptions: Mutex<HashMap<Uuid, Subscription>>,
	ws: Mutex<HashMap<WebSub, Uuid>>,
}

#[derive(Debug, Clone, Default)]
pub struct WebSubSVC(Arc<WebSubInner>);

impl Deref for WebSubSVC {
	type Target = WebSubInner;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

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
			.or_insert(Uuid::new_v7(Timestamp::now(NoContext))).clone();
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
		let resp = rb.send().await.map_err(|e| Status::unavailable(e.to_string()))?;
		tracing::info!("Response: {}", resp.status());
		resp.error_for_status().map_err(|e| Status::unavailable(e.to_string()))?;
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt::init();
	let (health_reporter, health_service) = tonic_health::server::health_reporter();
	health_reporter
		.set_serving::<WebSubServiceServer<WebSubSVC>>()
		.await;

	let port = std::env::var("GRPC_PORT")
		.ok()
		.and_then(|v| v.parse::<u16>().ok())
		.unwrap_or(50051);
	let http_port = std::env::var("HTTP_PORT")
		.ok()
		.and_then(|v| v.parse::<u16>().ok())
		.unwrap_or(80);

	let ip = "::".parse().unwrap();

	let addr = SocketAddr::new(ip, port);
	let http_addr = SocketAddr::new(ip, http_port);

	let svc = WebSubSVC::default();

	tracing::info!("WebSub service at: {}", addr);
	tracing::info!("WebSub endpoint at: {}", http_addr);
	let server = Server::builder()
		.add_service(health_service)
		.add_service(WebSubServiceServer::new(svc.clone()));

	let http_server = axum::serve(TcpListener::bind(http_addr).await?, app(svc));

	tokio::select! {
		res = server.serve(addr) => {
			if let Err(err) = res {
				tracing::error!("Failed to start gRPC server: {err}");
			}
		}
		res = http_server => {
			if let Err(err) = res {
				tracing::error!("Failed to start HTTP server: {err}");
			}
		}
	}

	Ok(())
}
