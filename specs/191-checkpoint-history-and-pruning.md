---
number: 191
title: Checkpoint History and Automatic Pruning
category: storage
priority: medium
status: draft
dependencies: [184]
created: 2025-11-29
---

# Specification 191: Checkpoint History and Automatic Pruning

**Category**: storage
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 184 (Unified Checkpoint System)

## Context

Current checkpoint system overwrites the previous checkpoint on each save, with no history or backup mechanism. This creates several problems:

### Issues with Single Checkpoint Model

1. **No Corruption Recovery**: If a checkpoint write is partially completed or corrupted, the previous valid checkpoint is lost
2. **No Audit Trail**: Cannot examine checkpoint evolution for debugging
3. **Limited Resume Options**: Cannot resume from an earlier checkpoint if the latest is problematic
4. **No Protection Against Bugs**: Checkpoint format bugs can make workflows unresumable

### Unbounded Storage Growth

Without automatic cleanup:
- Completed workflows leave checkpoints indefinitely
- Failed workflow checkpoints accumulate
- Test workflows create checkpoint clutter
- Disk usage grows unbounded over time

### Real-World Scenarios

**Scenario 1: Checkpoint Corruption**
- Step 5 completes, checkpoint saved
- Step 6 fails, attempts to save checkpoint
- Disk full error during write
- Checkpoint partially written, corrupted
- Previous (step 5) checkpoint overwritten
- Workflow cannot resume (all checkpoints lost)

**Scenario 2: Test Clutter**
- Developer runs workflow 50 times during development
- Each creates checkpoint
- 50 checkpoints accumulate in `~/.prodigy/sessions/`
- Manual cleanup required
- Disk space wasted

## Objective

Implement checkpoint history and automatic pruning that:
1. Maintains timestamped checkpoint history for fallback
2. Enables recovery from checkpoint corruption
3. Provides audit trail of workflow execution
4. Automatically cleans up old checkpoints
5. Configures retention policies per workflow type
6. Prevents unbounded storage growth

## Requirements

### Functional Requirements

#### FR1: Checkpoint History Storage
- **MUST** create timestamped backup of each checkpoint before overwrite
- **MUST** store history at `~/.prodigy/sessions/{id}/history/checkpoint-{timestamp}.json`
- **MUST** maintain chronological ordering of checkpoints
- **MUST** limit history to configurable count (default: 10)
- **MUST** preserve compressed format in history

#### FR2: Checkpoint Fallback
- **MUST** detect corrupted latest checkpoint on load
- **MUST** automatically fallback to most recent valid checkpoint from history
- **MUST** log fallback with clear warning
- **MUST** validate integrity hash before fallback
- **MUST** continue fallback through history until valid checkpoint found

#### FR3: History Pruning
- **MUST** remove oldest checkpoint when history limit reached
- **MUST** preserve most recent N checkpoints (configurable)
- **MUST** use LRU policy for history management
- **MUST** atomic delete to prevent corruption
- **MUST** log history pruning operations

#### FR4: Automatic Cleanup
- **MUST** clean completed workflow checkpoints after configurable duration (default: 7 days)
- **MUST** clean failed workflow checkpoints after configurable duration (default: 30 days)
- **MUST** preserve checkpoints for actively running workflows
- **MUST** support immediate cleanup via configuration
- **MUST** support manual cleanup command

#### FR5: Cleanup Policies
- **MUST** support per-workflow cleanup policies
- **MUST** support global default policies
- **MUST** allow disabling cleanup for debugging
- **MUST** provide policy for test workflows vs. production
- **MUST** honor retention policies during cleanup

#### FR6: Audit and Inspection
- **MUST** list all checkpoints for a session
- **MUST** show checkpoint history with timestamps
- **MUST** display checkpoint sizes and compression ratios
- **MUST** support checkpoint diff between history entries
- **MUST** enable checkpoint export for debugging

### Non-Functional Requirements

#### NFR1: Performance
- History save MUST NOT increase checkpoint latency >10%
- Pruning MUST complete asynchronously without blocking
- Cleanup MUST NOT impact active workflow execution
- History lookup MUST complete in <100ms

#### NFR2: Reliability
- History operations MUST be atomic
- Pruning failures MUST NOT corrupt checkpoint history
- Cleanup MUST gracefully handle missing files
- Fallback MUST preserve latest valid checkpoint

