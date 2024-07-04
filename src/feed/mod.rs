// use anyhow::anyhow;
// use atom_syndication::{
// 	CategoryBuilder, Entry, EntryBuilder, FeedBuilder, FixedDateTime, LinkBuilder, PersonBuilder,
// 	Text,
// };
// use rss::Channel;
// use serde::{Deserialize, Serialize};
// use std::ops::{Deref, DerefMut};
// 
// #[derive(Serialize, Deserialize)]
// // pub struct Feed {
// // 	pub title: Text,
// // 	pub id: String,
// // }
// pub struct Feed(atom_syndication::Feed);
// 
// impl TryFrom<Channel> for Feed {
// 	type Error = anyhow::Error;
// 
// 	fn try_from(value: Channel) -> Result<Self, Self::Error> {
// 		let mut builder = FeedBuilder::default();
// 		builder
// 			.title(value.title)
// 			.link(LinkBuilder::default().href(value.link).build())
// 			.subtitle(Text::plain(value.description));
// 
// 		if let Some(language) = value.language {
// 			builder.lang(Some(language));
// 		}
// 
// 		if let Some(atom) = value.atom_ext {
// 			for link in atom.links {
// 				builder.link(link);
// 			}
// 		}
// 
// 		if let Some(last_build_date) = value.last_build_date {
// 			builder.updated(
// 				FixedDateTime::parse_from_rfc2822(&last_build_date).map_err(|e| anyhow!(e))?,
// 			);
// 		}
// 
// 		for item in value.items {
// 			let mut entry = EntryBuilder::default();
// 
// 			if let Some(title) = item.title {
// 				entry.title(title);
// 			}
// 
// 			if let Some(description) = item.description {
// 				entry.summary(Text::plain(description));
// 			}
// 
// 			if let Some(link) = item.link {
// 				entry.link(LinkBuilder::default().href(link).build());
// 			}
// 
// 			if let Some(pub_date) = item.pub_date {
// 				let published =
// 					FixedDateTime::parse_from_rfc2822(&pub_date).map_err(|e| anyhow!(e))?;
// 				entry.published(published);
// 			}
// 
// 			for category in &value.categories {
// 				entry.category(
// 					CategoryBuilder::default()
// 						.label(category.name.clone())
// 						.build(),
// 				);
// 			}
// 
// 			entry.i
// 
// 			if let Some(dc) = item.dublin_core_ext {
// 				for creator in dc.creators {
// 					entry.author(PersonBuilder::default().name(creator).build());
// 				}
// 			}
// 		}
// 
// 		Ok(Feed(builder.build()))
// 	}
// }
// 
// impl From<atom_syndication::Feed> for Feed {
// 	fn from(value: atom_syndication::Feed) -> Self {
// 		// Feed { title: value.title }
// 		Feed(value)
// 	}
// }
// 
// impl Deref for Feed {
// 	type Target = atom_syndication::Feed;
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
// //
// // impl From<Feed> for atom_syndication::Feed {
// // 	fn from(value: Feed) -> Self {
// // 		atom_syndication::FeedBuilder::default()
// // 			.title(value.title)
// // 			.build()
// // 	}
// // }
