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

    /// Resource scope patterns (JSON serialized)
    pub resource_scopes: Option<String>,

    /// Action scopes (JSON serialized)
    pub action_scopes: Option<String>,

    /// The date and time when the token will expire
    pub expired_at: Option<jiff::Timestamp>,
}

impl ApiToken {
    /// Create a new API token
    pub fn new(
        user_id: u64,
        name: String,
        hashed_token: HashedToken,
        resource_scopes: Option<Vec<ResourceScope>>,
        action_scopes: Option<Vec<ActionScope>>,
        expired_at: Option<jiff::Timestamp>,
    ) -> Self {
        let now = jiff::Timestamp::now();

        // Serialize scopes to JSON for storage
        let resource_scopes_json = resource_scopes.and_then(|scopes| serde_json::to_string(&scopes).ok());
        let action_scopes_json =
            action_scopes.and_then(|scopes| serde_json::to_string(&scopes).ok());

        Self {
            id: 0, // Will be auto-generated
            user_id,
            name,
            token: hashed_token.as_bytes().to_vec(),
            created_at: now,
            last_used_at: None,
            revoked: false,
            resource_scopes: resource_scopes_json,
            action_scopes: action_scopes_json,
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

    /// Parse resource scopes from JSON
    pub fn parse_resource_scopes(&self) -> Option<Vec<ResourceScope>> {
        self.resource_scopes
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
    }

    /// Parse action scopes from JSON
    pub fn parse_action_scopes(&self) -> Option<Vec<ActionScope>> {
        self.action_scopes
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
    }
}

/// Action scopes for API tokens
///
/// These scopes control which actions a token can perform.
/// This is a generic scope system suitable for any web application.
///
/// Examples: `read`, `create`, `update`, `delete`, `admin`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionScope {
    /// Can read resources
    Read,
    /// Can create resources
    Create,
    /// Can update resources
    Update,
    /// Can delete resources
    Delete,
    /// Full administrative access
    Admin,
}

impl ActionScope {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionScope::Read => "read",
            ActionScope::Create => "create",
            ActionScope::Update => "update",
            ActionScope::Delete => "delete",
            ActionScope::Admin => "admin",
        }
    }

    /// Parse from string
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "read" => Ok(ActionScope::Read),
            "create" => Ok(ActionScope::Create),
            "update" => Ok(ActionScope::Update),
            "delete" => Ok(ActionScope::Delete),
            "admin" => Ok(ActionScope::Admin),
            _ => Err(format!("Invalid action scope: {}", s)),
        }
    }

    /// Check if this scope grants permission for a given action
    ///
    /// Admin scope grants all permissions.
    /// Read scope grants read access.
    /// Create/Update/Delete scopes grant their respective permissions.
    pub fn can_perform(&self, action: &str) -> bool {
        match self {
            ActionScope::Admin => true,
            ActionScope::Read => action == "read",
            ActionScope::Create => action == "create",
            ActionScope::Update => action == "update",
            ActionScope::Delete => action == "delete",
        }
    }
}

/// Resource scope pattern for API tokens
///
/// Controls which resources a token can access. Supports wildcards.
/// This is a generic resource scoping system suitable for any web application.
///
/// Examples:
/// - `posts` - access to posts resource only
/// - `posts*` - access to posts and any sub-resources
/// - `*` - access to all resources
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ResourceScope {
    pattern: String,
}

impl ResourceScope {
    /// Create a new resource scope from a pattern
    pub fn new(pattern: String) -> Result<Self, String> {
        if Self::is_valid_pattern(&pattern) {
            Ok(ResourceScope { pattern })
        } else {
            Err("Invalid resource scope pattern".to_string())
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
        // Basic validation: alphanumeric, hyphens, underscores, colons (for resource:action format)
        name_without_wildcard
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ':')
    }

    /// Check if this scope matches a resource name
    pub fn matches(&self, resource_name: &str) -> bool {
        if self.pattern == "*" {
            return true;
        }

        match self.pattern.strip_suffix('*') {
            Some(prefix) => resource_name.starts_with(prefix),
            None => resource_name == self.pattern,
        }
    }

    /// Get the pattern string
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

impl<'de> Deserialize<'de> for ResourceScope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let pattern = String::deserialize(deserializer)?;
        ResourceScope::new(pattern).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<&str> for ResourceScope {
    type Error = String;

    fn try_from(pattern: &str) -> Result<Self, Self::Error> {
        ResourceScope::new(pattern.to_string())
    }
}

impl TryFrom<String> for ResourceScope {
    type Error = String;

