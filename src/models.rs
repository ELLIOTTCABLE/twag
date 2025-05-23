use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;
use url::{Host, Url};
use uuid::Uuid;

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
   #[error("Invalid Notion page ID '{input}': must be 32 hex characters")]
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
      let raw_id = match Url::parse(input) {
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

            // Extract the last path segment
            let last_segment = url
               .path_segments()
               .and_then(|mut segments| segments.next_back())
               .filter(|segment| !segment.is_empty())
               .ok_or_else(|| NotionPageIdError::MissingPageId {
                  input: input.to_string(),
               })?;

            // Extract ID from the segment (handles both bare IDs and page-name-prefixed IDs)
            Self::extract_id_from_segment(last_segment)?
         }
         Err(_) => {
            // Not a URL, treat as direct ID input
            Self::extract_id_from_segment(input)?
         }
      };

      Ok(raw_id)
   }

   fn extract_id_from_segment(segment: &str) -> Result<String, NotionPageIdError> {
      // First, try to parse as a UUID (handles both hyphenated and non-hyphenated)
      if let Ok(uuid) = Self::try_parse_as_uuid(segment) {
         return Ok(uuid.simple().to_string());
      }

      // Case 2: Contains a 32-character ID at the end (page-name-prefixed)
      // Look for a 32-character hex suffix
      let cleaned = segment.replace('-', "");
      if cleaned.len() > 32 {
         let suffix = &cleaned[cleaned.len() - 32..];
         if let Ok(uuid) = Self::try_parse_as_uuid(suffix) {
            return Ok(uuid.simple().to_string());
         }
      }

      // If we can't extract a valid ID, return an error
      Err(NotionPageIdError::InvalidId {
         input: segment.to_string(),
      })
   }

   fn try_parse_as_uuid(input: &str) -> Result<Uuid, uuid::Error> {
      // Check if the input is a valid UUID (32 hex characters), missing the hyphens
      let input = if input.len() == 32 && input.chars().all(|c| c.is_ascii_hexdigit()) {
         // Format as hyphenated UUID and parse
         format!(
            "{}-{}-{}-{}-{}",
            &input[0..8],
            &input[8..12],
            &input[12..16],
            &input[16..20],
            &input[20..32]
         )
      } else {
         input.to_string()
      };

      Uuid::try_parse(&input)
   }

   fn format_as_uuid(id: &str) -> Result<String, NotionPageIdError> {
      if id.len() != 32 {
         return Err(NotionPageIdError::InvalidId { input: id.to_string() });
      }

      // Parse the 32-character hex string as a UUID
      let uuid = Self::try_parse_as_uuid(id).map_err(|_| NotionPageIdError::InvalidId { input: id.to_string() })?;

      // Return as lowercase hyphenated UUID
      Ok(uuid.hyphenated().to_string().to_lowercase())
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

#[cfg(test)]
mod tests {
   use super::*;

   mod hex14_tests {
      use super::*;

      #[test]
      fn test_hex14_validation_and_conversion() {
         // Valid creation and case conversion
         let hex = Hex14::new("a1b2c3d4e5f678").unwrap();
         assert_eq!(hex.as_str(), "A1B2C3D4E5F678");

         // Length validation
         assert!(matches!(Hex14::new("A1B2C3"), Err(Hex14Error::InvalidLength(_))));
         assert!(matches!(
            Hex14::new("A1B2C3D4E5F67890"),
            Err(Hex14Error::InvalidLength(_))
         ));

         // Character validation
         assert!(matches!(
            Hex14::new("G1B2C3D4E5F678"),
            Err(Hex14Error::InvalidCharacter(_))
         ));
      }

      #[test]
      fn test_hex14_string_traits() {
         let hex: Hex14 = "A1B2C3D4E5F678".parse().unwrap();

         // Conversion traits
         let s: String = hex.clone().into();
         assert_eq!(s, "A1B2C3D4E5F678");

         // Equality with strings
         assert_eq!(hex, "A1B2C3D4E5F678");
         assert_eq!(hex, "A1B2C3D4E5F678".to_string());
      }
   }

   mod notion_page_id_tests {
      use super::*;

      #[test]
      fn test_notion_page_id_validation() {
         // Valid 32-character ID
         let id = NotionPageId::new("a1b2c3d4e5f67890abcdef1234567890").unwrap();
         assert_eq!(id.as_str(), "a1b2c3d4-e5f6-7890-abcd-ef1234567890");

         // Valid UUID with different case works
         let valid_uuid = "A1B2C3D4-E5F6-7890-ABCD-EF1234567890";
         let id = NotionPageId::new(valid_uuid).unwrap();
         assert_eq!(id.as_str(), "a1b2c3d4-e5f6-7890-abcd-ef1234567890");

         // Invalid length
         assert!(matches!(
            NotionPageId::new("a1b2c3d4e5f678"),
            Err(NotionPageIdError::InvalidId { .. })
         ));

         // Invalid characters
         assert!(matches!(
            NotionPageId::new("g1b2c3d4e5f67890abcdef1234567890"),
            Err(NotionPageIdError::InvalidId { .. })
         ));

         // Not a UUID at all
         assert!(matches!(
            NotionPageId::new("not-a-valid-uuid-at-all"),
            Err(NotionPageIdError::InvalidId { .. })
         ));

         // Too short hyphenated UUID
         assert!(matches!(
            NotionPageId::new("a1b2c3d4-e5f6-7890-abcd"),
            Err(NotionPageIdError::InvalidId { .. })
         ));
      }

      #[test]
      fn test_notion_page_id_url_parsing() {
         // Simple URL
         let url = "https://www.notion.so/a1b2c3d4e5f67890abcdef1234567890";
         let id = NotionPageId::new(url).unwrap();
         assert_eq!(id.as_str(), "a1b2c3d4-e5f6-7890-abcd-ef1234567890");

         // Complex URL with query params and fragment
         let url = "https://www.notion.so/workspace/page-a1b2c3d4e5f67890abcdef1234567890?v=abc123&foo=bar#section";
         let id = NotionPageId::new(url).unwrap();
         assert_eq!(id.as_str(), "a1b2c3d4-e5f6-7890-abcd-ef1234567890");

         // URL without valid ID
         let url = "https://www.notion.so/some-page";
         assert!(matches!(
            NotionPageId::new(url),
            Err(NotionPageIdError::InvalidId { .. })
         ));

         // Non-Notion domain
         assert!(matches!(
            NotionPageId::new("https://example.com/a1b2c3d4e5f67890abcdef1234567890"),
            Err(NotionPageIdError::InvalidFormat { .. })
         ));
      }

      #[test]
      fn test_notion_page_id_string_traits() {
         let id: NotionPageId = "a1b2c3d4e5f67890abcdef1234567890".parse().unwrap();

         // String conversion and equality
         let s: String = id.clone().into();
         assert_eq!(s, "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
         assert_eq!(id, "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
      }

      #[test]
      fn test_notion_page_id_raw_format() {
         let id = NotionPageId::new("a1b2c3d4e5f67890abcdef1234567890").unwrap();
         assert_eq!(id.as_raw(), "a1b2c3d4e5f67890abcdef1234567890");
      }

      #[test]
      fn test_notion_page_id_hash_and_clone() {
         use std::collections::HashMap;
         let id1 = NotionPageId::new("a1b2c3d4e5f67890abcdef1234567890").unwrap();
         let id2 = id1.clone();

         let mut map = HashMap::new();
         map.insert(id1, "value");
         assert_eq!(map.len(), 1);
         assert_eq!(id2.as_str(), "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
      }

      #[test]
      fn test_notion_page_id_edge_cases() {
         // UUID with mixed case
         let mixed_case_uuid = "A1B2c3d4-E5F6-7890-ABCD-ef1234567890";
         let id1 = NotionPageId::new(mixed_case_uuid).unwrap();
         assert_eq!(id1.as_str(), "a1b2c3d4-e5f6-7890-abcd-ef1234567890");

         // Invalid cases should still fail
         assert!(matches!(
            NotionPageId::new("https://www.notion.so/page-with-invalid-id-g1b2c3d4e5f67890abcdef1234567890"),
            Err(NotionPageIdError::InvalidId { .. })
         ));

         assert!(matches!(
            NotionPageId::new("https://www.notion.so/page-with-short-id-a1b2c3d4e5f67890"),
            Err(NotionPageIdError::InvalidId { .. })
         ));
      }

      #[test]
      fn test_notion_page_id_uuid_validation() {
         // Test that invalid UUIDs are properly rejected
         assert!(matches!(
            NotionPageId::new("not-a-valid-uuid-at-all"),
            Err(NotionPageIdError::InvalidId { .. })
         ));

         // Test that malformed UUIDs are rejected
         assert!(matches!(
            NotionPageId::new("a1b2c3d4-e5f6-7890-abcd-ef123456789g"), // 'g' is not hex
            Err(NotionPageIdError::InvalidId { .. })
         ));

         // Test that UUIDs with wrong length are rejected
         assert!(matches!(
            NotionPageId::new("a1b2c3d4-e5f6-7890-abcd"), // too short
            Err(NotionPageIdError::InvalidId { .. })
         ));
      }
   }
}
