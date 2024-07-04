use url::Url;

#[derive(Debug)]
pub struct WebSub {
	pub hub: Url,
	pub this: Url,
}
