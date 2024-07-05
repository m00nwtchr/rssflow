use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSub {
	pub hub: Url,
	pub this: Url,
}
