use std::time::{Duration, SystemTime, UNIX_EPOCH};

use redis::{FromRedisValue, RedisResult, ToRedisArgs, Value};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

fn current_epoch_millis() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards")
		.as_millis() as u64
}

#[derive(Serialize, Deserialize)]
pub struct Cached<T> {
	time: u64, // epoch millis
	pub value: T,
}

impl<T> ToRedisArgs for Cached<T>
where
	T: Serialize,
{
	fn write_redis_args<W>(&self, out: &mut W)
	where
		W: ?Sized + redis::RedisWrite,
	{
		let json = serde_json::to_string(&self).expect("Failed to serialize to JSON");
		out.write_arg(json.as_bytes());
	}
}

impl<T> FromRedisValue for Cached<T>
where
	T: for<'de> Deserialize<'de>,
{
	fn from_redis_value(v: &Value) -> RedisResult<Self> {
		match v {
			Value::BulkString(json) => {
				let cached = serde_json::from_slice(json).map_err(|e| {
					redis::RedisError::from((
						redis::ErrorKind::TypeError,
						"JSON parse error",
						e.to_string(),
					))
				})?;

				Ok(cached)
			}
			_ => Err(redis::RedisError::from((
				redis::ErrorKind::TypeError,
				"Expected bulk string",
			))),
		}
	}
}

impl<T: Serialize + DeserializeOwned> Cached<T> {
	pub fn new(content: T) -> Self {
		Self {
			time: current_epoch_millis(),
			value: content,
		}
	}

	pub fn elapsed(&self) -> Duration {
		(UNIX_EPOCH + Duration::from_millis(self.time))
			.elapsed()
			.unwrap_or_default()
	}
}
