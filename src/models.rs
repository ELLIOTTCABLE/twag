use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;

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
   type Target = String;

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
