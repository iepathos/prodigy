---
number: 95
title: Storage Migration and Dual-Mode Operation
category: storage
priority: high
status: draft
dependencies: [93, 94]
created: 2025-01-17
---

# Specification 95: Storage Migration and Dual-Mode Operation

**Category**: storage
**Priority**: high
**Status**: draft
**Dependencies**: [93 - Storage Abstraction Layer, 94 - PostgreSQL Storage Backend]

## Context

With both file-based and PostgreSQL storage backends available through the unified abstraction layer, we need robust tools and processes to migrate existing data from file storage to database storage without disruption. Organizations running Prodigy need to transition from local file storage to database-backed storage for container deployments while maintaining data integrity, ensuring zero downtime, and providing rollback capabilities if issues arise.

The migration process must handle large datasets efficiently, validate data integrity throughout the transfer, support incremental migration for active systems, and provide clear progress tracking and error recovery. Additionally, a dual-mode operation capability is needed during transition periods where both storage backends operate simultaneously to ensure smooth cutover.

## Objective

Develop comprehensive migration tools and dual-mode storage capabilities that enable seamless transition from file-based to database storage. The solution must support zero-downtime migration, data validation, progress tracking, and rollback capabilities while allowing gradual migration of different data types and providing clear operational visibility throughout the process.

## Requirements

### Functional Requirements
- CLI tool for initiating and managing storage migrations
- Support for full and incremental migration modes
- Data validation with checksums and record counts
- Progress tracking with resumable migration capability
- Dual-write mode for zero-downtime migration
- Selective migration by data type (sessions, events, etc.)
- Dry-run mode to preview migration actions
- Rollback capability to revert to file storage
- Migration status reporting and monitoring
- Automatic retry for transient failures

### Non-Functional Requirements
- Migration performance >10,000 records/second
- Memory usage <1GB for migrations up to 1TB data
- Zero data loss during migration process
- Support for migration pause and resume
- Clear error messages with recovery instructions
- Minimal impact on running system (<5% CPU overhead)
- Support for parallel migration of independent data types
- Audit log of all migration activities

## Acceptance Criteria

- [ ] Migration CLI command successfully transfers all data types
- [ ] Data validation confirms 100% accuracy after migration
- [ ] Dual-write mode maintains consistency between backends
- [ ] Progress tracking shows real-time migration status
- [ ] Migration can be paused and resumed without data loss
- [ ] Rollback successfully reverts to file storage
- [ ] Incremental migration handles ongoing changes
- [ ] Performance meets throughput requirements
- [ ] Memory usage stays within limits for large datasets
- [ ] Integration tests verify migration scenarios
- [ ] Documentation covers all migration procedures

## Technical Details

### Implementation Approach

1. **Migration Orchestrator**
   ```rust
   pub struct StorageMigrator {
       source: Box<dyn UnifiedStorage>,
       target: Box<dyn UnifiedStorage>,
       config: MigrationConfig,
       state: Arc<RwLock<MigrationState>>,
       metrics: Arc<MigrationMetrics>,
   }

   impl StorageMigrator {
       pub async fn new(config: MigrationConfig) -> Result<Self> {
           let source = StorageFactory::create(&config.source_config)?;
           let target = StorageFactory::create(&config.target_config)?;

           let state = Self::load_or_create_state(&config).await?;

           Ok(Self {
               source,
               target,
               config,
               state: Arc::new(RwLock::new(state)),
               metrics: Arc::new(MigrationMetrics::new()),
           })
       }

       pub async fn migrate(&self) -> Result<MigrationReport> {
           info!("Starting migration from {:?} to {:?}",
                 self.config.source_config.backend_type,
                 self.config.target_config.backend_type);

           // Migrate each data type
           let mut report = MigrationReport::new();

           if self.config.migrate_sessions {
               report.sessions = self.migrate_sessions().await?;
           }

           if self.config.migrate_events {
               report.events = self.migrate_events().await?;
           }

           if self.config.migrate_checkpoints {
               report.checkpoints = self.migrate_checkpoints().await?;
           }

           if self.config.migrate_dlq {
               report.dlq = self.migrate_dlq().await?;
           }

           Ok(report)
       }
   }
   ```

