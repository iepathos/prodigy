//! Storage migration utility for transferring data between backends

use super::error::{StorageError, StorageResult};
use super::traits::UnifiedStorage;
use super::types::{CheckpointFilter, EventFilter, SessionFilter};
use chrono::{DateTime, Utc};
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{error, info, warn};

/// Migration configuration
pub struct MigrationConfig {
    /// Maximum parallel operations
    pub max_parallel: usize,
    /// Batch size for bulk operations
    pub batch_size: usize,
    /// Whether to verify data after migration
    pub verify: bool,
    /// Whether to delete source data after successful migration
    pub delete_source: bool,
    /// Progress reporting
    pub show_progress: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            max_parallel: 10,
            batch_size: 100,
            verify: true,
            delete_source: false,
            show_progress: true,
        }
    }
}

/// Migration statistics
#[derive(Debug, Default)]
pub struct MigrationStats {
    pub sessions_migrated: u64,
    pub events_migrated: u64,
    pub job_states_migrated: u64,
    pub checkpoints_migrated: u64,
    pub errors: Vec<String>,
    pub duration: std::time::Duration,
}

/// Storage migration utility
pub struct StorageMigrator {
    config: MigrationConfig,
}

impl Default for StorageMigrator {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageMigrator {
    /// Create a new migrator with default configuration
    pub fn new() -> Self {
        Self {
            config: Default::default(),
        }
    }

    /// Create a new migrator with custom configuration
    pub fn with_config(config: MigrationConfig) -> Self {
        Self { config }
    }

    /// Migrate all data from source to destination
    pub async fn migrate_all(
        &self,
        source: &dyn UnifiedStorage,
        destination: &dyn UnifiedStorage,
        repository: &str,
    ) -> StorageResult<MigrationStats> {
        info!("Starting full migration for repository: {}", repository);
        let start = std::time::Instant::now();
        let mut stats = MigrationStats::default();

        // Verify both backends are healthy
        self.verify_backends(source, destination).await?;

        // Migrate sessions
        match self.migrate_sessions(source, destination, repository).await {
            Ok(count) => stats.sessions_migrated = count,
            Err(e) => {
                error!("Failed to migrate sessions: {}", e);
                stats.errors.push(format!("Sessions: {}", e));
            }
        }

        // Migrate events
        match self.migrate_events(source, destination, repository).await {
            Ok(count) => stats.events_migrated = count,
            Err(e) => {
                error!("Failed to migrate events: {}", e);
                stats.errors.push(format!("Events: {}", e));
            }
        }

        // Migrate job states and checkpoints
        match self
            .migrate_state_data(source, destination, repository)
            .await
        {
            Ok((states, checkpoints)) => {
                stats.job_states_migrated = states;
                stats.checkpoints_migrated = checkpoints;
            }
            Err(e) => {
                error!("Failed to migrate state data: {}", e);
                stats.errors.push(format!("State: {}", e));
            }
        }

        stats.duration = start.elapsed();

        info!(
            "Migration completed in {:?}. Sessions: {}, Events: {}, Job States: {}, Checkpoints: {}, Errors: {}",
            stats.duration,
            stats.sessions_migrated,
            stats.events_migrated,
            stats.job_states_migrated,
            stats.checkpoints_migrated,
            stats.errors.len()
        );

        Ok(stats)
    }

    /// Verify both backends are healthy
    async fn verify_backends(
        &self,
        source: &dyn UnifiedStorage,
        destination: &dyn UnifiedStorage,
    ) -> StorageResult<()> {
        let source_health = source.health_check().await?;
        if !source_health.healthy {
            return Err(StorageError::configuration(format!(
                "Source backend unhealthy: {:?}",
                source_health.errors
            )));
        }

        let dest_health = destination.health_check().await?;
        if !dest_health.healthy {
            return Err(StorageError::configuration(format!(
                "Destination backend unhealthy: {:?}",
                dest_health.errors
            )));
        }

        info!(
            "Backends healthy - Source: {}, Destination: {}",
            source_health.backend_type, dest_health.backend_type
        );

        Ok(())
    }

