use std::{
	net::{IpAddr, Ipv6Addr},
	sync::OnceLock,
};

use figment::{Figment, providers::Env};
use serde::Deserialize;
use url::Url;

use crate::ServiceInfo;

macro_rules! const_default {
	($name:ident, $ty:ty, $value:expr) => {
		pub const fn $name() -> $ty {
			$value
		}
	};
}

macro_rules! default {
	($name:ident, $ty:ty, $value:expr) => {
		pub fn $name() -> $ty {
			$value
		}
	};
}

const_default!(default_port, u16, 50051);
const_default!(default_http_port, u16, 3434);
const_default!(default_address, &'static str, "::");
const_default!(default_ip, IpAddr, IpAddr::V6(Ipv6Addr::UNSPECIFIED));

default!(
	default_registry_url,
	Url,
	Url::parse("http://rssflow:50051").expect("Hardcoded URL")
);
default!(
	default_redis_url,
	Url,
	Url::parse("redis://valkey/").expect("Hardcoded Redis URL")
);

const fn default_option<T>() -> Option<T> {
	None
}

#[derive(Deserialize)]
pub struct ServiceConfig {
	#[serde(default = "default_port")]
	pub grpc_port: u16,

	#[cfg(feature = "http")]
	#[serde(default = "default_http_port")]
	pub http_port: u16,

	#[serde(default = "default_ip")]
	pub address: IpAddr,

	#[serde(default = "default_registry_url")]
	pub registry_url: Url,

	#[cfg(feature = "redis")]
	#[serde(default = "default_redis_url")]
	pub redis_url: Url,

	#[cfg(feature = "db")]
	pub postgres_url: Url,

	#[serde(default = "default_option")]
	pub public_url: Option<Url>,

	#[serde(default = "default_option")]
	pub service_url: Option<Url>,
}

pub fn config(info: &ServiceInfo) -> &'static ServiceConfig {
	pub static CONFIG: OnceLock<ServiceConfig> = OnceLock::new();

	CONFIG.get_or_init(|| {
		let mut c: ServiceConfig = Figment::new()
			// .merge(Toml::file("rssflow.toml"))
			.merge(Env::raw())
			.extract()
			.unwrap();

		if c.service_url.is_none() {
			c.service_url = Some(
				format!("http://{}:{}", info.name.to_lowercase(), c.grpc_port)
					.parse()
					.unwrap(),
			);
		}
		c
	})
}