2. **Dual-Mode Storage Wrapper**
   ```rust
   pub struct DualModeStorage {
       primary: Box<dyn UnifiedStorage>,
       secondary: Box<dyn UnifiedStorage>,
       mode: DualMode,
       consistency_checker: Arc<ConsistencyChecker>,
   }

   pub enum DualMode {
       /// Read from primary, write to both
       WriteThrough,
       /// Read from both and compare
       ReadValidation,
       /// Write to both, read from primary, log differences
       ShadowMode,
       /// Gradual migration with percentage-based routing
       Progressive { read_percentage: u8 },
   }

   #[async_trait]
   impl UnifiedStorage for DualModeStorage {
       async fn save_session(&self, session: &PersistedSession) -> Result<()> {
           match self.mode {
               DualMode::WriteThrough | DualMode::ShadowMode => {
                   // Write to both backends
                   let (r1, r2) = tokio::join!(
                       self.primary.save_session(session),
                       self.secondary.save_session(session)
                   );

                   // Primary must succeed
                   r1?;

                   // Log secondary failures but don't fail operation
                   if let Err(e) = r2 {
                       warn!("Secondary storage write failed: {}", e);
                       self.consistency_checker.record_divergence(
                           "session",
                           &session.id,
                           &e
                       ).await;
                   }

                   Ok(())
               }
               _ => self.primary.save_session(session).await,
           }
       }

       async fn load_session(&self, id: &SessionId) -> Result<Option<PersistedSession>> {
           match self.mode {
               DualMode::ReadValidation => {
                   let (r1, r2) = tokio::join!(
                       self.primary.load_session(id),
                       self.secondary.load_session(id)
                   );

                   let primary = r1?;
                   let secondary = r2.ok();

                   if let (Some(p), Some(s)) = (&primary, &secondary) {
                       if !self.consistency_checker.validate_sessions(p, &s) {
                           warn!("Session inconsistency detected for {}", id);
                           self.consistency_checker.record_inconsistency(
                               "session",
                               id,
                               p,
                               &s
                           ).await;
                       }
                   }

                   Ok(primary)
               }
               DualMode::Progressive { read_percentage } => {
                   if rand::random::<u8>() < read_percentage {
                       self.secondary.load_session(id).await
                   } else {
                       self.primary.load_session(id).await
                   }
               }
               _ => self.primary.load_session(id).await,
           }
       }
   }
   ```

3. **Streaming Migration for Large Datasets**
   ```rust
   impl StorageMigrator {
       async fn migrate_events(&self) -> Result<MigrationStats> {
           let mut stats = MigrationStats::new("events");
           let mut checkpoint = self.load_checkpoint("events").await?;

           // Create event stream from source
           let filter = EventFilter {
               after: checkpoint.last_timestamp,
               ..Default::default()
           };

           let mut stream = self.source.event_storage().stream(filter).await?;
           let mut batch = Vec::with_capacity(self.config.batch_size);

           while let Some(event) = stream.next().await {
               let event = event?;
               batch.push(event.clone());

               if batch.len() >= self.config.batch_size {
                   self.write_event_batch(&batch, &mut stats).await?;
                   checkpoint.last_timestamp = Some(event.timestamp);
                   checkpoint.records_migrated += batch.len();
                   self.save_checkpoint("events", &checkpoint).await?;
                   batch.clear();

                   // Check for pause request
                   if self.should_pause().await {
                       info!("Migration paused at {} records", stats.total);
                       break;
                   }
               }

               stats.total += 1;
               self.metrics.update(&stats);
           }

           // Write remaining batch
           if !batch.is_empty() {
               self.write_event_batch(&batch, &mut stats).await?;
           }

           Ok(stats)
       }

       async fn write_event_batch(
           &self,
           batch: &[EventRecord],
           stats: &mut MigrationStats
       ) -> Result<()> {
           let start = Instant::now();

           match self.target.event_storage().append(batch.to_vec()).await {
               Ok(_) => {
                   stats.successful += batch.len();
                   stats.total_duration += start.elapsed();
               }
               Err(e) => {
                   stats.failed += batch.len();
                   if self.config.fail_fast {
                       return Err(e);
                   }
                   warn!("Failed to migrate batch: {}", e);
                   // Record failed items for retry
                   self.record_failed_batch(batch, &e).await?;
               }
           }

           Ok(())
       }
   }
   ```

