//! GitHub token encryption utilities
//!
//! Provides AES-256-GCM encryption for GitHub OAuth tokens.

use aes_gcm::aead::{Aead, AeadCore, OsRng};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use anyhow::{Context, Result};

/// A struct that encapsulates GitHub token encryption and decryption
/// using AES-256-GCM.
pub struct GitHubTokenEncryption {
    cipher: Aes256Gcm,
}

impl GitHubTokenEncryption {
    /// Creates a new [GitHubTokenEncryption] instance with the provided cipher
    pub fn new(cipher: Aes256Gcm) -> Self {
        Self { cipher }
    }

    /// Creates a new [GitHubTokenEncryption] instance with a cipher for testing
    /// purposes.
    #[cfg(any(test, debug_assertions))]
    pub fn for_testing() -> Self {
        let test_key = b"test_key_32_bytes_long_for_tests";
        Self::new(Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(test_key)))
    }

    /// Creates a new [GitHubTokenEncryption] instance from the environment
    ///
    /// Reads the `GITHUB_TOKEN_ENCRYPTION_KEY` environment variable, which
    /// should be a 64-character hex string (32 bytes when decoded).
    pub fn from_environment() -> Result<Self> {
        let gh_token_key = std::env::var("GITHUB_TOKEN_ENCRYPTION_KEY")
            .context("GITHUB_TOKEN_ENCRYPTION_KEY environment variable not set")?;

        if gh_token_key.len() != 64 {
            anyhow::bail!("GITHUB_TOKEN_ENCRYPTION_KEY must be exactly 64 hex characters");
        }

        let gh_token_key = hex::decode(gh_token_key.as_bytes())
            .context("GITHUB_TOKEN_ENCRYPTION_KEY must be exactly 64 hex characters")?;

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&gh_token_key));

        Ok(Self::new(cipher))
    }

    /// Encrypts a GitHub access token using AES-256-GCM
    ///
    /// The encrypted data format is: `[12-byte nonce][encrypted data]`
    /// The nonce is randomly generated for each encryption to ensure uniqueness.
    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to encrypt GitHub token: {}", e))?;

        let mut encrypted = nonce.to_vec();
        encrypted.extend(ciphertext);
        Ok(encrypted)
    }

    /// Decrypts a GitHub access token using AES-256-GCM
    ///
    /// Expects the encrypted data format: `[12-byte nonce][encrypted data]`
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<String> {
        if encrypted.len() < 12 {
            anyhow::bail!("Encrypted data is too short");
        }

        let (nonce, ciphertext) = encrypted.split_at(12);
        let nonce = Nonce::from_slice(nonce);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Failed to decrypt GitHub token: {}", e))?;

        String::from_utf8(plaintext)
            .map_err(|e| anyhow::anyhow!("Decrypted data is not valid UTF-8: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_for_testing() {
        let _encryption = GitHubTokenEncryption::for_testing();
        // Just verify it creates successfully
        assert!(true);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let encryption = GitHubTokenEncryption::for_testing();
        let plaintext = "my_secret_github_token_12345";

        let encrypted = encryption.encrypt(plaintext).expect("Encryption failed");
        let decrypted = encryption.decrypt(&encrypted).expect("Decryption failed");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertext() {
        let encryption = GitHubTokenEncryption::for_testing();
        let plaintext = "my_secret_github_token_12345";

        let encrypted1 = encryption.encrypt(plaintext).expect("Encryption failed");
        let encrypted2 = encryption.encrypt(plaintext).expect("Encryption failed");

        // Ciphertext should be different due to random nonce
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to the same plaintext
        let decrypted1 = encryption.decrypt(&encrypted1).expect("Decryption failed");
        let decrypted2 = encryption.decrypt(&encrypted2).expect("Decryption failed");
        assert_eq!(decrypted1, plaintext);
        assert_eq!(decrypted2, plaintext);
    }

    #[test]
    fn test_decrypt_invalid_data_too_short() {
        let encryption = GitHubTokenEncryption::for_testing();
        let too_short = vec![1, 2, 3]; // Less than 12 bytes (nonce size)

        let result = encryption.decrypt(&too_short);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_invalid_data() {
        let encryption = GitHubTokenEncryption::for_testing();
        let invalid_data = vec![0u8; 24]; // 12 bytes nonce + 12 bytes garbage

        let result = encryption.decrypt(&invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_environment_missing_key() {
        // Ensure the env var is not set
        std::env::remove_var("GITHUB_TOKEN_ENCRYPTION_KEY");

        let result = GitHubTokenEncryption::from_environment();
        assert!(result.is_err());
    }

    #[test]
    fn test_from_environment_invalid_length() {
        std::env::set_var("GITHUB_TOKEN_ENCRYPTION_KEY", "short_key");

        let result = GitHubTokenEncryption::from_environment();
        assert!(result.is_err());

        std::env::remove_var("GITHUB_TOKEN_ENCRYPTION_KEY");
    }

    #[test]
    fn test_from_environment_invalid_hex() {
        std::env::set_var("GITHUB_TOKEN_ENCRYPTION_KEY", "g".repeat(64)); // Invalid hex chars

        let result = GitHubTokenEncryption::from_environment();
        assert!(result.is_err());

        std::env::remove_var("GITHUB_TOKEN_ENCRYPTION_KEY");
    }

    #[test]
    fn test_from_environment_valid() {
        let valid_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        std::env::set_var("GITHUB_TOKEN_ENCRYPTION_KEY", valid_key);

        let result = GitHubTokenEncryption::from_environment();
        assert!(result.is_ok());

        std::env::remove_var("GITHUB_TOKEN_ENCRYPTION_KEY");
    }

    #[test]
    fn test_encrypted_format() {
        let encryption = GitHubTokenEncryption::for_testing();
        let plaintext = "test_token";

        let encrypted = encryption.encrypt(plaintext).expect("Encryption failed");

        // Encrypted format: [12-byte nonce][encrypted data]
        assert!(encrypted.len() >= 12);
        assert!(encrypted.len() > 12); // Should have ciphertext after nonce
    }
}
