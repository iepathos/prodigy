//! Storage factory for creating storage instances

use super::error::StorageResult;
use super::global::GlobalStorage;

/// Factory for creating storage instances
pub struct StorageFactory;

impl StorageFactory {
    /// Create storage from environment configuration
    pub async fn from_env() -> StorageResult<GlobalStorage> {
        GlobalStorage::new()
    }

    /// Create a test storage instance
    #[cfg(test)]
    pub fn create_test_storage() -> StorageResult<GlobalStorage> {
        GlobalStorage::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_factory_creates_global_storage() {
        let storage = StorageFactory::from_env().await.unwrap();
        let health = storage.health_check().await.unwrap();
        assert!(health.healthy);
    }

    #[test]
    fn test_factory_creates_test_storage() {
        let storage = StorageFactory::create_test_storage().unwrap();
        // The storage should be usable
        let _ = storage.session_storage();
        let _ = storage.event_storage();
    }
}
