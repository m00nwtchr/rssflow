use axum::{
	http::{header, HeaderValue},
	response::{IntoResponse, Response},
};
use rss::Channel;

mod api;
mod pipe;

pub use pipe::router as pipe;

// fn internal_error<E>(err: E) -> (StatusCode, String)
// where
// 	E: std::error::Error,
// {
// 	(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
// }

static APPLICATION_ATOM_XML: HeaderValue = HeaderValue::from_static("application/atom+xml");

struct Atom(Channel);
impl IntoResponse for Atom {
	fn into_response(self) -> Response {
		(
			[(header::CONTENT_TYPE, &APPLICATION_ATOM_XML)],
			self.0.to_string(),
		)
			.into_response()
	}
}
