use std::sync::OnceLock;

use figment::{
	Figment, Metadata, Profile, Provider,
	value::{Dict, Map},
};
use runesys::Service;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceConfig {
	pub registry_url: Url,
	pub public_url: Option<Url>,
	pub service_url: Option<Url>,
}

impl Default for ServiceConfig {
	fn default() -> Self {
		ServiceConfig {
			registry_url: Url::parse("http://rssflow:50051").expect("Hardcoded URL"),
			public_url: None,
			service_url: None,
		}
	}
}

impl ServiceConfig {
	// Allow the configuration to be extracted from any `Provider`.
	fn from<T: Provider>(provider: T) -> Result<Self, figment::Error> {
		Figment::from(provider).extract()
	}

	// Provide a default provider, a `Figment`.
	// fn figment() -> Figment {
	// 	use figment::providers::Env;
	//
	// 	Figment::from(Self::default()).merge(Env::raw())
	// }
}

impl Provider for ServiceConfig {
	fn metadata(&self) -> Metadata {
		Metadata::named("rssflow Config")
	}

	fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
		figment::providers::Serialized::defaults(Self::default()).data()
	}
}

pub fn config<S: Service>() -> &'static ServiceConfig {
	pub static CONFIG: OnceLock<ServiceConfig> = OnceLock::new();

	CONFIG.get_or_init(|| {
		let mut c: ServiceConfig = Figment::from(ServiceConfig::default())
			.merge(runesys::config::FIGMENT.clone())
			.extract()
			.unwrap();

		if c.service_url.is_none() {
			c.service_url = Some(
				format!(
					"http://{}:{}",
					S::INFO.name.to_lowercase(),
					runesys::config::config().grpc_port
				)
				.parse()
				.unwrap(),
			);
		}
		c
	})
}
