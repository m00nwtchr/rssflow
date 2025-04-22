use axum::{
	Router,
	body::Bytes,
	extract::{Path, Query, State},
	http::{HeaderMap, StatusCode},
	response::IntoResponse,
	routing::{get, post},
};
use proto::{node::ProcessRequest, websub::WebSubEvent};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
	Subscription, WebSubSVC,
	ws::{Verification, X_HUB_SIGNATURE},
};

async fn send_to_listeners(sub: &Subscription, body: Bytes) {
	let mut options = prost_types::Struct::default();
	options
		.fields
		.insert("url".to_string(), sub.web_sub.topic.clone().into());
	options
		.fields
		.insert("hub".to_string(), sub.web_sub.hub.clone().into());

	let payload: prost_types::Any = WebSubEvent {
		body: body.to_vec(),
	}
	.into();

	for node in &sub.nodes {
		let _ = node
			.process(ProcessRequest {
				payload: Some(payload.clone()),
				options: Some(options.clone()),
			})
			.await;
	}
}

pub async fn receive(
	Path(uuid): Path<Uuid>,
	// State(pool): State<SqlitePool>,
	State(state): State<WebSubSVC>,
	headers: HeaderMap,
	body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	// let mut conn = pool.acquire().await.map_err(internal_error)?;
	// let record = sqlx::query!("SELECT secret, topic, hub FROM websub WHERE uuid = ?", uuid)
	// 	.fetch_optional(&mut *conn)
	// 	.await
	// 	.map_err(internal_error)?;

	let subscription = state.subscriptions.lock().unwrap().get(&uuid).cloned();

	if let Some(subscription) = subscription {
		let signature = headers.get(X_HUB_SIGNATURE);

		// let Some(signature) = signature
		// 	.and_then(|v| v.to_str().ok())
		// 	.and_then(|s| XHubSignature::from_str(s).ok())
		// else {
		// 	return Ok(StatusCode::OK);
		// };

		// let verified = signature
		// 	.verify(record.secret.as_bytes(), &body)
		// 	.map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
		//
		// if verified {
		tracing::info!("Received WebSub push for `{}`", subscription.web_sub.topic);

		send_to_listeners(&subscription, body).await;

		// state
		// 	.web_sub_subscriber
		// 	.handle(
		// 		&state,
		// 		&WebSub {
		// 			topic: record.topic,
		// 			hub: record.hub,
		// 		},
		// 		body,
		// 	)
		// 	.await
		// 	.map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
		// }
	}

	Ok(StatusCode::OK)
}

// #[tracing::instrument(skip(pool, verification))]
pub async fn verify(
	Path(uuid): Path<Uuid>,
	// State(pool): State<SqlitePool>,
	Query(verification): Query<Verification>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
	// let mut conn = pool.acquire().await.map_err(internal_error)?;
	// if let Some(record) = sqlx::query!("SELECT subscribed, topic FROM websub WHERE uuid = ?", uuid)
	// 	.fetch_optional(&mut *conn)
	// 	.await
	// 	.map_err(internal_error)?
	// {
	// 	match verification {
	// 		Verification::Subscribe {
	// 			topic,
	// 			challenge,
	// 			lease_seconds,
	// 		} => {
	// 			let lease_end = Utc::now() + lease_seconds;
	// 			sqlx::query!(
	// 				"UPDATE websub SET lease_end = ? WHERE topic = ?",
	// 				lease_end,
	// 				record.topic
	// 			)
	// 			.execute(&mut *conn)
	// 			.await
	// 			.map_err(internal_error)?;
	//
	// 			if record.subscribed && topic.eq(&record.topic) {
	// 				Ok((StatusCode::OK, challenge))
	// 			} else {
	// 				Err((StatusCode::BAD_REQUEST, "Bad request".to_string()))
	// 			}
	// 		}
	// 		Verification::Unsubscribe { topic, challenge } => {
	// 			if !record.subscribed && topic.eq(&record.topic) {
	// 				tracing::info!("Unsubscribed from `{}`", record.topic);
	//
	// 				sqlx::query!("DELETE FROM websub WHERE topic = ?", record.topic)
	// 					.execute(&mut *conn)
	// 					.await
	// 					.map_err(internal_error)?;
	// 				Ok((StatusCode::OK, challenge))
	// 			} else {
	// 				Err((StatusCode::BAD_REQUEST, "Bad request".to_string()))
	// 			}
	// 		}
	// 	}
	// } else {
	// 	Err((StatusCode::NOT_FOUND, "Not found".to_string()))
	// }

	match verification {
		Verification::Subscribe { challenge, .. } | Verification::Unsubscribe { challenge, .. } => {
			Ok((StatusCode::OK, challenge))
		}
	}
}

pub fn app(state: WebSubSVC) -> Router {
	Router::new()
		.route("/websub/{uuid}", post(receive))
		.route("/websub/{uuid}", get(verify))
		.route("/websub/check", get(|| async {}))
		.layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()))
		.with_state(state)
}

fn internal_error<E>(err: E) -> (StatusCode, String)
where
	E: std::error::Error,
{
	(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
