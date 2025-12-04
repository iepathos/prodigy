//! Secret store for managing sensitive values
//!
//! Provides secure storage and retrieval of secrets with support for
//! multiple providers including environment variables, files, and external systems.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use thiserror::Error;

/// Secret store errors
#[derive(Debug, Error)]
pub enum SecretStoreError {
    #[error("Secret not found: {0}")]
    NotFound(String),

    #[error("Provider not configured: {0}")]
    ProviderNotConfigured(String),

    #[error("Failed to decrypt secret: {0}")]
    DecryptionFailed(String),

    #[error("Invalid secret format: {0}")]
    InvalidFormat(String),
}

/// Secret store for managing sensitive values
pub struct SecretStore {
    /// In-memory cache of decrypted secrets
    cache: HashMap<String, String>,
    /// Provider configurations
    providers: HashMap<String, Box<dyn SecretProvider>>,
}

impl SecretStore {
    /// Create a new secret store
    pub fn new() -> Self {
        let mut providers: HashMap<String, Box<dyn SecretProvider>> = HashMap::new();

        // Add default providers
        providers.insert("env".to_string(), Box::new(EnvSecretProvider));
        providers.insert("file".to_string(), Box::new(FileSecretProvider));

        Self {
            cache: HashMap::new(),
            providers,
        }
    }

    /// Get a secret by key
    pub async fn get_secret(&mut self, key: &str) -> Result<String> {
        // Check cache first
        if let Some(cached) = self.cache.get(key) {
            return Ok(cached.clone());
        }

        // Try to parse provider from key format (provider:path)
        let (provider_name, secret_key) = if let Some(colon_pos) = key.find(':') {
            (&key[..colon_pos], &key[colon_pos + 1..])
        } else {
            // Default to env provider
            ("env", key)
        };

        // Get from provider
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| SecretStoreError::ProviderNotConfigured(provider_name.to_string()))?;

        let value = provider.get_secret(secret_key).await?;

        // Cache the value
        self.cache.insert(key.to_string(), value.clone());

        Ok(value)
    }

    /// Add a custom secret provider
    pub fn add_provider(&mut self, name: String, provider: Box<dyn SecretProvider>) {
        self.providers.insert(name, provider);
    }

    /// Clear cached secrets
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Check if a secret exists
    pub async fn has_secret(&self, key: &str) -> bool {
        if self.cache.contains_key(key) {
            return true;
        }

        // Parse provider and check
        let (provider_name, secret_key) = if let Some(colon_pos) = key.find(':') {
            (&key[..colon_pos], &key[colon_pos + 1..])
        } else {
            ("env", key)
        };

        if let Some(provider) = self.providers.get(provider_name) {
            provider.has_secret(secret_key).await
        } else {
            false
        }
    }
}

/// Trait for secret providers
#[async_trait::async_trait]
pub trait SecretProvider: Send + Sync {
    /// Get a secret by key
    async fn get_secret(&self, key: &str) -> Result<String>;

    /// Check if a secret exists
    async fn has_secret(&self, key: &str) -> bool;
}

/// Environment variable secret provider
struct EnvSecretProvider;

#[async_trait::async_trait]
impl SecretProvider for EnvSecretProvider {
    async fn get_secret(&self, key: &str) -> Result<String> {
        std::env::var(key).map_err(|_| anyhow!(SecretStoreError::NotFound(key.to_string())))
    }

    async fn has_secret(&self, key: &str) -> bool {
        std::env::var(key).is_ok()
    }
}

/// File-based secret provider
struct FileSecretProvider;

#[async_trait::async_trait]
impl SecretProvider for FileSecretProvider {
    async fn get_secret(&self, path: &str) -> Result<String> {
        tokio::fs::read_to_string(path)
            .await
            .map(|s| s.trim().to_string())
            .map_err(|e| anyhow!("Failed to read secret file {}: {}", path, e))
    }

    async fn has_secret(&self, path: &str) -> bool {
        tokio::fs::metadata(path).await.is_ok()
    }
}

/// Mock secret provider for testing
#[cfg(test)]
pub struct MockSecretProvider {
    secrets: HashMap<String, String>,
}

#[cfg(test)]
impl MockSecretProvider {
    pub fn new() -> Self {
        Self {
            secrets: HashMap::new(),
        }
    }

    pub fn add_secret(&mut self, key: String, value: String) {
        self.secrets.insert(key, value);
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl SecretProvider for MockSecretProvider {
    async fn get_secret(&self, key: &str) -> Result<String> {
        self.secrets
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow!(SecretStoreError::NotFound(key.to_string())))
    }

    async fn has_secret(&self, key: &str) -> bool {
        self.secrets.contains_key(key)
    }
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[serial_test::serial] // Must run alone - modifies global env vars
    async fn test_env_secret_provider() {
        std::env::set_var("TEST_SECRET", "secret_value");

        let provider = EnvSecretProvider;
        let value = provider.get_secret("TEST_SECRET").await.unwrap();
        assert_eq!(value, "secret_value");

        assert!(provider.has_secret("TEST_SECRET").await);
        assert!(!provider.has_secret("NONEXISTENT").await);
    }

    #[tokio::test]
    #[serial_test::serial] // Must run alone - modifies global env vars
    async fn test_secret_store_cache() {
        let mut store = SecretStore::new();

        std::env::set_var("CACHED_SECRET", "cached_value");

        // First access should fetch from provider
        let value1 = store.get_secret("env:CACHED_SECRET").await.unwrap();
        assert_eq!(value1, "cached_value");

        // Change the env var
        std::env::set_var("CACHED_SECRET", "new_value");

        // Second access should use cache
        let value2 = store.get_secret("env:CACHED_SECRET").await.unwrap();
        assert_eq!(value2, "cached_value"); // Still cached

        // Clear cache and fetch again
        store.clear_cache();
        let value3 = store.get_secret("env:CACHED_SECRET").await.unwrap();
        assert_eq!(value3, "new_value"); // Fresh from provider
    }

    #[tokio::test]
    async fn test_mock_secret_provider() {
        let mut mock = MockSecretProvider::new();
        mock.add_secret("test_key".to_string(), "test_value".to_string());

        let mut store = SecretStore::new();
        store.add_provider("mock".to_string(), Box::new(mock));

        let value = store.get_secret("mock:test_key").await.unwrap();
        assert_eq!(value, "test_value");

        assert!(store.has_secret("mock:test_key").await);
    }

    #[tokio::test]
    #[serial_test::serial] // Must run alone - modifies global env vars
    async fn test_default_provider() {
        let mut store = SecretStore::new();

        std::env::set_var("DEFAULT_SECRET", "default_value");

        // Without provider prefix, should use env provider
        let value = store.get_secret("DEFAULT_SECRET").await.unwrap();
        assert_eq!(value, "default_value");
    }
}
