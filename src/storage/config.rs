//! Storage configuration types and utilities

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Storage backend type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendType {
    /// File-based storage (default)
    File,
    /// Memory storage (for testing)
    Memory,
}

impl Default for BackendType {
    fn default() -> Self {
        Self::File
    }
}

/// Main storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage backend type
    pub backend: BackendType,

    /// Connection pool size for database backends
    #[serde(default = "default_pool_size")]
    pub connection_pool_size: usize,

    /// Retry policy for failed operations
    #[serde(default)]
    pub retry_policy: RetryPolicy,

    /// Default timeout for operations
    #[serde(with = "humantime_serde", default = "default_timeout")]
    pub timeout: Duration,

    /// Backend-specific configuration
    pub backend_config: BackendConfig,

    /// Enable distributed locking
    #[serde(default = "default_true")]
    pub enable_locking: bool,

    /// Enable caching layer
    #[serde(default)]
    pub enable_cache: bool,

    /// Cache configuration
    #[serde(default)]
    pub cache_config: CacheConfig,
}

/// Backend-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BackendConfig {
    File(FileConfig),
    Memory(MemoryConfig),
}

/// File storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    /// Base directory for storage
    pub base_dir: PathBuf,

    /// Use global storage (~/.prodigy) - local storage is deprecated
    #[serde(default = "default_true")]
    pub use_global: bool,

    /// Enable file-based locking
    #[serde(default = "default_true")]
    pub enable_file_locks: bool,

    /// Maximum file size before rotation (bytes)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Compression for archived files
    #[serde(default)]
    pub enable_compression: bool,
}

/// Memory storage configuration (for testing)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum memory usage (bytes)
    #[serde(default = "default_memory_limit")]
    pub max_memory: u64,

    /// Enable persistence to disk
    #[serde(default)]
    pub persist_to_disk: bool,

    /// Persistence file path
    pub persistence_path: Option<PathBuf>,
}

/// Retry policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Initial retry delay
    #[serde(with = "humantime_serde", default = "default_retry_delay")]
    pub initial_delay: Duration,

    /// Maximum retry delay
    #[serde(with = "humantime_serde", default = "default_max_retry_delay")]
    pub max_delay: Duration,

    /// Exponential backoff multiplier
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,

    /// Enable jitter
    #[serde(default = "default_true")]
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_delay: default_retry_delay(),
            max_delay: default_max_retry_delay(),
            backoff_multiplier: default_backoff_multiplier(),
            jitter: true,
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache size limit (entries)
    #[serde(default = "default_cache_size")]
    pub max_entries: usize,

    /// Cache TTL
    #[serde(with = "humantime_serde", default = "default_cache_ttl")]
    pub ttl: Duration,

    /// Cache implementation
    #[serde(default)]
    pub cache_type: CacheType,
}

/// Cache implementation type
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheType {
    #[default]
    Memory,
}


// Default value functions for serde
fn default_pool_size() -> usize {
    10
}

fn default_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_true() -> bool {
    true
}

fn default_max_file_size() -> u64 {
    100 * 1024 * 1024 // 100MB
}


fn default_memory_limit() -> u64 {
    1024 * 1024 * 1024 // 1GB
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay() -> Duration {
    Duration::from_secs(1)
}

fn default_max_retry_delay() -> Duration {
    Duration::from_secs(30)
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

fn default_cache_size() -> usize {
    1000
}

fn default_cache_ttl() -> Duration {
    Duration::from_secs(3600) // 1 hour
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: BackendType::default(),
            connection_pool_size: default_pool_size(),
            retry_policy: RetryPolicy::default(),
            timeout: default_timeout(),
            backend_config: BackendConfig::Memory(MemoryConfig::default()),
            enable_locking: true,
            enable_cache: false,
            cache_config: CacheConfig::default(),
        }
    }
}

impl StorageConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self, anyhow::Error> {
        // Check for backend type - now only File or Memory supported
        let backend = std::env::var("PRODIGY_STORAGE_BACKEND")
            .ok()
            .and_then(|s| match s.to_lowercase().as_str() {
                "file" => Some(BackendType::File),
                "memory" => Some(BackendType::Memory),
                _ => None,
            })
            .unwrap_or_default();

        let backend_config = match backend {
            BackendType::File => BackendConfig::File(FileConfig {
                base_dir: std::env::var("PRODIGY_STORAGE_DIR")
                    .or_else(|_| std::env::var("PRODIGY_STORAGE_PATH"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| {
                        dirs::home_dir()
                            .unwrap_or_else(|| PathBuf::from("/tmp"))
                            .join(".prodigy")
                    }),
                use_global: true, // Always use global storage
                enable_file_locks: true,
                max_file_size: default_max_file_size(),
                enable_compression: false,
            }),
            BackendType::Memory => BackendConfig::Memory(Default::default()),
        };

        Ok(Self {
            backend,
            connection_pool_size: default_pool_size(),
            retry_policy: Default::default(),
            timeout: default_timeout(),
            backend_config,
            enable_locking: true,
            enable_cache: false,
            cache_config: Default::default(),
        })
    }
}