#### NFR3: Storage Efficiency
- History MUST NOT exceed configured storage limit
- Compressed checkpoints MUST remain compressed in history
- Cleanup MUST reclaim disk space promptly
- Old history MUST be pruned before hitting disk limits

## Acceptance Criteria

### Checkpoint History

- [ ] **AC1**: History created on checkpoint save
  - Checkpoint saved at step 5
  - Previous checkpoint (step 4) backed up to history
  - History file: `~/.prodigy/sessions/session-abc/history/checkpoint-20251129-143022.json`
  - Latest checkpoint: `~/.prodigy/sessions/session-abc/checkpoint.json`
  - Both files identical to step 4 checkpoint

- [ ] **AC2**: History bounded to limit
  - History limit is 10 checkpoints
  - 11th checkpoint saved
  - Oldest checkpoint removed from history
  - 10 most recent checkpoints retained
  - Disk space reclaimed

- [ ] **AC3**: History preserves compression
  - Compressed checkpoint saved (5MB → 1MB)
  - Checkpoint backed up to history
  - History file is compressed (1MB)
  - No re-compression overhead
  - Original compression metadata preserved

### Corruption Recovery

- [ ] **AC4**: Automatic fallback on corruption
  - Latest checkpoint corrupted (invalid JSON)
  - Resume attempted
  - Corruption detected (integrity hash mismatch)
  - Fallback to previous checkpoint from history
  - Warning logged: "Latest checkpoint corrupted, using history checkpoint from 2025-11-29 14:30:22"
  - Resume successful from step 5

- [ ] **AC5**: Multiple fallback attempts
  - Latest checkpoint corrupted
  - Previous checkpoint also corrupted
  - Second-previous checkpoint valid
  - System tries 3 checkpoints before finding valid one
  - Resume successful from step 4
  - All corruption logged

- [ ] **AC6**: Fallback exhaustion
  - All checkpoints in history corrupted
  - Resume attempted
  - All fallback attempts fail
  - Clear error: "No valid checkpoint found (tried 10 checkpoints)"
  - Suggestion to start workflow fresh
  - No infinite fallback loop

### Automatic Cleanup

- [ ] **AC7**: Completed workflow cleanup
  - Workflow completes successfully 7 days ago
  - Retention policy: 7 days for completed
  - Cleanup job runs
  - Checkpoint and history deleted
  - Disk space reclaimed
  - No impact on active workflows

- [ ] **AC8**: Failed workflow retention
  - Workflow failed 35 days ago
  - Retention policy: 30 days for failed
  - Cleanup job runs
  - Checkpoint and history deleted
  - Failed workflows retained longer for debugging

- [ ] **AC9**: Active workflow preservation
  - Workflow running (in-progress)
  - Cleanup job runs
  - Checkpoint NOT deleted
  - History preserved
  - Only completed/failed workflows cleaned

### Cleanup Policies

- [ ] **AC10**: Per-workflow cleanup policy
  - Workflow A: `cleanup: { retention_days: 1 }`
  - Workflow B: `cleanup: { retention_days: 30 }`
  - Global default: 7 days
  - Workflow A cleaned after 1 day
  - Workflow B cleaned after 30 days
  - Policies respected per workflow

- [ ] **AC11**: Cleanup disabled for debugging
  - Workflow config: `cleanup: { enabled: false }`
  - Workflow completes
  - Cleanup job runs
  - Checkpoint NOT deleted
  - Preserved indefinitely for debugging

- [ ] **AC12**: Test workflow immediate cleanup
  - Workflow tagged as test: `test: true`
  - Test policy: immediate cleanup on completion
  - Workflow completes
  - Checkpoint deleted immediately
  - No retention for test workflows

### Audit and Inspection

- [ ] **AC13**: List checkpoint history
  - Command: `prodigy checkpoint history session-abc123`
  - Output shows:
    ```
    Latest: checkpoint.json (2025-11-29 14:45:22, 1.2MB, step 8)
    History:
      - checkpoint-20251129-144022.json (2025-11-29 14:40:22, 1.1MB, step 7)
      - checkpoint-20251129-143522.json (2025-11-29 14:35:22, 1.0MB, step 6)
      - checkpoint-20251129-143022.json (2025-11-29 14:30:22, 950KB, step 5)
      ... (7 more)
    ```
  - Chronological ordering
  - Size and timestamp displayed

