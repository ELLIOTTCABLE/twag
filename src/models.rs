use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;
use url::{Host, Url};

/// A type representing a fixed-length, 14-character hexadecimal string.
#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[sqlx(type_name = "hex_14", transparent)]
pub struct Hex14(String);

#[derive(Debug, thiserror::Error)]
pub enum Hex14Error {
   #[error("Invalid length: expected 14 characters, got {0}")]
   InvalidLength(usize),
   #[error("Invalid character: expected hex digit, found '{0}'")]
   InvalidCharacter(char),
}

impl Hex14 {
   pub fn new(s: impl Into<String>) -> Result<Self, Hex14Error> {
      let s = s.into();
      if s.len() != 14 {
         return Err(Hex14Error::InvalidLength(s.len()));
      }
      if s.chars().any(|c| !c.is_ascii_hexdigit()) {
         return Err(Hex14Error::InvalidCharacter(
            s.chars().find(|&c| !c.is_ascii_hexdigit()).unwrap(),
         ));
      }
      Ok(Hex14(s.to_uppercase()))
   }

   pub fn as_str(&self) -> &str { &self.0 }
}

impl Deref for Hex14 {
   type Target = str;

   fn deref(&self) -> &Self::Target { &self.0 }
}

impl FromStr for Hex14 {
   type Err = Hex14Error;

   fn from_str(s: &str) -> Result<Self, Self::Err> { Hex14::new(s) }
}

impl From<Hex14> for String {
   fn from(hex: Hex14) -> Self { hex.0 }
}

impl AsRef<str> for Hex14 {
   fn as_ref(&self) -> &str { &self.0 }
}

impl<'a> TryFrom<&'a str> for Hex14 {
   type Error = Hex14Error;

   fn try_from(s: &'a str) -> Result<Self, Self::Error> { Self::new(s) }
}

impl std::fmt::Display for Hex14 {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) }
}

impl Hash for Hex14 {
   fn hash<H: Hasher>(&self, state: &mut H) { self.0.hash(state); }
}

impl PartialEq<String> for Hex14 {
   fn eq(&self, other: &String) -> bool { self.0 == *other }
}

impl PartialEq<&str> for Hex14 {
   fn eq(&self, other: &&str) -> bool { self.0 == *other }
}

impl PartialEq<Hex14> for String {
   fn eq(&self, other: &Hex14) -> bool { *self == other.0 }
}

impl PartialEq<Hex14> for &str {
   fn eq(&self, other: &Hex14) -> bool { *self == other.0 }
}

impl PartialEq<Hex14> for str {
   fn eq(&self, other: &Hex14) -> bool { self == other.0 }
}

impl std::borrow::Borrow<str> for Hex14 {
   fn borrow(&self) -> &str { &self.0 }
}

/// A type representing a Notion page/database ID with validation and parsing from URLs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct NotionPageId(String);

#[derive(Debug, thiserror::Error)]
pub enum NotionPageIdError {
   #[error("Invalid Notion page ID '{input}': must be either a bare ID or a Notion URL")]
   InvalidFormat { input: String },
   #[error("Invalid Notion URL '{input}': missing page ID in path")]
   MissingPageId { input: String },
   #[error("Invalid Notion page ID '{input}': must be 32 alphanumeric characters")]
   InvalidId { input: String },
}

impl NotionPageId {
   /// Parse a Notion page ID from either a bare ID or a Notion URL
   pub fn new(input: impl Into<String>) -> Result<Self, NotionPageIdError> {
      let input = input.into();
      let page_id = Self::parse_page_id_from_possible_url(&input)?;

      // Format as UUID-style string (8-4-4-4-12)
      let formatted_id = Self::format_as_uuid(&page_id)?;
      Ok(NotionPageId(formatted_id))
   }

   fn parse_page_id_from_possible_url(input: &str) -> Result<String, NotionPageIdError> {
      let database_id = match Url::parse(input) {
         Ok(url) => {
            // Validate the URL is from Notion
            match url.host() {
               Some(Host::Domain("www.notion.so")) => (),
               _ => {
                  return Err(NotionPageIdError::InvalidFormat {
                     input: input.to_string(),
                  });
               }
            }

            // Extract the database ID from the URL path
            url.path_segments()
               .and_then(|mut segments| segments.next_back())
               .filter(|segment| !segment.is_empty())
               .ok_or_else(|| NotionPageIdError::MissingPageId {
                  input: input.to_string(),
               })?
               .to_string()
         }
         Err(_) => input.to_string(),
      };

      if database_id.len() != 32 || !database_id.chars().all(|c| c.is_ascii_alphanumeric()) {
         return Err(NotionPageIdError::InvalidId {
            input: input.to_string(),
         });
      }

      Ok(database_id)
   }

   fn format_as_uuid(id: &str) -> Result<String, NotionPageIdError> {
      if id.len() != 32 {
         return Err(NotionPageIdError::InvalidId { input: id.to_string() });
      }

      // Format as UUID: 8-4-4-4-12
      let formatted = format!(
         "{}-{}-{}-{}-{}",
         &id[0..8],
         &id[8..12],
         &id[12..16],
         &id[16..20],
         &id[20..32]
      );

      Ok(formatted)
   }

   pub fn as_str(&self) -> &str { &self.0 }

   pub fn as_raw(&self) -> String { self.0.replace('-', "") }
}

impl Deref for NotionPageId {
   type Target = str;

   fn deref(&self) -> &Self::Target { &self.0 }
}

impl PartialEq<String> for NotionPageId {
   fn eq(&self, other: &String) -> bool { self.0 == *other }
}

impl PartialEq<&str> for NotionPageId {
   fn eq(&self, other: &&str) -> bool { self.0 == *other }
}

impl PartialEq<NotionPageId> for String {
   fn eq(&self, other: &NotionPageId) -> bool { *self == other.0 }
}

impl PartialEq<NotionPageId> for &str {
   fn eq(&self, other: &NotionPageId) -> bool { *self == other.0 }
}

impl PartialEq<NotionPageId> for str {
   fn eq(&self, other: &NotionPageId) -> bool { self == other.0 }
}

impl From<NotionPageId> for String {
   fn from(id: NotionPageId) -> Self { id.0 }
}

impl std::borrow::Borrow<str> for NotionPageId {
   fn borrow(&self) -> &str { &self.0 }
}

impl FromStr for NotionPageId {
   type Err = NotionPageIdError;

   fn from_str(s: &str) -> Result<Self, Self::Err> { NotionPageId::new(s) }
}

impl std::fmt::Display for NotionPageId {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) }
}

impl AsRef<str> for NotionPageId {
   fn as_ref(&self) -> &str { &self.0 }
}

#[allow(dead_code)]
#[derive(sqlx::FromRow)]
pub struct TwagTag {
   pub id: Hex14,
   pub target_url: String,
   pub created_at: DateTime<Utc>,
   pub updated_at: DateTime<Utc>,
   pub last_accessed: Option<DateTime<Utc>>,
   pub access_count: i32,
   pub last_seen_tap_count: Option<i32>,
}
