//! API token model and scopes
//!
//! Provides API token management with scoped permissions.

use serde::{Deserialize, Serialize};
use toasty::Model;

use crate::util::HashedToken;

/// API token model for database storage
#[derive(Debug, Model, Serialize)]
pub struct ApiToken {
    /// Primary key - auto-generated
    #[key]
    #[auto]
    pub id: u64,

    /// Foreign key to the user
    pub user_id: u64,

    /// The name of the token
    pub name: String,

    /// Hashed token value (SHA-256)
    pub token: Vec<u8>,

    /// The date and time when the token was created
    pub created_at: jiff::Timestamp,

    /// The date and time when the token was last used
    pub last_used_at: Option<jiff::Timestamp>,

    /// Whether the token has been revoked
    pub revoked: bool,

    /// Crate scope patterns (JSON serialized)
    pub crate_scopes: Option<String>,

    /// Endpoint scopes (JSON serialized)
    pub endpoint_scopes: Option<String>,

    /// The date and time when the token will expire
    pub expired_at: Option<jiff::Timestamp>,
}

impl ApiToken {
    /// Create a new API token
    pub fn new(
        user_id: u64,
        name: String,
        hashed_token: HashedToken,
        crate_scopes: Option<Vec<CrateScope>>,
        endpoint_scopes: Option<Vec<EndpointScope>>,
        expired_at: Option<jiff::Timestamp>,
    ) -> Self {
        let now = jiff::Timestamp::now();
        
        // Serialize scopes to JSON for storage
        let crate_scopes_json = crate_scopes
            .and_then(|scopes| serde_json::to_string(&scopes).ok());
        let endpoint_scopes_json = endpoint_scopes
            .and_then(|scopes| serde_json::to_string(&scopes).ok());

        Self {
            id: 0, // Will be auto-generated
            user_id,
            name,
            token: hashed_token.as_bytes().to_vec(),
            created_at: now,
            last_used_at: None,
            revoked: false,
            crate_scopes: crate_scopes_json,
            endpoint_scopes: endpoint_scopes_json,
            expired_at,
        }
    }

    /// Check if the token is currently valid (not revoked and not expired)
    pub fn is_valid(&self) -> bool {
        if self.revoked {
            return false;
        }

        if let Some(expired_at) = self.expired_at {
            if expired_at < jiff::Timestamp::now() {
                return false;
            }
        }

        true
    }

    /// Parse crate scopes from JSON
    pub fn parse_crate_scopes(&self) -> Option<Vec<CrateScope>> {
        self.crate_scopes
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
    }

    /// Parse endpoint scopes from JSON
    pub fn parse_endpoint_scopes(&self) -> Option<Vec<EndpointScope>> {
        self.endpoint_scopes
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
    }
}

/// Endpoint scopes for API tokens
///
/// These scopes control which endpoints a token can access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndpointScope {
    /// Can publish new crates
    PublishNew,
    /// Can publish updates to existing crates
    PublishUpdate,
    /// Can yank/unyank crate versions
    Yank,
    /// Can change crate owners
    ChangeOwners,
}

impl EndpointScope {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            EndpointScope::PublishNew => "publish-new",
            EndpointScope::PublishUpdate => "publish-update",
            EndpointScope::Yank => "yank",
            EndpointScope::ChangeOwners => "change-owners",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "publish-new" => Ok(EndpointScope::PublishNew),
            "publish-update" => Ok(EndpointScope::PublishUpdate),
            "yank" => Ok(EndpointScope::Yank),
            "change-owners" => Ok(EndpointScope::ChangeOwners),
            _ => Err(format!("Invalid endpoint scope: {}", s)),
        }
    }
}

/// Crate scope pattern for API tokens
///
/// Controls which crates a token can access. Supports wildcards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CrateScope {
    pattern: String,
}

impl CrateScope {
    /// Create a new crate scope from a pattern
    pub fn new(pattern: String) -> Result<Self, String> {
        if Self::is_valid_pattern(&pattern) {
            Ok(CrateScope { pattern })
        } else {
            Err("Invalid crate scope pattern".to_string())
        }
    }

    /// Check if a pattern is valid
    fn is_valid_pattern(pattern: &str) -> bool {
        if pattern.is_empty() {
            return false;
        }

        if pattern == "*" {
            return true;
        }

        let name_without_wildcard = pattern.strip_suffix('*').unwrap_or(pattern);
        // Basic validation: alphanumeric, hyphens, underscores
        name_without_wildcard
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    }

    /// Check if this scope matches a crate name
    pub fn matches(&self, crate_name: &str) -> bool {
        if self.pattern == "*" {
            return true;
        }

        match self.pattern.strip_suffix('*') {
            Some(prefix) => crate_name.starts_with(prefix),
            None => crate_name == self.pattern,
        }
    }