- [ ] **AC14**: Checkpoint diff
  - Command: `prodigy checkpoint diff session-abc123 --from step-5 --to step-7`
  - Shows differences:
    ```
    Variables changed:
      + analysis_result: "success"
      ~ iteration: 5 → 7
    Steps added:
      + Step 6: shell: "make test"
      + Step 7: claude: "/review"
    ```
  - Clear visualization of changes
  - Useful for debugging state evolution

- [ ] **AC15**: Checkpoint export
  - Command: `prodigy checkpoint export session-abc123 --output debug.json`
  - Latest checkpoint exported to file
  - Decompressed if compressed
  - Pretty-printed JSON
  - Useful for bug reports

## Technical Details

### Implementation Approach

#### 1. Checkpoint History Management

```rust
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

pub struct CheckpointHistory {
    session_dir: PathBuf,
    history_dir: PathBuf,
    max_history: usize,
}

impl CheckpointHistory {
    pub fn new(session_id: &str, max_history: usize) -> Result<Self> {
        let session_dir = global_sessions_dir()?.join(session_id);
        let history_dir = session_dir.join("history");

        fs::create_dir_all(&history_dir)?;

        Ok(Self {
            session_dir,
            history_dir,
            max_history,
        })
    }

    /// Save current checkpoint to history before overwriting
    pub async fn archive_current(&self) -> Result<()> {
        let current_path = self.session_dir.join("checkpoint.json");

        if !current_path.exists() {
            return Ok(()); // No current checkpoint to archive
        }

        // Generate timestamped filename
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
        let history_path = self.history_dir.join(format!("checkpoint-{}.json", timestamp));

        // Copy current to history (preserves compression)
        fs::copy(&current_path, &history_path).await?;

        tracing::debug!("Archived checkpoint to history: {}", history_path.display());

        // Prune history if needed
        self.prune_history().await?;

        Ok(())
    }

    /// Remove oldest checkpoints to maintain size limit
    async fn prune_history(&self) -> Result<()> {
        let mut entries = self.list_history_files().await?;

        // Sort by timestamp (oldest first)
        entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        // Remove oldest entries beyond limit
        if entries.len() > self.max_history {
            let to_remove = entries.len() - self.max_history;

            for entry in entries.iter().take(to_remove) {
                tracing::info!("Pruning old checkpoint from history: {}", entry.path.display());
                fs::remove_file(&entry.path).await?;
            }
        }

        Ok(())
    }

    /// List all history checkpoints
    pub async fn list_history(&self) -> Result<Vec<HistoryEntry>> {
        let mut entries = self.list_history_files().await?;
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); // Newest first
        Ok(entries)
    }

    async fn list_history_files(&self) -> Result<Vec<HistoryEntry>> {
        let mut entries = vec![];

        let mut dir = fs::read_dir(&self.history_dir).await?;

        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                // Parse timestamp from filename
                if let Some(timestamp) = self.parse_timestamp_from_filename(&path) {
                    let metadata = fs::metadata(&path).await?;

                    entries.push(HistoryEntry {
                        path,
                        timestamp,
                        size: metadata.len(),
                    });
                }
            }
        }

        Ok(entries)
    }

    fn parse_timestamp_from_filename(&self, path: &Path) -> Option<DateTime<Utc>> {
        let filename = path.file_stem()?.to_str()?;

        // Parse "checkpoint-20251129-143022" format
        let timestamp_str = filename.strip_prefix("checkpoint-")?;
        let datetime_str = timestamp_str.replace('-', "");

        DateTime::parse_from_str(&format!("{}+0000", datetime_str), "%Y%m%d%H%M%S%z")
            .ok()
            .map(|dt| dt.into())
    }
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub path: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub size: u64,
}
```

#### 2. Corruption Recovery with Fallback