    /// Migrate sessions
    async fn migrate_sessions(
        &self,
        source: &dyn UnifiedStorage,
        destination: &dyn UnifiedStorage,
        repository: &str,
    ) -> StorageResult<u64> {
        info!("Migrating sessions for repository: {}", repository);

        let session_ids = source
            .session_storage()
            .list(SessionFilter::default())
            .await?;
        let total = session_ids.len() as u64;

        if total == 0 {
            info!("No sessions to migrate");
            return Ok(0);
        }

        // Load all sessions
        let mut sessions = Vec::new();
        for id in session_ids {
            if let Some(session) = source.session_storage().load(&id).await? {
                sessions.push(session);
            }
        }

        let progress = if self.config.show_progress {
            let pb = ProgressBar::new(total);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40} {pos}/{len} {msg}")
                    .unwrap(),
            );
            pb.set_message("Migrating sessions");
            Some(pb)
        } else {
            None
        };

        let mut migrated = 0u64;

        // Process sessions in batches, but sequentially due to trait object limitations
        for session in sessions {
            let session_id = session.id.0.clone();

            match destination.session_storage().save(&session).await {
                Ok(_) => {
                    migrated += 1;
                    if let Some(ref pb) = progress {
                        pb.inc(1);
                    }
                }
                Err(e) => {
                    warn!("Failed to migrate session {}: {}", session_id, e);
                }
            }
        }

        if let Some(pb) = progress {
            pb.finish_with_message(format!("Migrated {} sessions", migrated));
        }

        // Verify if requested
        if self.config.verify && migrated > 0 {
            self.verify_sessions(source, destination, repository)
                .await?;
        }