    /// Get the pattern string
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

impl<'de> Deserialize<'de> for CrateScope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let pattern = String::deserialize(deserializer)?;
        CrateScope::new(pattern).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<&str> for CrateScope {
    type Error = String;

    fn try_from(pattern: &str) -> Result<Self, Self::Error> {
        CrateScope::new(pattern.to_string())
    }
}

impl TryFrom<String> for CrateScope {
    type Error = String;

    fn try_from(pattern: String) -> Result<Self, Self::Error> {
        CrateScope::new(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_scope_serialization() {
        assert_eq!(
            serde_json::to_string(&EndpointScope::ChangeOwners).unwrap(),
            "\"change-owners\""
        );
        assert_eq!(
            serde_json::to_string(&EndpointScope::PublishNew).unwrap(),
            "\"publish-new\""
        );
    }

    #[test]
    fn test_endpoint_scope_deserialization() {
        let scope: EndpointScope = serde_json::from_str("\"change-owners\"").unwrap();
        assert_eq!(scope, EndpointScope::ChangeOwners);
        
        let scope: EndpointScope = serde_json::from_str("\"publish-new\"").unwrap();
        assert_eq!(scope, EndpointScope::PublishNew);
    }

    #[test]
    fn test_endpoint_scope_from_str() {
        assert_eq!(EndpointScope::from_str("publish-new"), Ok(EndpointScope::PublishNew));
        assert_eq!(EndpointScope::from_str("publish-update"), Ok(EndpointScope::PublishUpdate));
        assert_eq!(EndpointScope::from_str("yank"), Ok(EndpointScope::Yank));
        assert_eq!(EndpointScope::from_str("change-owners"), Ok(EndpointScope::ChangeOwners));
        assert!(EndpointScope::from_str("invalid").is_err());
    }

    #[test]
    fn test_endpoint_scope_as_str() {
        assert_eq!(EndpointScope::PublishNew.as_str(), "publish-new");
        assert_eq!(EndpointScope::PublishUpdate.as_str(), "publish-update");
        assert_eq!(EndpointScope::Yank.as_str(), "yank");
        assert_eq!(EndpointScope::ChangeOwners.as_str(), "change-owners");
    }

    #[test]
    fn test_crate_scope_validation() {
        assert!(CrateScope::new("foo".to_string()).is_ok());
        assert!(CrateScope::new("foo*".to_string()).is_ok());
        assert!(CrateScope::new("*".to_string()).is_ok());
        assert!(CrateScope::new("foo-bar".to_string()).is_ok());
        assert!(CrateScope::new("foo_bar".to_string()).is_ok());
        assert!(CrateScope::new("".to_string()).is_err());
        assert!(CrateScope::new("foo#".to_string()).is_err());
        assert!(CrateScope::new("foo@".to_string()).is_err());
    }

    #[test]
    fn test_crate_scope_matching() {
        let scope = |pattern: &str| CrateScope::new(pattern.to_string()).unwrap();

        assert!(scope("foo").matches("foo"));
        assert!(!scope("foo").matches("bar"));
        assert!(scope("foo*").matches("foo"));
        assert!(scope("foo*").matches("foo-bar"));
        assert!(scope("foo*").matches("foo_bar"));
        assert!(!scope("foo*").matches("bar"));
        assert!(scope("*").matches("foo"));
        assert!(scope("*").matches("bar"));
        assert!(scope("*").matches("anything"));
    }

    #[test]
    fn test_crate_scope_pattern() {
        let scope = CrateScope::new("foo*".to_string()).unwrap();
        assert_eq!(scope.pattern(), "foo*");
        
        let scope = CrateScope::new("bar".to_string()).unwrap();
        assert_eq!(scope.pattern(), "bar");
    }

    #[test]
    fn test_crate_scope_try_from_str() {
        assert!(CrateScope::try_from("foo").is_ok());
        assert!(CrateScope::try_from("foo*").is_ok());
        assert!(CrateScope::try_from("").is_err());
    }

    #[test]
    fn test_crate_scope_try_from_string() {
        assert!(CrateScope::try_from("foo".to_string()).is_ok());
        assert!(CrateScope::try_from("foo*".to_string()).is_ok());
        assert!(CrateScope::try_from("".to_string()).is_err());
    }

    #[test]
    fn test_crate_scope_serialization() {
        let scope = CrateScope::new("foo*".to_string()).unwrap();
        let json = serde_json::to_string(&scope).unwrap();
        assert_eq!(json, "\"foo*\"");
    }

    #[test]
    fn test_crate_scope_deserialization() {
        let scope: CrateScope = serde_json::from_str("\"foo*\"").unwrap();
        assert_eq!(scope.pattern(), "foo*");
        
        assert!(serde_json::from_str::<CrateScope>("\"\"").is_err());
        assert!(serde_json::from_str::<CrateScope>("\"foo#\"").is_err());
    }
}
