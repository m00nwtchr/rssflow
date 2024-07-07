use std::net::IpAddr;

use confique::Config;
use url::Url;

#[derive(Config)]
pub struct AppConfig {
	#[config(env = "PORT", default = 3434)]
	pub port: u16,

	#[config(env = "LISTEN_ADDRESS", default = "::")]
	pub address: IpAddr,

	#[config(env = "DATABASE_FILE", default = "rssflow.db")]
	pub database_file: String,

	#[config(env = "PUBLIC_URL")]
	pub public_url: Option<Url>,
}

impl AppConfig {
	pub fn load() -> anyhow::Result<AppConfig> {
		Ok(Config::builder().env().file("rssflow.toml").load()?)
	}
}