```rust
pub struct CheckpointLoader {
    history: CheckpointHistory,
    validator: CheckpointValidator,
}

impl CheckpointLoader {
    pub async fn load_with_fallback(&self, session_id: &str) -> Result<WorkflowCheckpoint> {
        let session_dir = global_sessions_dir()?.join(session_id);
        let current_path = session_dir.join("checkpoint.json");

        // Try latest checkpoint first
        match self.try_load_checkpoint(&current_path).await {
            Ok(checkpoint) => {
                tracing::debug!("Loaded latest checkpoint");
                return Ok(checkpoint);
            }
            Err(e) => {
                tracing::warn!("Latest checkpoint failed to load: {}", e);
            }
        }

        // Fallback to history
        let history_entries = self.history.list_history().await?;

        tracing::info!("Attempting fallback to {} history checkpoints", history_entries.len());

        for (i, entry) in history_entries.iter().enumerate() {
            tracing::debug!("Trying history checkpoint {}: {}", i + 1, entry.path.display());

            match self.try_load_checkpoint(&entry.path).await {
                Ok(checkpoint) => {
                    tracing::warn!(
                        "Recovered from history checkpoint: {} ({})",
                        entry.timestamp,
                        entry.path.display()
                    );

                    return Ok(checkpoint);
                }
                Err(e) => {
                    tracing::warn!("History checkpoint {} failed: {}", i + 1, e);
                }
            }
        }

        Err(anyhow!(
            "No valid checkpoint found (tried {} checkpoints)",
            history_entries.len() + 1
        ))
    }

    async fn try_load_checkpoint(&self, path: &Path) -> Result<WorkflowCheckpoint> {
        // Read file
        let data = fs::read(path).await?;

        // Parse envelope
        let envelope: CheckpointEnvelope = serde_json::from_slice(&data)?;

        // Decompress if needed
        let checkpoint_data = if envelope.compressed {
            self.decompress(&envelope.data)?
        } else {
            envelope.data
        };

        // Deserialize checkpoint
        let checkpoint: WorkflowCheckpoint = serde_json::from_slice(&checkpoint_data)?;

        // Validate integrity
        self.validator.validate_integrity(&checkpoint, &envelope.integrity_hash)?;

        Ok(checkpoint)
    }
}
```

#### 3. Automatic Cleanup

```rust
pub struct CheckpointCleaner {
    retention_policies: RetentionPolicies,
}

impl CheckpointCleaner {
    pub async fn cleanup_old_checkpoints(&self) -> Result<CleanupReport> {
        let sessions_dir = global_sessions_dir()?;
        let mut report = CleanupReport::default();

        let mut dir = fs::read_dir(&sessions_dir).await?;

        while let Some(entry) = dir.next_entry().await? {
            let session_dir = entry.path();

            if !session_dir.is_dir() {
                continue;
            }

            if let Some(session_id) = session_dir.file_name().and_then(|n| n.to_str()) {
                match self.cleanup_session(session_id).await {
                    Ok(cleaned) => {
                        if cleaned {
                            report.cleaned_count += 1;
                        } else {
                            report.retained_count += 1;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to cleanup session {}: {}", session_id, e);
                        report.error_count += 1;
                    }
                }
            }
        }

        Ok(report)
    }

    async fn cleanup_session(&self, session_id: &str) -> Result<bool> {
        // Load session state
        let session = self.load_session(session_id).await?;

        // Check if cleanup should happen
        if !self.should_cleanup(&session) {
            return Ok(false);
        }

        // Get retention policy
        let policy = self.get_policy(&session);

        // Check age against policy
        let age = Utc::now() - session.completed_at.unwrap_or(session.created_at);

        if age.num_days() < policy.retention_days as i64 {
            return Ok(false); // Too new to clean
        }

        tracing::info!(
            "Cleaning up session {} (age: {} days, policy: {} days)",
            session_id,
            age.num_days(),
            policy.retention_days
        );

        // Delete checkpoint and history
        let session_dir = global_sessions_dir()?.join(session_id);
        fs::remove_dir_all(&session_dir).await?;

        Ok(true)
    }

    fn should_cleanup(&self, session: &UnifiedSession) -> bool {
        match session.status {
            SessionStatus::Running | SessionStatus::Paused => false,  // Active
            SessionStatus::Completed | SessionStatus::Failed => true,  // Cleanable
            SessionStatus::Cancelled => true,
        }
    }

    fn get_policy(&self, session: &UnifiedSession) -> &RetentionPolicy {
        // Check workflow-specific policy
        if let Some(workflow_id) = &session.workflow_data.as_ref().and_then(|w| w.workflow_id.as_ref()) {
            if let Some(policy) = self.retention_policies.workflows.get(workflow_id) {
                return policy;
            }
        }

        // Check test workflow policy
        if session.metadata.get("test").and_then(|v| v.as_bool()).unwrap_or(false) {
            return &self.retention_policies.test_default;
        }

        // Use status-based default
        match session.status {
            SessionStatus::Completed => &self.retention_policies.completed_default,
            SessionStatus::Failed => &self.retention_policies.failed_default,
            _ => &self.retention_policies.completed_default,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    pub enabled: bool,
    pub retention_days: u32,
}

#[derive(Debug, Clone)]
pub struct RetentionPolicies {
    pub completed_default: RetentionPolicy,
    pub failed_default: RetentionPolicy,
    pub test_default: RetentionPolicy,
    pub workflows: HashMap<String, RetentionPolicy>,
}

#[derive(Debug, Default)]
pub struct CleanupReport {
    pub cleaned_count: usize,
    pub retained_count: usize,
    pub error_count: usize,
}
```