        Ok(migrated)
    }

    /// Migrate events
    async fn migrate_events(
        &self,
        source: &dyn UnifiedStorage,
        destination: &dyn UnifiedStorage,
        repository: &str,
    ) -> StorageResult<u64> {
        info!("Migrating events for repository: {}", repository);

        // TODO: EventStorage trait doesn't have list_job_ids method
        // Need to determine how to list all job IDs for migration
        let job_ids: Vec<String> = vec![];
        let total_jobs = job_ids.len();

        if total_jobs == 0 {
            info!("No events to migrate");
            return Ok(0);
        }

        let progress = if self.config.show_progress {
            let pb = ProgressBar::new(total_jobs as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40} {pos}/{len} {msg}")
                    .unwrap(),
            );
            pb.set_message("Migrating events");
            Some(pb)
        } else {
            None
        };

        let mut total_events = 0u64;

        for job_id in &job_ids {
            match self
                .migrate_job_events(source, destination, repository, job_id)
                .await
            {
                Ok(count) => {
                    total_events += count;
                    if let Some(ref pb) = progress {
                        pb.inc(1);
                    }
                }
                Err(e) => {
                    warn!("Failed to migrate events for job {}: {}", job_id, e);
                }
            }
        }

        if let Some(pb) = progress {
            pb.finish_with_message(format!("Migrated {} events", total_events));
        }

        Ok(total_events)
    }

    /// Migrate events for a specific job
    async fn migrate_job_events(
        &self,
        source: &dyn UnifiedStorage,
        destination: &dyn UnifiedStorage,
        _repository: &str,
        job_id: &str,
    ) -> StorageResult<u64> {
        // Query events for this job
        let filter = EventFilter {
            job_id: Some(job_id.to_string()),
            ..Default::default()
        };

        let mut event_stream = source.event_storage().query(filter).await?;
        let mut events = Vec::new();

        // Collect events from stream
        use futures::StreamExt;
        while let Some(event) = event_stream.next().await {
            events.push(event?);
        }

        let count = events.len() as u64;

        // Append all events to destination
        if !events.is_empty() {
            destination.event_storage().append(events).await?;
        }

        Ok(count)
    }

    /// Migrate state data (job states and checkpoints)
    async fn migrate_state_data(
        &self,
        source: &dyn UnifiedStorage,
        destination: &dyn UnifiedStorage,
        repository: &str,
    ) -> StorageResult<(u64, u64)> {
        info!("Migrating state data for repository: {}", repository);

        let job_states_migrated = 0u64;
        let mut checkpoints_migrated = 0u64;

        // Migrate checkpoints
        let checkpoint_info_list = source
            .checkpoint_storage()
            .list(CheckpointFilter::default())
            .await?;

        let progress = if self.config.show_progress && !checkpoint_info_list.is_empty() {
            let pb = ProgressBar::new(checkpoint_info_list.len() as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40} {pos}/{len} {msg}")
                    .unwrap(),
            );
            pb.set_message("Migrating checkpoints");
            Some(pb)
        } else {
            None
        };

        for checkpoint_info in checkpoint_info_list {
            let checkpoint_id = checkpoint_info.id;
            match source.checkpoint_storage().load(&checkpoint_id).await {
                Ok(Some(checkpoint)) => {
                    if destination
                        .checkpoint_storage()
                        .save(&checkpoint)
                        .await
                        .is_ok()
                    {
                        checkpoints_migrated += 1;
                        if let Some(ref pb) = progress {
                            pb.inc(1);
                        }
                    }
                }
                Ok(None) => {
                    warn!("Checkpoint {} not found during migration", checkpoint_id);
                }
                Err(e) => {
                    warn!("Failed to load checkpoint {}: {}", checkpoint_id, e);
                }
            }
        }

        if let Some(pb) = progress {
            pb.finish_with_message(format!("Migrated {} checkpoints", checkpoints_migrated));
        }

        Ok((job_states_migrated, checkpoints_migrated))
    }

    /// Verify sessions were migrated correctly
    async fn verify_sessions(
        &self,
        source: &dyn UnifiedStorage,
        destination: &dyn UnifiedStorage,
        _repository: &str,
    ) -> StorageResult<()> {
        info!("Verifying session migration");

        let source_sessions = source
            .session_storage()
            .list(SessionFilter::default())
            .await?;
        let dest_sessions = destination
            .session_storage()
            .list(SessionFilter::default())
            .await?;

        let source_ids: std::collections::HashSet<_> =
            source_sessions.iter().map(|s| s.0.clone()).collect();

        let dest_ids: std::collections::HashSet<_> =
            dest_sessions.iter().map(|s| s.0.clone()).collect();

        let missing: Vec<_> = source_ids.difference(&dest_ids).collect();

        if !missing.is_empty() {
            return Err(StorageError::conflict(format!(
                "Verification failed: {} sessions missing in destination",
                missing.len()
            )));
        }

        info!("Session verification passed");
        Ok(())
    }

    /// Perform incremental migration since a specific timestamp
    pub async fn migrate_incremental(
        &self,
        source: &dyn UnifiedStorage,
        destination: &dyn UnifiedStorage,
        repository: &str,
        since: DateTime<Utc>,
    ) -> StorageResult<MigrationStats> {
        info!(
            "Starting incremental migration for repository: {} since {}",
            repository, since
        );

        let start = std::time::Instant::now();
        let mut stats = MigrationStats::default();

        // Verify backends
        self.verify_backends(source, destination).await?;

        // Migrate recent events
        // TODO: EventStorage trait doesn't have list_job_ids method
        // Need to determine how to list all job IDs for migration
        let job_ids: Vec<String> = vec![];

        for job_id in &job_ids {
            match source
                .event_storage()
                .query(EventFilter {
                    job_id: Some(job_id.to_string()),
                    after: Some(since),
                    ..Default::default()
                })
                .await
            {
                Ok(mut event_stream) => {
                    let mut events_batch = Vec::new();

                    // Collect events from stream
                    use futures::StreamExt;
                    while let Some(event_result) = event_stream.next().await {
                        if let Ok(event) = event_result {
                            events_batch.push(event);
                        }
                    }

                    // Append events in batch
                    if !events_batch.is_empty() {
                        let batch_size = events_batch.len();
                        if destination
                            .event_storage()
                            .append(events_batch)
                            .await
                            .is_ok()
                        {
                            stats.events_migrated += batch_size as u64;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read events for job {}: {}", job_id, e);
                    stats
                        .errors
                        .push(format!("Events for job {}: {}", job_id, e));
                }
            }
        }

        // Migrate recent sessions
        let session_ids = source
            .session_storage()
            .list(SessionFilter::default())
            .await?;
        for id in session_ids {
            if let Some(session) = source.session_storage().load(&id).await? {
                if session.started_at >= since
                    && destination.session_storage().save(&session).await.is_ok()
                {
                    stats.sessions_migrated += 1;
                }
            }
        }

        stats.duration = start.elapsed();

        info!(
            "Incremental migration completed in {:?}. Sessions: {}, Events: {}",
            stats.duration, stats.sessions_migrated, stats.events_migrated
        );

        Ok(stats)
    }
}

/// CLI interface for migration tool
pub mod cli {
    use super::*;
    use crate::storage::{config::StorageConfig, factory::StorageFactory};

    /// Run migration from command line arguments
    pub async fn run_migration(
        source_config: StorageConfig,
        dest_config: StorageConfig,
        repository: &str,
        incremental: Option<DateTime<Utc>>,
    ) -> StorageResult<()> {
        info!("Initializing storage backends");

        let source = StorageFactory::from_config(&source_config).await?;
        let destination = StorageFactory::from_config(&dest_config).await?;

        let migrator = StorageMigrator::new();

        let stats = if let Some(since) = incremental {
            migrator
                .migrate_incremental(&*source, &*destination, repository, since)
                .await?
        } else {
            migrator
                .migrate_all(&*source, &*destination, repository)
                .await?
        };

        // Print summary
        println!("\n=== Migration Summary ===");
        println!("Duration: {:?}", stats.duration);
        println!("Sessions migrated: {}", stats.sessions_migrated);
        println!("Events migrated: {}", stats.events_migrated);
        println!("Job states migrated: {}", stats.job_states_migrated);
        println!("Checkpoints migrated: {}", stats.checkpoints_migrated);

        if !stats.errors.is_empty() {
            println!("\nErrors encountered:");
            for error in &stats.errors {
                println!("  - {}", error);
            }
        }

        if stats.errors.is_empty() {
            println!("\nMigration completed successfully!");
            Ok(())
        } else {
            Err(StorageError::conflict(format!(
                "Migration completed with {} errors",
                stats.errors.len()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        backends::MemoryBackend,
        config::{BackendConfig, BackendType, MemoryConfig, StorageConfig},
        types::{PersistedSession, SessionId, SessionState},
    };
    use std::collections::HashMap;

    fn create_test_session(id: &str) -> PersistedSession {
        PersistedSession {
            id: SessionId(id.to_string()),
            state: SessionState::InProgress,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            iterations_completed: 0,
            files_changed: 0,
            worktree_name: Some(format!("worktree-{}", id)),
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_migrate_sessions() {
        let config = StorageConfig {
            backend: BackendType::Memory,
            backend_config: BackendConfig::Memory(MemoryConfig::default()),
            connection_pool_size: 10,
            retry_policy: Default::default(),
            timeout: std::time::Duration::from_secs(30),
            enable_locking: true,
            enable_cache: false,
            cache_config: Default::default(),
        };

        let source = MemoryBackend::new(&config).unwrap();
        let destination = MemoryBackend::new(&config).unwrap();

        // Add test sessions to source
        for i in 1..=5 {
            let session = create_test_session(&format!("session-{}", i));
            source.session_storage().save(&session).await.unwrap();
        }

        // Migrate
        let migrator = StorageMigrator::new();
        let stats = migrator
            .migrate_all(&source, &destination, "test-repo")
            .await
            .unwrap();

        assert_eq!(stats.sessions_migrated, 5);

        // Verify all sessions in destination
        let dest_sessions = destination
            .session_storage()
            .list(SessionFilter::default())
            .await
            .unwrap();
        assert_eq!(dest_sessions.len(), 5);
    }

    #[tokio::test]
    async fn test_incremental_migration() {
        let config = StorageConfig {
            backend: BackendType::Memory,
            backend_config: BackendConfig::Memory(MemoryConfig::default()),
            connection_pool_size: 10,
            retry_policy: Default::default(),
            timeout: std::time::Duration::from_secs(30),
            enable_locking: true,
            enable_cache: false,
            cache_config: Default::default(),
        };

        let source = MemoryBackend::new(&config).unwrap();
        let destination = MemoryBackend::new(&config).unwrap();

        // Add old session
        let old_session = create_test_session("old-session");
        source.session_storage().save(&old_session).await.unwrap();

        let cutoff = Utc::now();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Add new session after cutoff
        let new_session = create_test_session("new-session");
        source.session_storage().save(&new_session).await.unwrap();

        // Migrate only recent data
        let migrator = StorageMigrator::new();
        let stats = migrator
            .migrate_incremental(&source, &destination, "test-repo", cutoff)
            .await
            .unwrap();

        assert_eq!(stats.sessions_migrated, 1); // Only new session

        let dest_sessions = destination
            .session_storage()
            .list(SessionFilter::default())
            .await
            .unwrap();
        assert_eq!(dest_sessions.len(), 1);
        assert_eq!(dest_sessions[0].0, "new-session");
    }
}
