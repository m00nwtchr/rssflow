use chrono::{DateTime, FixedOffset, Utc};
use prost_types::Timestamp;

use crate::{impl_name, node::Field};

tonic::include_proto!("rssflow.feed");

impl_name!(Feed, "rssflow.feed");
impl_name!(Entry, "rssflow.feed");
impl_name!(Content, "rssflow.feed");
impl_name!(Text, "rssflow.feed");
impl_name!(Link, "rssflow.feed");

impl_name!(StringValue, "rssflow.feed");

impl Entry {
	pub fn value(&self, field: Field) -> Option<&String> {
		match field {
			Field::Author => {
				// self.authors.first().map(|p| &p.name)
				None
			}
			Field::Summary => self.summary.as_ref().map(|t| &t.value),
			Field::Content => self.content.as_ref().and_then(|c| Some(&c.value)),
			Field::Title => Some(&self.title),
		}
	}

	pub fn value_mut(&mut self, field: Field) -> Option<&mut String> {
		match field {
			Field::Author => {
				// self.authors.first().map(|p| &p.name)
				None
			}
			Field::Summary => self.summary.as_mut().map(|t| &mut t.value),
			Field::Content => self.content.as_mut().and_then(|c| Some(&mut c.value)),
			Field::Title => Some(&mut self.title),
		}
	}
}

#[cfg(feature = "atom")]
mod atom {
	use atom_syndication::{
		Content as AtomContent, Entry as AtomEntry, Feed as AtomFeed, Link as AtomLink,
		Person as AtomPerson, Text as AtomText, TextType as AtomTextType,
	};

	use super::*;

	/// Converts an Atom feed into a protobuf `Feed`
	impl From<&AtomFeed> for Feed {
		fn from(feed: &AtomFeed) -> Self {
			Feed {
				title: feed.title.value.clone(),
				id: feed.id.clone(),
				updated: Some(to_timestamp(feed.updated())),
				authors: feed.authors.iter().map(Into::into).collect(),
				entries: feed.entries.iter().map(Into::into).collect(),
			}
		}
	}

	/// Converts an Atom entry into a protobuf `Entry`
	impl From<&AtomEntry> for Entry {
		fn from(entry: &AtomEntry) -> Self {
			Entry {
				title: entry.title.value.clone(),
				id: entry.id.clone(),
				updated: Some(to_timestamp(entry.updated())),
				authors: entry.authors.iter().map(Into::into).collect(),
				links: entry.links.iter().map(Into::into).collect(),
				summary: entry.summary.as_ref().map(Into::into),
				content: entry.content.as_ref().map(Into::into),
			}
		}
	}

	impl From<&AtomTextType> for TextType {
		fn from(value: &AtomTextType) -> Self {
			match value {
				AtomTextType::Text => TextType::Text,
				AtomTextType::Html => TextType::Html,
				AtomTextType::Xhtml => TextType::Xhtml,
			}
		}
	}

	impl From<&TextType> for AtomTextType {
		fn from(value: &TextType) -> Self {
			match value {
				TextType::Text => AtomTextType::Text,
				TextType::Html => AtomTextType::Html,
				TextType::Xhtml => AtomTextType::Xhtml,
			}
		}
	}

	impl From<&AtomText> for Text {
		fn from(text: &AtomText) -> Self {
			Text {
				value: text.value.clone(),
				r#type: TextType::from(&text.r#type) as i32,
			}
		}
	}

	impl From<Text> for AtomText {
		fn from(text: Text) -> Self {
			AtomText {
				value: text.value,
				r#type: AtomTextType::from(&TextType::try_from(text.r#type).unwrap()),
				..Default::default()
			}
		}
	}

	impl From<&AtomLink> for Link {
		fn from(link: &AtomLink) -> Self {
			Link {
				href: link.href.clone(),
				rel: link.rel.clone(),
			}
		}
	}

	impl From<Link> for AtomLink {
		fn from(link: Link) -> Self {
			AtomLink {
				href: link.href,
				rel: link.rel,
				..Default::default()
			}
		}
	}

	/// Converts an Atom content object into a protobuf `Content`
	impl From<&AtomContent> for Content {
		fn from(content: &AtomContent) -> Self {
			Content {
				value: content.value.clone().unwrap_or_default(),
				lang: content.lang.clone().unwrap_or_default(),
				content_type: content.content_type.clone().unwrap_or_default(),
			}
		}
	}
	impl From<Content> for AtomContent {
		fn from(value: Content) -> Self {
			AtomContent {
				value: if value.value.is_empty() {
					None
				} else {
					Some(value.value)
				},
				lang: if value.lang.is_empty() {
					None
				} else {
					Some(value.lang)
				},
				content_type: if value.content_type.is_empty() {
					None
				} else {
					Some(value.content_type)
				},
				..AtomContent::default()
			}
		}
	}

	/// Converts an Atom content object into a protobuf `Content`
	impl From<&AtomPerson> for Person {
		fn from(person: &AtomPerson) -> Self {
			Self {
				name: person.name.clone(),
				email: person.email.clone().unwrap_or_default(),
				uri: person.uri.clone().unwrap_or_default(),
			}
		}
	}
	impl From<Person> for AtomPerson {
		fn from(person: Person) -> Self {
			Self {
				name: person.name.clone(),
				email: if person.email.is_empty() {
					None
				} else {
					Some(person.email.clone())
				},
				uri: if person.uri.is_empty() {
					None
				} else {
					Some(person.uri.clone())
				},
			}
		}
	}

	impl From<Entry> for AtomEntry {
		fn from(value: Entry) -> Self {
			AtomEntry {
				title: value.title.into(),
				id: value.id,

				updated: value
					.updated
					.and_then(from_timestamp)
					.map(Into::into)
					.unwrap_or_default(),
				links: value.links.into_iter().map(Into::into).collect(),
				summary: value.summary.map(Into::into),
				content: value.content.map(Into::into),

				..AtomEntry::default()
			}
		}
	}

	impl From<Feed> for AtomFeed {
		fn from(value: Feed) -> Self {
			AtomFeed {
				title: value.title.into(),
				id: value.id,
				updated: value
					.updated
					.and_then(from_timestamp)
					.map(Into::into)
					.unwrap_or_default(),
				entries: value.entries.into_iter().map(Into::into).collect(),
				..Default::default()
			}
		}
	}
}

/// Converts a chrono `DateTime<FixedOffset>` into a protobuf `Timestamp`
fn to_timestamp(dt: &DateTime<FixedOffset>) -> Timestamp {
	Timestamp {
		seconds: dt.timestamp(),
		nanos: dt.timestamp_subsec_nanos() as i32,
	}
}

fn from_timestamp(t: Timestamp) -> Option<DateTime<Utc>> {
	DateTime::from_timestamp(t.seconds, t.nanos as u32)
}
