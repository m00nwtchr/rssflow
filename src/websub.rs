use anyhow::anyhow;
use reqwest::header::HeaderValue;

pub struct WebSubSpec {
	hub: String,
	feed: String,
}

// impl TryFrom<HeaderValue> for WebSubSpec {
// 	type Error = anyhow::Error;
//
// 	fn try_from(value: HeaderValue) -> Result<Self, Self::Error> {
// 		let str = value.to_str()?;
// 		for part in str.split(", ") {}
//
// 		Ok(())
// 	}
// }
