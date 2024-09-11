use std::net::IpAddr;

use confique::Config;
use tokio::sync::OnceCell;
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

pub static CONFIG: OnceCell<AppConfig> = OnceCell::const_new();

async fn init() -> AppConfig {
	// dotenv().ok();

	Config::builder()
		.env()
		.file("rssflow.toml")
		.load()
		.expect("")
}

pub async fn config() -> &'static AppConfig {
	CONFIG.get_or_init(init).await
}
