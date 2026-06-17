//! Storage abstraction layer for file uploads and static assets
//!
//! This module provides a unified interface for file storage operations,
//! currently supporting local filesystem storage. The abstraction allows
//! for easy extension to other backends (S3, in-memory) in the future.
//!
//! # Example Usage
//!
//! ```no_run
//! use axum_kickoff::storage::{Storage, StorageConfig};
//! use bytes::Bytes;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = StorageConfig::local_filesystem("/tmp/uploads");
//! let storage = Storage::from_config(&config);
//!
//! // Upload a file
//! let bytes = Bytes::from_static(b"hello world");
//! storage.upload("test.txt", bytes).await?;
//!
//! // Download a file
//! let data = storage.download("test.txt").await?;
//! # Ok(())
//! # }
//! ```

use anyhow::Context;
use bytes::Bytes;
use std::path::{Path, PathBuf};
use tracing::instrument;

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    backend: StorageBackend,
    pub cdn_prefix: Option<String>,
}

#[derive(Debug, Clone)]
pub enum StorageBackend {
    LocalFileSystem { path: PathBuf },
}

impl StorageConfig {
    /// Create a local filesystem storage configuration
    pub fn local_filesystem(path: impl Into<PathBuf>) -> Self {
        Self {
            backend: StorageBackend::LocalFileSystem { path: path.into() },
            cdn_prefix: None,
        }
    }

    /// Create a local filesystem storage configuration with CDN prefix
    pub fn local_filesystem_with_cdn(path: impl Into<PathBuf>, cdn_prefix: String) -> Self {
        Self {
            backend: StorageBackend::LocalFileSystem { path: path.into() },
            cdn_prefix: Some(cdn_prefix),
        }
    }

    /// Create storage configuration from environment variables
    ///
    /// Environment variables:
    /// - `STORAGE_PATH`: Path for local filesystem storage (default: "./local_uploads")
    /// - `CDN_PREFIX`: Optional CDN prefix for generating URLs
    pub fn from_environment() -> Self {
        let path = dotenvy::var("STORAGE_PATH").unwrap_or_else(|_| "./local_uploads".to_string());
        let cdn_prefix = dotenvy::var("CDN_PREFIX").ok();

        Self {
            backend: StorageBackend::LocalFileSystem { path: path.into() },
            cdn_prefix,
        }
    }
}

/// Storage backend for file operations
pub struct Storage {
    cdn_prefix: Option<String>,
    base_path: PathBuf,
}

/// Validate that a storage path is safe and doesn't escape the base directory
fn safe_storage_path(base: &Path, input: &str) -> anyhow::Result<PathBuf> {
    let path = Path::new(input);

    // Reject absolute paths
    if path.is_absolute() {
        anyhow::bail!("Invalid storage path: absolute paths not allowed");
    }

    // Reject paths with parent directory components (..)
    if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        anyhow::bail!("Invalid storage path: parent directory references not allowed");
    }

    // Normalize the path and join with base
    let full_path = base.join(path);

    // Verify the resulting path is still within the base directory
    if !full_path.starts_with(base) {
        anyhow::bail!("Invalid storage path: path escapes base directory");
    }

    Ok(full_path)
}

impl Storage {
    /// Create storage from configuration
    pub fn from_config(config: &StorageConfig) -> Self {
        let cdn_prefix = config.cdn_prefix.clone();

        match &config.backend {
            StorageBackend::LocalFileSystem { path } => {
                tracing::info!(?path, "Using local file system for storage");

                std::fs::create_dir_all(path)
                    .context("Failed to create storage directory")
                    .unwrap();

                Self {
                    cdn_prefix,
                    base_path: path.clone(),
                }
            }
        }
    }

    /// Create storage from environment variables
    pub fn from_environment() -> Self {
        Self::from_config(&StorageConfig::from_environment())
    }

    /// Get the public URL for a file
    ///
    /// This function doesn't check for file existence, it only generates the URL.
    pub fn public_url(&self, path: &str) -> String {
        apply_cdn_prefix(&self.cdn_prefix, path)
    }

