use std::collections::HashMap;
use std::time::Duration;
use std::{ops::Deref, sync::Arc};

use crate::pipeline::feed::Feed;
use crate::pipeline::filter::{Field, Filter, Kind};
use crate::pipeline::Node;
use crate::route;
use axum::{routing::get, Router};
use rss::Channel;
use scraper::Selector;

#[allow(clippy::module_name_repetitions)]
pub struct AppStateInner {
	pub pipelines: HashMap<String, Box<dyn Node<Channel> + Send + Sync>>,
}

#[derive(Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct AppState(Arc<AppStateInner>);

impl Deref for AppState {
	type Target = AppStateInner;

	#[allow(clippy::explicit_deref_methods)]
	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

pub async fn app() -> Router {
	let mut p: HashMap<String, Box<dyn Node<Channel> + Send + Sync>> = HashMap::new();
	p.insert(
		"azaleaellis".to_string(),
		Box::new(
			Feed::new("https://www.azaleaellis.com/tag/pgts/feed".parse().unwrap())
				.filter(
					Field::Description,
					Kind::Contains("BELOW IS A SNEAK PEEK OF THIS CONTENT!".to_string()),
					true,
				)
				.retrieve(Selector::parse(".entry-content").unwrap())
				.cache(Duration::from_secs(60 * 60)),
		),
	);

	let a: Box<dyn Node<Channel>> = Box::new(Feed::new(
		"https://www.azaleaellis.com/tag/pgts/feed".parse().unwrap(),
	));

	let a: Box<dyn Node<Channel>> = Box::new(Filter::new(
		a,
		Field::Description,
		Kind::Contains("BELOW IS A SNEAK PEEK OF THIS CONTENT!".to_string()),
		true,
	));

	let state = AppState(Arc::new(AppStateInner { pipelines: p }));

	Router::new()
		// .nest("/api", route::api())
		.nest("/pipe", route::pipe())
		.route("/", get(|| async { "Hello, World!".to_string() }))
		.with_state(state)
	// .with_state(config)
}
