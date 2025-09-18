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
    /// PostgreSQL database
    Postgres,
    /// Redis cache/database
    Redis,
    /// Amazon S3 or compatible object storage
    S3,
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
    Postgres(PostgresConfig),
    Redis(RedisConfig),
    S3(S3Config),
    Memory(MemoryConfig),
}

/// File storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    /// Base directory for storage
    pub base_dir: PathBuf,

    /// Use global storage (~/.prodigy) vs local (.prodigy)
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

/// PostgreSQL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    /// Connection string
    pub connection_string: String,

    /// Schema name
    #[serde(default = "default_schema")]
    pub schema: String,

    /// Enable SSL
    #[serde(default)]
    pub ssl_mode: SslMode,

    /// Maximum connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Connection timeout
    #[serde(with = "humantime_serde", default = "default_connection_timeout")]
    pub connection_timeout: Duration,

    /// Statement timeout
    #[serde(with = "humantime_serde", default = "default_statement_timeout")]
    pub statement_timeout: Duration,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    pub url: String,

    /// Database number
    #[serde(default)]
    pub database: u8,

    /// Key prefix
    #[serde(default = "default_key_prefix")]
    pub key_prefix: String,

    /// Enable cluster mode
    #[serde(default)]
    pub cluster_mode: bool,

    /// Connection pool size
    #[serde(default = "default_redis_pool_size")]
    pub pool_size: usize,

    /// Key expiration TTL
    #[serde(with = "humantime_serde", default = "default_redis_ttl")]
    pub default_ttl: Duration,
}

/// S3 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// Bucket name
    pub bucket: String,

    /// Region
    pub region: String,

    /// Endpoint URL (for S3-compatible services)
    pub endpoint: Option<String>,

    /// Access key ID
    pub access_key_id: Option<String>,

    /// Secret access key
    pub secret_access_key: Option<String>,

    /// Object key prefix
    #[serde(default = "default_s3_prefix")]
    pub prefix: String,

    /// Enable server-side encryption
    #[serde(default)]
    pub enable_encryption: bool,

    /// Storage class
    #[serde(default)]
    pub storage_class: S3StorageClass,
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
    Redis,
}

/// SSL mode for PostgreSQL
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SslMode {
    #[default]
    Prefer,
    Disable,
    Require,
    VerifyCa,
    VerifyFull,
}

/// S3 storage class
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum S3StorageClass {
    #[default]
    Standard,
    StandardIa,
    IntelligentTiering,
    GlacierFlexibleRetrieval,
    GlacierInstantRetrieval,
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

fn default_schema() -> String {
    "prodigy".to_string()
}

fn default_max_connections() -> u32 {
    20
}

fn default_connection_timeout() -> Duration {
    Duration::from_secs(10)
}

fn default_statement_timeout() -> Duration {
    Duration::from_secs(60)
}

fn default_key_prefix() -> String {
    "prodigy:".to_string()
}

fn default_redis_pool_size() -> usize {
    10
}

fn default_redis_ttl() -> Duration {
    Duration::from_secs(86400) // 24 hours
}

fn default_s3_prefix() -> String {
    "prodigy/".to_string()
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

impl StorageConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self, anyhow::Error> {
        // Check for backend type
        let backend = std::env::var("PRODIGY_STORAGE_BACKEND")
            .ok()
            .and_then(|s| serde_json::from_value(serde_json::Value::String(s)).ok())
            .unwrap_or_default();

        let backend_config = match backend {
            BackendType::File => BackendConfig::File(FileConfig {
                base_dir: std::env::var("PRODIGY_STORAGE_PATH")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| {
                        dirs::home_dir()
                            .unwrap_or_else(|| PathBuf::from("/tmp"))
                            .join(".prodigy")
                    }),
                use_global: std::env::var("PRODIGY_USE_LOCAL_STORAGE") != Ok("true".to_string()),
                enable_file_locks: true,
                max_file_size: default_max_file_size(),
                enable_compression: false,
            }),
            BackendType::Postgres => {
                let connection_string = std::env::var("PRODIGY_POSTGRES_URL")
                    .or_else(|_| std::env::var("DATABASE_URL"))
                    .map_err(|_| anyhow::anyhow!("PostgreSQL connection string not found"))?;

                BackendConfig::Postgres(PostgresConfig {
                    connection_string,
                    schema: std::env::var("PRODIGY_POSTGRES_SCHEMA")
                        .unwrap_or_else(|_| default_schema()),
                    ssl_mode: Default::default(),
                    max_connections: default_max_connections(),
                    connection_timeout: default_connection_timeout(),
                    statement_timeout: default_statement_timeout(),
                })
            }
            BackendType::Redis => {
                let url = std::env::var("PRODIGY_REDIS_URL")
                    .or_else(|_| std::env::var("REDIS_URL"))
                    .map_err(|_| anyhow::anyhow!("Redis URL not found"))?;

                BackendConfig::Redis(RedisConfig {
                    url,
                    database: 0,
                    key_prefix: default_key_prefix(),
                    cluster_mode: false,
                    pool_size: default_redis_pool_size(),
                    default_ttl: default_redis_ttl(),
                })
            }
            BackendType::S3 => BackendConfig::S3(S3Config {
                bucket: std::env::var("PRODIGY_S3_BUCKET")
                    .map_err(|_| anyhow::anyhow!("S3 bucket not specified"))?,
                region: std::env::var("AWS_REGION")
                    .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
                    .unwrap_or_else(|_| "us-east-1".to_string()),
                endpoint: std::env::var("PRODIGY_S3_ENDPOINT").ok(),
                access_key_id: std::env::var("AWS_ACCESS_KEY_ID").ok(),
                secret_access_key: std::env::var("AWS_SECRET_ACCESS_KEY").ok(),
                prefix: default_s3_prefix(),
                enable_encryption: false,
                storage_class: Default::default(),
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