    /// Upload a file
    #[instrument(skip(self, bytes))]
    pub async fn upload(&self, path: &str, bytes: Bytes) -> anyhow::Result<()> {
        let file_path = safe_storage_path(&self.base_path, path)?;

        // Create parent directories if they don't exist
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directories")?;
        }

        tokio::fs::write(&file_path, bytes)
            .await
            .context("Failed to write file")?;

        Ok(())
    }

    /// Upload a file with custom content type
    ///
    /// Note: Local filesystem doesn't support content-type metadata.
    /// This is a no-op for local storage but kept for API compatibility.
    #[instrument(skip(self, bytes))]
    pub async fn upload_with_content_type(
        &self,
        path: &str,
        bytes: Bytes,
        _content_type: &'static str,
    ) -> anyhow::Result<()> {
        self.upload(path, bytes).await
    }

    /// Download a file
    #[instrument(skip(self))]
    pub async fn download(&self, path: &str) -> anyhow::Result<Bytes> {
        let file_path = safe_storage_path(&self.base_path, path)?;
        let data = tokio::fs::read(&file_path)
            .await
            .context("Failed to read file")?;
        Ok(Bytes::from(data))
    }

    /// Delete a file
    #[instrument(skip(self))]
    pub async fn delete(&self, path: &str) -> anyhow::Result<()> {
        let file_path = safe_storage_path(&self.base_path, path)?;
        tokio::fs::remove_file(&file_path)
            .await
            .context("Failed to delete file")?;
        Ok(())
    }

    /// Delete all files with a given prefix
    #[instrument(skip(self))]
    pub async fn delete_all_with_prefix(&self, prefix: &str) -> anyhow::Result<Vec<String>> {
        let prefix_path = safe_storage_path(&self.base_path, prefix)?;
        let mut deleted = Vec::new();

        if !prefix_path.exists() {
            return Ok(deleted);
        }

        // Recursively delete all files and directories
        let mut stack = vec![prefix_path.clone()];

        while let Some(current_path) = stack.pop() {
            let mut entries = tokio::fs::read_dir(&current_path)
                .await
                .context("Failed to read directory")?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .context("Failed to read directory entry")?
            {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    tokio::fs::remove_file(&path)
                        .await
                        .context("Failed to delete file")?;

                    if let Ok(rel_path) = path.strip_prefix(&self.base_path) {
                        deleted.push(rel_path.to_string_lossy().to_string());
                    }
                }
            }
        }

        // Remove the directory itself
        tokio::fs::remove_dir_all(&prefix_path).await.ok();

        Ok(deleted)
    }

    /// Check if a file exists
    #[instrument(skip(self))]
    pub async fn exists(&self, path: &str) -> anyhow::Result<bool> {
        let file_path = safe_storage_path(&self.base_path, path)?;
        Ok(tokio::fs::try_exists(&file_path).await.unwrap_or(false))
    }

    /// List all files with a given prefix
    #[instrument(skip(self))]
    pub async fn list(&self, prefix: Option<&str>) -> anyhow::Result<Vec<String>> {
        let base_path = if let Some(prefix) = prefix {
            safe_storage_path(&self.base_path, prefix)?
        } else {
            self.base_path.clone()
        };

        let mut files = Vec::new();

        if !base_path.exists() {
            return Ok(files);
        }

        // Recursively list all files
        let mut stack = vec![base_path];

        while let Some(current_path) = stack.pop() {
            let mut entries = tokio::fs::read_dir(&current_path)
                .await
                .context("Failed to read directory")?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .context("Failed to read directory entry")?
            {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    if let Ok(rel_path) = path.strip_prefix(&self.base_path) {
                        files.push(rel_path.to_string_lossy().to_string());
                    }
                }
            }
        }

        files.sort();
        Ok(files)
    }

    /// Get the base path for this storage backend
    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }
}

