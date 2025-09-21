//! Storage factory for creating storage instances

use super::backends::{FileBackend, MemoryBackend};

#[cfg(feature = "postgres")]
use super::backends::PostgresBackend;
#[cfg(feature = "redis")]
use super::backends::RedisBackend;
#[cfg(feature = "s3")]
use super::backends::S3Backend;
use super::config::{BackendType, StorageConfig};
use super::error::StorageResult;
use super::traits::UnifiedStorage;

/// Factory for creating storage instances
pub struct StorageFactory;

impl StorageFactory {
    /// Create storage from environment configuration
    pub async fn from_env() -> StorageResult<Box<dyn UnifiedStorage>> {
        let config =
            StorageConfig::from_env().map_err(super::error::StorageError::configuration)?;

        Self::from_config(&config).await
    }

    /// Create storage from explicit configuration
    pub async fn from_config(config: &StorageConfig) -> StorageResult<Box<dyn UnifiedStorage>> {
        match &config.backend {
            BackendType::File => {
                let backend = FileBackend::new(config).await?;
                Ok(Box::new(backend))
            }
            BackendType::Memory => {
                let backend = MemoryBackend::new(config)?;
                Ok(Box::new(backend))
            }
            #[cfg(feature = "postgres")]
            BackendType::Postgres => {
                if let BackendConfig::Postgres(ref pg_config) = config.backend_config {
                    let backend = PostgresBackend::new(pg_config).await?;
                    Ok(Box::new(backend))
                } else {
                    Err(super::error::StorageError::configuration(
                        "Invalid backend configuration for PostgreSQL",
                    ))
                }
            }
            #[cfg(not(feature = "postgres"))]
            BackendType::Postgres => {
                Err(super::error::StorageError::configuration(
                    "PostgreSQL backend not enabled. Enable with --features postgres",
                ))
            }
            #[cfg(feature = "redis")]
            BackendType::Redis => {
                if let BackendConfig::Redis(ref redis_config) = config.backend_config {
                    let backend = RedisBackend::new(redis_config).await?;
                    Ok(Box::new(backend))
                } else {
                    Err(super::error::StorageError::configuration(
                        "Invalid backend configuration for Redis",
                    ))
                }
            }
            #[cfg(not(feature = "redis"))]
            BackendType::Redis => {
                Err(super::error::StorageError::configuration(
                    "Redis backend not enabled. Enable with --features redis",
                ))
            }
            #[cfg(feature = "s3")]
            BackendType::S3 => {
                if let BackendConfig::S3(ref s3_config) = config.backend_config {
                    let backend = S3Backend::new(s3_config).await?;
                    Ok(Box::new(backend))
                } else {
                    Err(super::error::StorageError::configuration(
                        "Invalid backend configuration for S3",
                    ))
                }
            }
            #[cfg(not(feature = "s3"))]
            BackendType::S3 => {
                Err(super::error::StorageError::configuration(
                    "S3 backend not enabled. Enable with --features s3",
                ))
            }
        }
    }

    /// Create a test storage instance (memory backend)
    #[cfg(test)]
    pub fn create_test_storage() -> Box<dyn UnifiedStorage> {
        let config = StorageConfig {
            backend: BackendType::Memory,
            backend_config: BackendConfig::Memory(Default::default()),
            ..Default::default()
        };

        Box::new(MemoryBackend::new(&config).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_factory_creates_file_backend() {
        use crate::storage::config::FileConfig;

        let config = StorageConfig {
            backend: BackendType::File,
            backend_config: BackendConfig::File(FileConfig {
                base_dir: std::env::temp_dir().join("prodigy-test"),
                use_global: false,
                enable_file_locks: true,
                max_file_size: 1024 * 1024,
                enable_compression: false,
            }),
            ..Default::default()
        };

        let storage = StorageFactory::from_config(&config).await.unwrap();
        let health = storage.health_check().await.unwrap();
        assert!(health.healthy);
        assert_eq!(health.backend_type, "file");
    }

    #[test]
    fn test_factory_creates_memory_backend() {
        let storage = StorageFactory::create_test_storage();
        // The storage should be usable
        let _ = storage.session_storage();
        let _ = storage.event_storage();
    }
}
