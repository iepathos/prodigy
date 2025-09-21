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
        Ok(())
    }
}
