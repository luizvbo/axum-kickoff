//! Token utilities for API token generation and hashing
//!
//! Provides secure token generation and hashing for API tokens.

use rand::distributions::Alphanumeric;
use rand::Rng;
use secrecy::{ExposeSecret, SecretSlice, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TOKEN_LENGTH: usize = 32;

/// Token prefix for axum-kickoff API tokens
///
/// NEVER CHANGE THE PREFIX OF EXISTING TOKENS!!! Doing so will implicitly
/// revoke all the tokens, disrupting production users.
const TOKEN_PREFIX: &str = "ako";

/// An error indicating that a token is invalid.
///
/// This error is returned when a token is not prefixed with a
/// known axum-kickoff-specific prefix.
#[derive(Debug, Error)]
#[error("invalid token format")]
pub struct InvalidTokenError;

/// Hashed token for database storage
#[derive(Clone)]
pub struct HashedToken(SecretSlice<u8>);

impl HashedToken {
    /// Parse a plaintext token and return its hashed version
    ///
    /// This will both reject tokens without a prefix and tokens of the wrong kind.
    pub fn parse(plaintext: &str) -> Result<Self, InvalidTokenError> {
        if !plaintext.starts_with(TOKEN_PREFIX) {
            return Err(InvalidTokenError);
        }

        let sha256 = Self::hash(plaintext).into();
        Ok(Self(sha256))
    }

    /// Hash a plaintext token
    pub fn hash(plaintext: &str) -> Vec<u8> {
        Sha256::digest(plaintext.as_bytes()).as_slice().to_vec()
    }

    /// Get the underlying bytes (for database storage)
    pub fn as_bytes(&self) -> &[u8] {
        self.0.expose_secret()
    }
}

impl std::fmt::Debug for HashedToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HashedToken")
    }
}

/// Plain token for API token generation
#[derive(Debug)]
pub struct PlainToken(SecretString);

impl PlainToken {
    /// Generate a new random API token
    pub fn generate() -> Self {
        let plaintext = format!(
            "{}{}",
            TOKEN_PREFIX,
            generate_secure_alphanumeric_string(TOKEN_LENGTH)
        )
        .into();

        Self(plaintext)
    }

    /// Hash the token for database storage
    pub fn hashed(&self) -> HashedToken {
        let sha256 = HashedToken::hash(self.expose_secret()).into();
        HashedToken(sha256)
    }
}

impl ExposeSecret<str> for PlainToken {
    fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

/// Generate a cryptographically secure random alphanumeric string
fn generate_secure_alphanumeric_string(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len).map(|_| rng.sample(Alphanumeric) as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generated_and_parse() {
        let token = PlainToken::generate();
        assert!(token.expose_secret().starts_with(TOKEN_PREFIX));
        assert_eq!(
            token.hashed().as_bytes(),
            Sha256::digest(token.expose_secret().as_bytes()).as_slice()
        );

        let parsed =
            HashedToken::parse(token.expose_secret()).expect("failed to parse back the token");
        assert_eq!(parsed.as_bytes(), token.hashed().as_bytes());
    }

    #[test]
    fn test_parse_no_kind() {
        assert!(HashedToken::parse("nokind").is_err());
    }

    #[test]
    fn test_token_length() {
        let token = PlainToken::generate();
        let expected_length = TOKEN_PREFIX.len() + TOKEN_LENGTH;
        assert_eq!(token.expose_secret().len(), expected_length);
    }

    #[test]
    fn test_hash_consistency() {
        let token = PlainToken::generate();
        let plaintext = token.expose_secret();

        let hash1 = token.hashed();
        let hash2 = HashedToken::hash(plaintext);

        assert_eq!(hash1.as_bytes(), hash2.as_slice());
    }

    #[test]
    fn test_different_tokens_different_hashes() {
        let token1 = PlainToken::generate();
        let token2 = PlainToken::generate();

        assert_ne!(token1.expose_secret(), token2.expose_secret());
        assert_ne!(token1.hashed().as_bytes(), token2.hashed().as_bytes());
    }

    #[test]
    fn test_invalid_token_format() {
        // No prefix
        assert!(HashedToken::parse("randomstring").is_err());

        // Wrong prefix
        assert!(HashedToken::parse("crs_randomstring").is_err());

        // Empty string
        assert!(HashedToken::parse("").is_err());
    }

    #[test]
    fn test_valid_token_format() {
        let token = PlainToken::generate();
        assert!(HashedToken::parse(token.expose_secret()).is_ok());
    }

    #[test]
    fn test_hashed_token_debug() {
        let token = PlainToken::generate();
        let hashed = token.hashed();
        let debug_str = format!("{:?}", hashed);
        assert_eq!(debug_str, "HashedToken");
    }

    #[test]
    fn test_plain_token_debug() {
        let token = PlainToken::generate();
        let debug_str = format!("{:?}", token);
        // Debug should not expose the secret
        assert!(!debug_str.contains(token.expose_secret()));
    }

    #[test]
    fn test_hashed_token_clone() {
        let token = PlainToken::generate();
        let hashed1 = token.hashed();
        let hashed2 = hashed1.clone();
        assert_eq!(hashed1.as_bytes(), hashed2.as_bytes());
    }

    #[test]
    fn test_hash_same_plaintext() {
        let plaintext = "ako_test123456789012345678901234";
        let hash1 = HashedToken::hash(plaintext);
        let hash2 = HashedToken::hash(plaintext);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_parse_valid_token_with_prefix() {
        let valid_token = "ako_abcdefghijklmnopqrstuvwxyz123456";
        let result = HashedToken::parse(valid_token);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_token_case_sensitive() {
        // Prefix should be case-sensitive
        let uppercase_prefix = "AKO_test123456789012345678901234";
        assert!(HashedToken::parse(uppercase_prefix).is_err());
    }

    #[test]
    fn test_token_alphanumeric() {
        let token = PlainToken::generate();
        let token_str = token.expose_secret();
        // After prefix, should be alphanumeric
        let suffix = &token_str[TOKEN_PREFIX.len()..];
        assert!(suffix.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_hashed_token_as_bytes_length() {
        let token = PlainToken::generate();
        let hashed = token.hashed();
        // SHA256 produces 32 bytes
        assert_eq!(hashed.as_bytes().len(), 32);
    }

    #[test]
    fn test_multiple_generations_unique() {
        let mut tokens = std::collections::HashSet::new();
        for _ in 0..100 {
            let token = PlainToken::generate();
            let plaintext = token.expose_secret().to_string();
            assert!(!tokens.contains(&plaintext), "Generated duplicate token");
            tokens.insert(plaintext);
        }
    }

    #[test]
    fn test_plain_token_expose_secret() {
        let token = PlainToken::generate();
        let secret = token.expose_secret();
        assert!(secret.starts_with(TOKEN_PREFIX));
        assert_eq!(secret.len(), TOKEN_PREFIX.len() + TOKEN_LENGTH);
    }
}