### Architecture Changes

**New modules:**
- `src/cook/workflow/checkpoint/history.rs` - History management
- `src/cook/workflow/checkpoint/cleanup.rs` - Automatic cleanup
- `src/cook/workflow/checkpoint/recovery.rs` - Corruption recovery

**Modified components:**
- `CheckpointManager` - Integrate history archival
- Configuration system - Add cleanup policies
- CLI commands - Add history inspection commands

### Configuration Schema

```yaml
checkpoint:
  history:
    enabled: true
    max_count: 10  # Keep 10 most recent checkpoints

  cleanup:
    enabled: true

    # Retention by workflow status
    completed_retention_days: 7
    failed_retention_days: 30

    # Test workflows cleaned immediately
    test_retention_days: 0

    # Per-workflow overrides
    workflows:
      critical-production-workflow:
        retention_days: 90

      dev-test-workflow:
        retention_days: 1

      never-delete-workflow:
        enabled: false  # Keep indefinitely
```

## Dependencies

- **Prerequisites**: Spec 184 (Unified Checkpoint System)
- **External Dependencies**: None (uses standard library)
- **Affected Components**: CheckpointManager, CLI, configuration

## Testing Strategy

### Unit Tests
- History archival and pruning
- Fallback logic with multiple corruptions
- Cleanup policy evaluation
- Timestamp parsing
- Retention age calculations

### Integration Tests
- End-to-end history creation
- Corruption recovery scenarios
- Automatic cleanup execution
- History inspection commands
- Multi-corruption fallback

### Performance Tests
- History save latency impact
- Cleanup operation speed
- Large history lookup performance
- Concurrent cleanup operations

## Documentation Requirements

### Code Documentation
- Document fallback behavior
- Explain cleanup policies
- Describe history format
- Add examples for retention policies

### User Documentation
- Checkpoint history usage guide
- Cleanup configuration guide
- Recovery from corruption procedure
- Debugging with checkpoint history

### Architecture Updates
- Document history architecture
- Update storage layout diagrams
- Add cleanup job scheduling details

## Implementation Notes

### History Storage Format

History checkpoints are exact copies of the checkpoint at the time of archival:
- No re-serialization (preserves exact bytes)
- Compression preserved (no re-compression)
- Metadata included in filename (timestamp)
- Directory structure mirrors main checkpoint

### Cleanup Scheduling

Cleanup runs:
- On session completion (immediate for test workflows)
- Via cron job (daily for production)
- Manually via `prodigy checkpoint cleanup` command
- Background thread during long-running workflows

### Fallback Strategy

Fallback attempts checkpoints in order:
1. Latest checkpoint (checkpoint.json)
2. Most recent history checkpoint
3. Second-most recent history checkpoint
4. ... continues through all history
5. Fails if no valid checkpoint found

## Migration and Compatibility

### Backward Compatibility

- Existing checkpoints work without history
- History created prospectively (not retroactively)
- No breaking changes to checkpoint format
- Graceful handling of missing history

### Migration Path

1. Deploy with history disabled (default: off)
2. Enable history for new checkpoints
3. Monitor history disk usage
4. Enable cleanup with conservative retention
5. Tune policies based on usage patterns

### Rollback Strategy

- Disable history creation via configuration
- Existing history preserved but not updated
- Cleanup can be disabled independently
- No data loss on rollback
