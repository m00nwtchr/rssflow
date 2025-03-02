// use std::ops::{Deref, DerefMut};
//
// use atom_syndication::Text;
// use serde::{Deserialize, Serialize};
//
// #[derive(Serialize, Deserialize)]
// pub struct Feed(feed_rs::model::Feed);
//
// impl From<feed_rs::model::Feed> for Feed {
// 	fn from(value: feed_rs::model::Feed) -> Self {
// 		Feed(value)
// 	}
// }
//
// impl Deref for Feed {
// 	type Target = feed_rs::model::Feed;
//
// 	fn deref(&self) -> &Self::Target {
// 		&self.0
// 	}
// }
//
// impl DerefMut for Feed {
// 	fn deref_mut(&mut self) -> &mut Self::Target {
// 		&mut self.0
// 	}
// }
//
// impl Into<atom_syndication::Feed> for Feed {
// 	fn into(self) -> atom_syndication::Feed {
// 		let mut out = atom_syndication::Feed::default();
// 		let feed = self.0;
//
// 		out.id = feed.id;
// 		out.title = Text { value: feed.t };
//
// 		out
// 	}
// }
//
// fn text(text: Option<feed_rs::model::Text>) -> Text {
// 	Text { ..Text::default() }
// }
