//! Response caching system

use crate::claude::api::ClaudeResponse;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Cached response with metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedResponse {
    pub response: String,
    pub timestamp: SystemTime,
    pub prompt_hash: String,
    pub model: String,
    pub tokens_used: usize,
}

/// Response cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_age: Duration,
    pub max_size_mb: usize,
    pub cache_successful_only: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(86400), // 24 hours
            max_size_mb: 100,
            cache_successful_only: true,
        }
    }
}

/// Response caching system
pub struct ResponseCache {
    cache_dir: PathBuf,
    config: CacheConfig,
}

impl ResponseCache {
    /// Create a new response cache
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        Self::with_config(cache_dir, CacheConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(cache_dir: PathBuf, config: CacheConfig) -> Result<Self> {
        // Create cache directory if needed
        fs::create_dir_all(&cache_dir).map_err(|e| Error::Io(e))?;

        Ok(Self { cache_dir, config })
    }

    /// Get cached response if available and fresh
    pub fn get_cached(&self, prompt: &str) -> Option<CachedResponse> {
        let hash = self.hash_prompt(prompt);
        let cache_file = self.cache_dir.join(format!("{}.json", hash));

        if !cache_file.exists() {
            return None;
        }

        // Read cache file
        let content = fs::read_to_string(&cache_file).ok()?;
        let cached: CachedResponse = serde_json::from_str(&content).ok()?;

        // Check if cache is fresh
        if let Ok(elapsed) = cached.timestamp.elapsed() {
            if elapsed > self.config.max_age {
                // Cache expired, remove it
                let _ = fs::remove_file(&cache_file);
                return None;
            }
        }

        Some(cached)
    }

    /// Store a response in cache
    pub fn store(&self, prompt: &str, response: &ClaudeResponse) -> Result<()> {
        let hash = self.hash_prompt(prompt);
        let cache_file = self.cache_dir.join(format!("{}.json", hash));

        let cached = CachedResponse {
            response: response.content.clone(),
            timestamp: SystemTime::now(),
            prompt_hash: hash.clone(),
            model: response.model.clone(),
            tokens_used: response.tokens_used,
        };

        let json = serde_json::to_string_pretty(&cached)
            .map_err(|e| Error::Parse(format!("Failed to serialize cache: {}", e)))?;

        fs::write(&cache_file, json).map_err(|e| Error::Io(e))?;

        // Check cache size and clean if needed
        self.cleanup_if_needed()?;

        Ok(())
    }

    /// Check if a response should be cached
    pub fn should_cache(&self, response: &ClaudeResponse) -> bool {
        // Don't cache if configured to only cache successful responses
        if self.config.cache_successful_only {
            // Check for error indicators
            if response.content.contains("error")
                || response.content.contains("failed")
                || response.stop_reason == Some("error".to_string())
            {
                return false;
            }
        }

        // Don't cache very small responses
        if response.content.len() < 100 {
            return false;
        }

        // Cache expensive operations (high token usage)
        if response.tokens_used > 1000 {
            return true;
        }

        // Default to caching
        true
    }

    /// Hash a prompt for cache key
    fn hash_prompt(&self, prompt: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(prompt.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Clean up cache if it exceeds size limit
    fn cleanup_if_needed(&self) -> Result<()> {
        let mut total_size = 0;
        let mut cache_files = Vec::new();

        // Collect all cache files with metadata
        for entry in fs::read_dir(&self.cache_dir).map_err(|e| Error::Io(e))? {
            let entry = entry.map_err(|e| Error::Io(e))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                    if let Ok(modified) = metadata.modified() {
                        cache_files.push((path, modified, metadata.len()));
                    }
                }
            }
        }

        // Check if cleanup is needed
        let max_size_bytes = self.config.max_size_mb * 1024 * 1024;
        if total_size > max_size_bytes as u64 {
            // Sort by modification time (oldest first)
            cache_files.sort_by_key(|&(_, modified, _)| modified);

            // Remove oldest files until under limit
            let mut removed_size = 0;
            for (path, _, size) in cache_files {
                fs::remove_file(&path).map_err(|e| Error::Io(e))?;
                removed_size += size;

                if total_size - removed_size <= max_size_bytes as u64 {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Clear all cache
    pub fn clear(&self) -> Result<()> {
        for entry in fs::read_dir(&self.cache_dir).map_err(|e| Error::Io(e))? {
            let entry = entry.map_err(|e| Error::Io(e))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                fs::remove_file(&path).map_err(|e| Error::Io(e))?;
            }
        }

        Ok(())
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> Result<CacheStats> {
        let mut total_size = 0;
        let mut file_count = 0;
        let mut oldest = SystemTime::now();
        let mut newest = SystemTime::UNIX_EPOCH;

        for entry in fs::read_dir(&self.cache_dir).map_err(|e| Error::Io(e))? {
            let entry = entry.map_err(|e| Error::Io(e))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                file_count += 1;

                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();

                    if let Ok(modified) = metadata.modified() {
                        if modified < oldest {
                            oldest = modified;
                        }
                        if modified > newest {
                            newest = modified;
                        }
                    }
                }
            }
        }

        Ok(CacheStats {
            total_size_mb: total_size as f64 / 1024.0 / 1024.0,
            file_count,
            oldest_entry: oldest,
            newest_entry: newest,
        })
    }
}

/// Cache statistics
#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub total_size_mb: f64,
    pub file_count: usize,
    pub oldest_entry: SystemTime,
    pub newest_entry: SystemTime,
}