    fn try_from(pattern: String) -> Result<Self, Self::Error> {
        ResourceScope::new(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_scope_serialization() {
        assert_eq!(
            serde_json::to_string(&ActionScope::Read).unwrap(),
            "\"read\""
        );
        assert_eq!(
            serde_json::to_string(&ActionScope::Create).unwrap(),
            "\"create\""
        );
        assert_eq!(
            serde_json::to_string(&ActionScope::Update).unwrap(),
            "\"update\""
        );
        assert_eq!(
            serde_json::to_string(&ActionScope::Delete).unwrap(),
            "\"delete\""
        );
        assert_eq!(
            serde_json::to_string(&ActionScope::Admin).unwrap(),
            "\"admin\""
        );
    }

    #[test]
    fn test_action_scope_deserialization() {
        let scope: ActionScope = serde_json::from_str("\"read\"").unwrap();
        assert_eq!(scope, ActionScope::Read);

        let scope: ActionScope = serde_json::from_str("\"create\"").unwrap();
        assert_eq!(scope, ActionScope::Create);

        let scope: ActionScope = serde_json::from_str("\"update\"").unwrap();
        assert_eq!(scope, ActionScope::Update);

        let scope: ActionScope = serde_json::from_str("\"delete\"").unwrap();
        assert_eq!(scope, ActionScope::Delete);

        let scope: ActionScope = serde_json::from_str("\"admin\"").unwrap();
        assert_eq!(scope, ActionScope::Admin);
    }

    #[test]
    fn test_action_scope_from_str() {
        assert_eq!(ActionScope::parse("read"), Ok(ActionScope::Read));
        assert_eq!(ActionScope::parse("create"), Ok(ActionScope::Create));
        assert_eq!(ActionScope::parse("update"), Ok(ActionScope::Update));
        assert_eq!(ActionScope::parse("delete"), Ok(ActionScope::Delete));
        assert_eq!(ActionScope::parse("admin"), Ok(ActionScope::Admin));
        assert!(ActionScope::parse("invalid").is_err());
    }

    #[test]
    fn test_action_scope_as_str() {
        assert_eq!(ActionScope::Read.as_str(), "read");
        assert_eq!(ActionScope::Create.as_str(), "create");
        assert_eq!(ActionScope::Update.as_str(), "update");
        assert_eq!(ActionScope::Delete.as_str(), "delete");
        assert_eq!(ActionScope::Admin.as_str(), "admin");
    }

    #[test]
    fn test_action_scope_can_perform() {
        assert!(ActionScope::Admin.can_perform("read"));
        assert!(ActionScope::Admin.can_perform("create"));
        assert!(ActionScope::Admin.can_perform("update"));
        assert!(ActionScope::Admin.can_perform("delete"));

        assert!(ActionScope::Read.can_perform("read"));
        assert!(!ActionScope::Read.can_perform("create"));
        assert!(!ActionScope::Read.can_perform("update"));
        assert!(!ActionScope::Read.can_perform("delete"));

        assert!(!ActionScope::Create.can_perform("read"));
        assert!(ActionScope::Create.can_perform("create"));
        assert!(!ActionScope::Create.can_perform("update"));
        assert!(!ActionScope::Create.can_perform("delete"));

        assert!(!ActionScope::Update.can_perform("read"));
        assert!(!ActionScope::Update.can_perform("create"));
        assert!(ActionScope::Update.can_perform("update"));
        assert!(!ActionScope::Update.can_perform("delete"));

        assert!(!ActionScope::Delete.can_perform("read"));
        assert!(!ActionScope::Delete.can_perform("create"));
        assert!(!ActionScope::Delete.can_perform("update"));
        assert!(ActionScope::Delete.can_perform("delete"));
    }

    #[test]
    fn test_resource_scope_validation() {
        assert!(ResourceScope::new("foo".to_string()).is_ok());
        assert!(ResourceScope::new("foo*".to_string()).is_ok());
        assert!(ResourceScope::new("*".to_string()).is_ok());
        assert!(ResourceScope::new("foo-bar".to_string()).is_ok());
        assert!(ResourceScope::new("foo_bar".to_string()).is_ok());
        assert!(ResourceScope::new("".to_string()).is_err());
        assert!(ResourceScope::new("foo#".to_string()).is_err());
        assert!(ResourceScope::new("foo@".to_string()).is_err());
    }

    #[test]
    fn test_resource_scope_matching() {
        let scope = |pattern: &str| ResourceScope::new(pattern.to_string()).unwrap();

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
    fn test_resource_scope_pattern() {
        let scope = ResourceScope::new("foo*".to_string()).unwrap();
        assert_eq!(scope.pattern(), "foo*");

        let scope = ResourceScope::new("bar".to_string()).unwrap();
        assert_eq!(scope.pattern(), "bar");
    }

    #[test]
    fn test_resource_scope_try_from_str() {
        assert!(ResourceScope::try_from("foo").is_ok());
        assert!(ResourceScope::try_from("foo*").is_ok());
        assert!(ResourceScope::try_from("").is_err());
    }

    #[test]
    fn test_resource_scope_try_from_string() {
        assert!(ResourceScope::try_from("foo".to_string()).is_ok());
        assert!(ResourceScope::try_from("foo*".to_string()).is_ok());
        assert!(ResourceScope::try_from("".to_string()).is_err());
    }

    #[test]
    fn test_resource_scope_serialization() {
        let scope = ResourceScope::new("foo*".to_string()).unwrap();
        let json = serde_json::to_string(&scope).unwrap();
        assert_eq!(json, "\"foo*\"");
    }

    #[test]
    fn test_resource_scope_deserialization() {
        let scope: ResourceScope = serde_json::from_str("\"foo*\"").unwrap();
        assert_eq!(scope.pattern(), "foo*");

        assert!(serde_json::from_str::<ResourceScope>("\"\"").is_err());
        assert!(serde_json::from_str::<ResourceScope>("\"foo#\"").is_err());
    }
}