4. **Data Validation**
   ```rust
   pub struct DataValidator {
       source: Box<dyn UnifiedStorage>,
       target: Box<dyn UnifiedStorage>,
       config: ValidationConfig,
   }

   impl DataValidator {
       pub async fn validate_migration(&self) -> Result<ValidationReport> {
           let mut report = ValidationReport::new();

           // Validate record counts
           report.count_validation = self.validate_counts().await?;

           // Sample-based content validation
           report.content_validation = self.validate_content_sample().await?;

           // Checksum validation for critical data
           report.checksum_validation = self.validate_checksums().await?;

           // Query consistency validation
           report.query_validation = self.validate_queries().await?;

           Ok(report)
       }

       async fn validate_content_sample(&self) -> Result<ContentValidation> {
           let mut validation = ContentValidation::new();

           // Sample sessions
           let source_sessions = self.source.session_storage()
               .list(SessionFilter::default())
               .await?;

           let sample_size = (source_sessions.len() as f64 *
                              self.config.sample_percentage / 100.0) as usize;

           for id in source_sessions.iter().take(sample_size) {
               let source_session = self.source.session_storage().load(id).await?;
               let target_session = self.target.session_storage().load(id).await?;

               match (source_session, target_session) {
                   (Some(s), Some(t)) => {
                       if !self.sessions_equal(&s, &t) {
                           validation.add_mismatch("session", id, s, t);
                       } else {
                           validation.matches += 1;
                       }
                   }
                   (Some(_), None) => validation.add_missing("session", id),
                   (None, Some(_)) => validation.add_extra("session", id),
                   _ => {}
               }
           }

           Ok(validation)
       }
   }
   ```

### Architecture Changes

- Add migration module to storage subsystem
- Implement progress tracking with persistent state
- Add validation framework for data comparison
- Create dual-mode storage wrapper
- Integrate with CLI for migration commands

### Data Structures

```rust
pub struct MigrationConfig {
    pub source_config: StorageConfig,
    pub target_config: StorageConfig,
    pub migrate_sessions: bool,
    pub migrate_events: bool,
    pub migrate_checkpoints: bool,
    pub migrate_dlq: bool,
    pub batch_size: usize,
    pub parallel_workers: usize,
    pub fail_fast: bool,
    pub validation_mode: ValidationMode,
    pub checkpoint_interval: Duration,
}

pub struct MigrationState {
    pub id: Uuid,
    pub status: MigrationStatus,
    pub started_at: DateTime<Utc>,
    pub last_checkpoint: DateTime<Utc>,
    pub checkpoints: HashMap<String, MigrationCheckpoint>,
    pub stats: MigrationStats,
}

pub struct MigrationReport {
    pub duration: Duration,
    pub sessions: MigrationStats,
    pub events: MigrationStats,
    pub checkpoints: MigrationStats,
    pub dlq: MigrationStats,
    pub validation: Option<ValidationReport>,
    pub errors: Vec<MigrationError>,
}
```

### APIs and Interfaces

```rust
// CLI Commands
pub enum MigrationCommand {
    Start {
        source: StorageConfig,
        target: StorageConfig,
        options: MigrationOptions,
    },
    Status {
        migration_id: Option<Uuid>,
    },
    Pause {
        migration_id: Uuid,
    },
    Resume {
        migration_id: Uuid,
    },
    Rollback {
        migration_id: Uuid,
        force: bool,
    },
    Validate {
        source: StorageConfig,
        target: StorageConfig,
        sample_percentage: f64,
    },
}
```

## Dependencies

- **Prerequisites**: [93 - Storage Abstraction Layer, 94 - PostgreSQL Storage Backend]
- **Affected Components**: CLI, storage module, configuration system
- **External Dependencies**:
  - indicatif - Progress bars and status display
  - tokio-stream - Async streaming utilities
  - checksums - Data integrity validation
  - clap - CLI argument parsing

## Testing Strategy

- **Unit Tests**: Mock storage backends for migration logic
- **Integration Tests**: File to PostgreSQL migration scenarios
- **Performance Tests**: Large dataset migration benchmarks
- **Failure Tests**: Network interruption, storage failures
- **Validation Tests**: Data integrity verification
- **Dual-Mode Tests**: Consistency during simultaneous operations
- **Rollback Tests**: Successful reversion scenarios

## Documentation Requirements

- **Migration Guide**: Step-by-step migration procedures
- **Operations Manual**: Monitoring and troubleshooting
- **CLI Reference**: All migration commands and options
- **Best Practices**: Recommendations for production migrations
- **Recovery Procedures**: Handling migration failures

## Implementation Notes

- Use streaming to handle large datasets without memory issues
- Implement checkpoint persistence for resumable migrations
- Add comprehensive logging for audit trail
- Consider read-repair for inconsistencies found during validation
- Implement rate limiting to avoid overwhelming target storage
- Use parallel workers for independent data types
- Add metrics collection for operational visibility
- Consider using CDC for incremental migration of live systems

## Migration and Compatibility

- Support migration in both directions (file to DB, DB to file)
- Maintain backward compatibility with existing file formats
- Version all migration state for future upgrades
- Provide clear upgrade path documentation
- Support heterogeneous storage during transition
- Enable gradual rollout with feature flags