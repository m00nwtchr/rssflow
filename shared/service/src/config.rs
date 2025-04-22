use std::{net::IpAddr, sync::OnceLock};

use confique::Config;
use url::Url;

#[derive(Config)]
pub struct ServiceConfig {
	#[config(env = "GRPC_PORT", default = 50051)]
	pub port: u16,

	#[config(env = "HTTP_PORT", default = 3434)]
	pub http_port: u16,

	#[config(env = "ADDRESS", default = "::")]
	pub address: IpAddr,

	#[config(env = "REGISTRY_URL", default = "http://rssflow:50051")]
	pub registry_url: Url,

	#[config(env = "REDIS_URL", default = "redis://valkey/")]
	pub redis_url: Url,

	// #[config(env = "DATABASE_FILE", default = "rssflow.db")]
	// pub database_file: String,
	// #[config(env = "PUBLIC_URL")]
	// pub public_url: Option<Url>,
	#[config(env = "SERVICE_URL", default = "")]
	pub service_url: String,
}

pub fn config(service_name: &str) -> &'static ServiceConfig {
	pub static CONFIG: OnceLock<ServiceConfig> = OnceLock::new();

	CONFIG.get_or_init(|| {
		let mut c: ServiceConfig = Config::builder()
			.env()
			// .file("rssflow.toml")
			.load()
			.expect("");

		if c.service_url.is_empty() {
			c.service_url = format!("http://{}:{}", service_name.to_lowercase(), c.port);
		}
		c
	})
}
