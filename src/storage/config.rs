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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_memory: 100 * 1024 * 1024, // 100 MB
            persist_to_disk: false,
            persistence_path: None,
        }
    }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: default_cache_size(),
            ttl: default_cache_ttl(),
            cache_type: CacheType::default(),
        }
    }
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
    pub fn from_env() -> crate::LibResult<Self> {
        // Check for backend type - now only File or Memory supported
        let backend = std::env::var("PRODIGY_STORAGE_TYPE")
            .ok()
            .and_then(|s| match s.to_lowercase().as_str() {
                "file" => Some(BackendType::File),
                "memory" => Some(BackendType::Memory),
                _ => None,
            })
            .unwrap_or_default();

        let backend_config = match backend {
            BackendType::File => BackendConfig::File(FileConfig {
                base_dir: std::env::var("PRODIGY_STORAGE_BASE_PATH")
                    .or_else(|_| std::env::var("PRODIGY_STORAGE_DIR"))
                    .or_else(|_| std::env::var("PRODIGY_STORAGE_PATH"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| {
                        directories::BaseDirs::new()
                            .map(|dirs| dirs.home_dir().join(".prodigy"))
                            .unwrap_or_else(|| PathBuf::from("/tmp").join(".prodigy"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use std::time::Duration;

    #[test]
    fn test_backend_type_default() {
        assert_eq!(BackendType::default(), BackendType::File);
    }

    #[test]
    fn test_backend_type_serialization() {
        let backend = BackendType::File;
        let json = serde_json::to_string(&backend).unwrap();
        assert_eq!(json, r#""file""#);

        let backend = BackendType::Memory;
        let json = serde_json::to_string(&backend).unwrap();
        assert_eq!(json, r#""memory""#);
    }

    #[test]
    fn test_backend_type_deserialization() {
        let backend: BackendType = serde_json::from_str(r#""file""#).unwrap();
        assert_eq!(backend, BackendType::File);

        let backend: BackendType = serde_json::from_str(r#""memory""#).unwrap();
        assert_eq!(backend, BackendType::Memory);
    }

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();

        assert_eq!(config.backend, BackendType::File);
        assert_eq!(config.connection_pool_size, 10);
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(config.enable_locking);
        assert!(!config.enable_cache);
    }

    #[test]
    fn test_file_config_defaults() {
        let config = FileConfig {
            base_dir: PathBuf::from("/test"),
            use_global: default_true(),
            enable_file_locks: default_true(),
            max_file_size: default_max_file_size(),
            enable_compression: false,
        };

        assert_eq!(config.base_dir, PathBuf::from("/test"));
        assert!(config.use_global);
        assert!(config.enable_file_locks);
        assert_eq!(config.max_file_size, 100 * 1024 * 1024); // 100 MB
    }

    #[test]
    fn test_memory_config_defaults() {
        let config = MemoryConfig::default();

        assert_eq!(config.max_memory, 100 * 1024 * 1024); // 100 MB
    }

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();

        assert!(policy.max_retries > 0);
        assert!(policy.initial_delay > Duration::from_secs(0));
        assert!(policy.max_delay > Duration::from_secs(0));
        assert!(policy.backoff_multiplier > 1.0);
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();

        assert!(config.max_entries > 0);
        assert!(config.ttl > Duration::from_secs(0));
    }

    #[test]
    #[serial]
    fn test_storage_config_from_env_defaults() {
        // Clear relevant env vars
        env::remove_var("PRODIGY_STORAGE_TYPE");
        env::remove_var("PRODIGY_STORAGE_BASE_PATH");

        let config = StorageConfig::from_env().unwrap();

        assert_eq!(config.backend, BackendType::File);
        assert!(config.enable_locking);
        assert!(!config.enable_cache);

        if let BackendConfig::File(file_config) = config.backend_config {
            // Should use home directory
            assert!(file_config.base_dir.to_string_lossy().contains(".prodigy"));
            assert!(file_config.use_global);
        } else {
            panic!("Expected FileConfig");
        }
    }

    #[test]
    #[serial]
    fn test_storage_config_from_env_file_type() {
        env::set_var("PRODIGY_STORAGE_TYPE", "file");
        env::set_var("PRODIGY_STORAGE_BASE_PATH", "/custom/path");

        let config = StorageConfig::from_env().unwrap();

        assert_eq!(config.backend, BackendType::File);

        if let BackendConfig::File(file_config) = config.backend_config {
            assert_eq!(file_config.base_dir, PathBuf::from("/custom/path"));
        } else {
            panic!("Expected FileConfig");
        }

        // Cleanup
        env::remove_var("PRODIGY_STORAGE_TYPE");
        env::remove_var("PRODIGY_STORAGE_BASE_PATH");
    }

    #[test]
    #[serial]
    fn test_storage_config_from_env_memory_type() {
        env::set_var("PRODIGY_STORAGE_TYPE", "memory");

        let config = StorageConfig::from_env().unwrap();

        assert_eq!(config.backend, BackendType::Memory);

        if let BackendConfig::Memory(memory_config) = config.backend_config {
            assert_eq!(memory_config.max_memory, 100 * 1024 * 1024);
        } else {
            panic!("Expected MemoryConfig");
        }

        // Cleanup
        env::remove_var("PRODIGY_STORAGE_TYPE");
    }

    #[test]
    #[serial]
    fn test_storage_config_from_env_invalid_type() {
        env::set_var("PRODIGY_STORAGE_TYPE", "invalid");

        let result = StorageConfig::from_env();
        // Invalid types should default to File backend
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.backend, BackendType::File);

        // Cleanup
        env::remove_var("PRODIGY_STORAGE_TYPE");
    }

    #[test]
    fn test_storage_config_serialization() {
        let config = StorageConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: StorageConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.backend, deserialized.backend);
        assert_eq!(
            config.connection_pool_size,
            deserialized.connection_pool_size
        );
        assert_eq!(config.enable_locking, deserialized.enable_locking);
    }

    #[test]
    fn test_file_config_serialization() {
        let config = FileConfig {
            base_dir: PathBuf::from("/test/path"),
            use_global: true,
            enable_file_locks: false,
            max_file_size: 1024,
            enable_compression: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: FileConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.base_dir, deserialized.base_dir);
        assert_eq!(config.use_global, deserialized.use_global);
        assert_eq!(config.enable_file_locks, deserialized.enable_file_locks);
        assert_eq!(config.max_file_size, deserialized.max_file_size);
    }

    #[test]
    fn test_default_helper_functions() {
        assert!(default_true());
        // default_false doesn't exist, just test default_true
        assert_eq!(default_pool_size(), 10);
        assert_eq!(default_timeout(), Duration::from_secs(30));
        assert_eq!(default_max_file_size(), 100 * 1024 * 1024);
    }

    #[test]
    fn test_backend_config_untagged_enum() {
        // Test that BackendConfig can be deserialized from JSON without type tags
        let file_json = r#"{
            "base_dir": "/test",
            "use_global": true,
            "enable_file_locks": true,
            "max_file_size": 1000000,
            "enable_compression": false
        }"#;

        let backend_config: BackendConfig = serde_json::from_str(file_json).unwrap();

        if let BackendConfig::File(config) = backend_config {
            assert_eq!(config.base_dir, PathBuf::from("/test"));
            assert!(config.use_global);
        } else {
            panic!("Expected FileConfig");
        }
    }

    #[test]
    fn test_retry_policy_validation() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
        };

        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.initial_delay, Duration::from_millis(50));
        assert_eq!(policy.max_delay, Duration::from_secs(60));
        assert_eq!(policy.backoff_multiplier, 2.0);
        assert!(policy.jitter);
    }

    #[test]
    fn test_cache_config_with_custom_values() {
        let config = CacheConfig {
            max_entries: 5000,
            ttl: Duration::from_secs(600),
            cache_type: Default::default(),
        };

        assert_eq!(config.max_entries, 5000);
        assert_eq!(config.ttl, Duration::from_secs(600));
    }
}
