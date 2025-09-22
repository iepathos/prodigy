//! Storage migration utility - simplified for global storage only

use super::error::StorageResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

/// Migration configuration
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub batch_size: usize,
    pub repositories: Vec<String>,
    pub progress: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            repositories: vec![],
            progress: true,
        }
    }
}

/// Migration statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MigrationStats {
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub sessions_migrated: usize,
    pub events_migrated: usize,
    pub checkpoints_migrated: usize,
    pub dlq_items_migrated: usize,
    pub workflows_migrated: usize,
    pub errors_encountered: Vec<String>,
}

/// Storage migrator - simplified version since we only use GlobalStorage
pub struct StorageMigrator {
    _config: MigrationConfig,
    stats: MigrationStats,
}

impl StorageMigrator {
    /// Create a new migrator with the given configuration
    pub fn new(config: MigrationConfig) -> Self {
        Self {
            _config: config,
            stats: MigrationStats::default(),
        }
    }

    /// Get the migration statistics
    pub fn stats(&self) -> &MigrationStats {
        &self.stats
    }

    /// No-op migration since we only have GlobalStorage now
    pub async fn migrate(&mut self, _repository: &str) -> StorageResult<()> {
        info!("Migration not needed - using GlobalStorage");
        self.stats.started_at = Some(Utc::now());
        self.stats.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Update migration stats
    pub fn update_stats(&mut self, update: impl FnOnce(&mut MigrationStats)) {
        update(&mut self.stats);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_config_default() {
        let config = MigrationConfig::default();
        assert_eq!(config.batch_size, 1000);
        assert!(config.repositories.is_empty());
        assert!(config.progress);
    }

    #[test]
    fn test_migration_config_custom() {
        let config = MigrationConfig {
            batch_size: 500,
            repositories: vec!["repo1".to_string(), "repo2".to_string()],
            progress: false,
        };
        assert_eq!(config.batch_size, 500);
        assert_eq!(config.repositories.len(), 2);
        assert_eq!(config.repositories[0], "repo1");
        assert_eq!(config.repositories[1], "repo2");
        assert!(!config.progress);
    }

    #[test]
    fn test_migration_stats_default() {
        let stats = MigrationStats::default();
        assert!(stats.started_at.is_none());
        assert!(stats.completed_at.is_none());
        assert_eq!(stats.sessions_migrated, 0);
        assert_eq!(stats.events_migrated, 0);
        assert_eq!(stats.checkpoints_migrated, 0);
        assert_eq!(stats.dlq_items_migrated, 0);
        assert_eq!(stats.workflows_migrated, 0);
        assert!(stats.errors_encountered.is_empty());
    }

    #[test]
    fn test_migration_stats_serialization() {
        let stats = MigrationStats {
            started_at: Some(Utc::now()),
            sessions_migrated: 10,
            events_migrated: 100,
            errors_encountered: vec!["test error".to_string()],
            ..Default::default()
        };

        // Serialize
        let json = serde_json::to_string(&stats).unwrap();

        // Deserialize
        let deserialized: MigrationStats = serde_json::from_str(&json).unwrap();

        assert!(deserialized.started_at.is_some());
        assert_eq!(deserialized.sessions_migrated, 10);
        assert_eq!(deserialized.events_migrated, 100);
        assert_eq!(deserialized.errors_encountered.len(), 1);
        assert_eq!(deserialized.errors_encountered[0], "test error");
    }

    #[test]
    fn test_storage_migrator_creation() {
        let config = MigrationConfig::default();
        let migrator = StorageMigrator::new(config);

        let stats = migrator.stats();
        assert!(stats.started_at.is_none());
        assert!(stats.completed_at.is_none());
        assert_eq!(stats.sessions_migrated, 0);
    }

    #[tokio::test]
    async fn test_migrate_noop() {
        let config = MigrationConfig::default();
        let mut migrator = StorageMigrator::new(config);

        // Should succeed as a no-op
        let result = migrator.migrate("test-repo").await;
        assert!(result.is_ok());

        // Stats should be updated
        let stats = migrator.stats();
        assert!(stats.started_at.is_some());
        assert!(stats.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_migrate_multiple_repos() {
        let config = MigrationConfig {
            repositories: vec!["repo1".to_string(), "repo2".to_string()],
            ..Default::default()
        };
        let mut migrator = StorageMigrator::new(config);

        // Migrate multiple repos
        assert!(migrator.migrate("repo1").await.is_ok());
        assert!(migrator.migrate("repo2").await.is_ok());

        let stats = migrator.stats();
        assert!(stats.started_at.is_some());
        assert!(stats.completed_at.is_some());
    }

    #[test]
    fn test_update_stats() {
        let config = MigrationConfig::default();
        let mut migrator = StorageMigrator::new(config);

        // Update stats using the update_stats method
        migrator.update_stats(|stats| {
            stats.sessions_migrated = 5;
            stats.events_migrated = 50;
            stats.errors_encountered.push("test error".to_string());
        });

        let stats = migrator.stats();
        assert_eq!(stats.sessions_migrated, 5);
        assert_eq!(stats.events_migrated, 50);
        assert_eq!(stats.errors_encountered.len(), 1);
    }

    #[test]
    fn test_migration_config_clone() {
        let config1 = MigrationConfig {
            batch_size: 2000,
            repositories: vec!["test".to_string()],
            progress: false,
        };

        let config2 = config1.clone();
        assert_eq!(config2.batch_size, 2000);
        assert_eq!(config2.repositories.len(), 1);
        assert_eq!(config2.repositories[0], "test");
        assert!(!config2.progress);
    }

    #[test]
    fn test_migration_stats_clone() {
        let stats1 = MigrationStats {
            sessions_migrated: 10,
            events_migrated: 100,
            ..Default::default()
        };

        let stats2 = stats1.clone();
        assert_eq!(stats2.sessions_migrated, 10);
        assert_eq!(stats2.events_migrated, 100);
    }

    #[tokio::test]
    async fn test_migrate_with_empty_repository_name() {
        let config = MigrationConfig::default();
        let mut migrator = StorageMigrator::new(config);

        // Should succeed even with empty repository name
        let result = migrator.migrate("").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_migrate_updates_timestamps() {
        let config = MigrationConfig::default();
        let mut migrator = StorageMigrator::new(config);

        // Initially no timestamps
        assert!(migrator.stats().started_at.is_none());
        assert!(migrator.stats().completed_at.is_none());

        // After migration
        migrator.migrate("test").await.unwrap();

        // Timestamps should be set
        let stats = migrator.stats();
        assert!(stats.started_at.is_some());
        assert!(stats.completed_at.is_some());

        // Started should be before or equal to completed
        if let (Some(started), Some(completed)) = (stats.started_at, stats.completed_at) {
            assert!(started <= completed);
        }
    }
}
