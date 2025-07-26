//! Cache manager for temporary data

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Manages temporary cache files
pub struct CacheManager {
    root: PathBuf,
    ttl: Duration,
}

impl CacheManager {
    /// Create a new cache manager with default TTL of 1 hour
    pub fn new() -> Result<Self> {
        let root = PathBuf::from(".mmm/cache");
        fs::create_dir_all(&root).context("Failed to create cache directory")?;
        Ok(Self {
            root,
            ttl: Duration::from_secs(3600), // 1 hour default
        })
    }

    /// Create cache manager with custom root and TTL
    pub fn with_config(root: PathBuf, ttl_seconds: u64) -> Result<Self> {
        fs::create_dir_all(&root).context("Failed to create cache directory")?;
        Ok(Self {
            root,
            ttl: Duration::from_secs(ttl_seconds),
        })
    }

    /// Get a cached value
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let cache_file = self.cache_path(key);

        if !cache_file.exists() {
            return Ok(None);
        }

        // Check age
        let metadata = fs::metadata(&cache_file).context("Failed to read cache file metadata")?;
        let age = SystemTime::now()
            .duration_since(metadata.modified()?)
            .unwrap_or(Duration::MAX);

        if age > self.ttl {
            // Cache expired
            fs::remove_file(&cache_file).context("Failed to remove expired cache file")?;
            return Ok(None);
        }

        // Read and deserialize
        let contents = fs::read_to_string(&cache_file).context("Failed to read cache file")?;
        let value = serde_json::from_str(&contents).context("Failed to deserialize cache value")?;

        Ok(Some(value))
    }

    /// Set a cached value
    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let cache_file = self.cache_path(key);
        let json =
            serde_json::to_string_pretty(value).context("Failed to serialize cache value")?;

        // Write atomically
        let temp_file = cache_file.with_extension("tmp");
        fs::write(&temp_file, json).context("Failed to write cache file")?;
        fs::rename(temp_file, cache_file).context("Failed to rename cache file")?;

        Ok(())
    }

    /// Remove a cached value
    pub fn remove(&self, key: &str) -> Result<()> {
        let cache_file = self.cache_path(key);
        if cache_file.exists() {
            fs::remove_file(cache_file).context("Failed to remove cache file")?;
        }
        Ok(())
    }

    /// Clear all cached values
    pub fn clear(&self) -> Result<()> {
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.path().extension() == Some("json".as_ref()) {
                fs::remove_file(entry.path()).context("Failed to remove cache file")?;
            }
        }
        Ok(())
    }

    /// Clean up expired cache entries
    pub fn cleanup(&self) -> Result<u32> {
        let mut removed = 0;

        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension() == Some("json".as_ref()) {
                if let Ok(metadata) = fs::metadata(&path) {
                    let age = SystemTime::now()
                        .duration_since(metadata.modified()?)
                        .unwrap_or(Duration::MAX);

                    if age > self.ttl && fs::remove_file(&path).is_ok() {
                        removed += 1;
                    }
                }
            }
        }

        Ok(removed)
    }

    /// Get the cache file path for a key
    fn cache_path(&self, key: &str) -> PathBuf {
        // Sanitize key to be filesystem-safe
        let safe_key = key.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.root.join(format!("{safe_key}.json"))
    }

    /// Check if a cache entry exists and is valid
    pub fn exists(&self, key: &str) -> bool {
        let cache_file = self.cache_path(key);

        if !cache_file.exists() {
            return false;
        }

        // Check if not expired
        if let Ok(metadata) = fs::metadata(&cache_file) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    return age <= self.ttl;
                }
            }
        }

        false
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new().expect("Failed to create cache manager")
    }
}