fn apply_cdn_prefix(cdn_prefix: &Option<String>, path: &str) -> String {
    match cdn_prefix {
        Some(cdn_prefix) if !cdn_prefix.starts_with("https://") => {
            format!("https://{cdn_prefix}/{path}")
        }
        Some(cdn_prefix) => format!("{cdn_prefix}/{path}"),
        None => format!("/{path}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn prepare_storage() -> (Storage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig::local_filesystem(temp_dir.path());
        let storage = Storage::from_config(&config);
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_upload_and_download() {
        let (storage, _temp_dir) = prepare_storage().await;

        let bytes = Bytes::from_static(b"hello world");
        storage.upload("test.txt", bytes.clone()).await.unwrap();

        let downloaded = storage.download("test.txt").await.unwrap();
        assert_eq!(downloaded, bytes);
    }

    #[tokio::test]
    async fn test_upload_with_content_type() {
        let (storage, _temp_dir) = prepare_storage().await;

        let bytes = Bytes::from_static(b"hello world");
        storage
            .upload_with_content_type("test.json", bytes, "application/json")
            .await
            .unwrap();

        let files = storage.list(None).await.unwrap();
        assert_eq!(files, vec!["test.json"]);
    }

    #[tokio::test]
    async fn test_delete() {
        let (storage, _temp_dir) = prepare_storage().await;

        let bytes = Bytes::from_static(b"hello world");
        storage.upload("test.txt", bytes).await.unwrap();

        storage.delete("test.txt").await.unwrap();

        let files = storage.list(None).await.unwrap();
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_delete_all_with_prefix() {
        let (storage, _temp_dir) = prepare_storage().await;

        let bytes = Bytes::from_static(b"hello world");
        storage
            .upload("dir/test1.txt", bytes.clone())
            .await
            .unwrap();
        storage
            .upload("dir/test2.txt", bytes.clone())
            .await
            .unwrap();
        storage.upload("other/test3.txt", bytes).await.unwrap();

        let deleted = storage.delete_all_with_prefix("dir/").await.unwrap();
        assert_eq!(deleted.len(), 2);

        let files = storage.list(None).await.unwrap();
        assert_eq!(files, vec!["other/test3.txt"]);
    }

    #[tokio::test]
    async fn test_exists() {
        let (storage, _temp_dir) = prepare_storage().await;

        assert!(!storage.exists("test.txt").await.unwrap());

        let bytes = Bytes::from_static(b"hello world");
        storage.upload("test.txt", bytes).await.unwrap();

        assert!(storage.exists("test.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_list() {
        let (storage, _temp_dir) = prepare_storage().await;

        let bytes = Bytes::from_static(b"hello world");
        storage.upload("test1.txt", bytes.clone()).await.unwrap();
        storage.upload("test2.txt", bytes.clone()).await.unwrap();
        storage.upload("dir/test3.txt", bytes).await.unwrap();

        let all_files = storage.list(None).await.unwrap();
        assert_eq!(all_files.len(), 3);

        let dir_files = storage.list(Some("dir/")).await.unwrap();
        assert_eq!(dir_files, vec!["dir/test3.txt"]);
    }

    #[test]
    fn test_public_url() {
        let config = StorageConfig::local_filesystem("/tmp");
        let storage = Storage::from_config(&config);

        assert_eq!(storage.public_url("test.txt"), "/test.txt");
    }

    #[test]
    fn test_public_url_with_cdn() {
        let config =
            StorageConfig::local_filesystem_with_cdn("/tmp", "cdn.example.com".to_string());
        let storage = Storage::from_config(&config);

        assert_eq!(
            storage.public_url("test.txt"),
            "https://cdn.example.com/test.txt"
        );
    }

    #[test]
    fn test_public_url_with_https_cdn() {
        let config =
            StorageConfig::local_filesystem_with_cdn("/tmp", "https://cdn.example.com".to_string());
        let storage = Storage::from_config(&config);

        assert_eq!(
            storage.public_url("test.txt"),
            "https://cdn.example.com/test.txt"
        );
    }

    #[test]
    fn test_config_from_environment() {
        std::env::set_var("STORAGE_PATH", "/custom/path");
        std::env::set_var("CDN_PREFIX", "cdn.example.com");

        let config = StorageConfig::from_environment();
        assert!(matches!(
            config.backend,
            StorageBackend::LocalFileSystem { path } if path == *"/custom/path"
        ));
        assert_eq!(config.cdn_prefix, Some("cdn.example.com".to_string()));

        std::env::remove_var("STORAGE_PATH");
        std::env::remove_var("CDN_PREFIX");
    }
}
