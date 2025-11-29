---
number: 192
title: Atomic Checkpoint Writes with Retry
category: storage
priority: high
status: draft
dependencies: [184]
created: 2025-11-29
---

# Specification 192: Atomic Checkpoint Writes with Retry

**Category**: storage
**Priority**: high
**Status**: draft
**Dependencies**: Spec 184 (Unified Checkpoint System)

## Context

Current checkpoint writes are not atomic and lack resilience against transient failures:

### Non-Atomic Write Problems

**Direct file write pattern:**
```rust
// Current (BROKEN)
fs::write("checkpoint.json", data)?;
```

**Problems:**
1. **Partial writes on crash**: Process crash mid-write leaves corrupted checkpoint
2. **Overwrite before validate**: Overwrites previous checkpoint before validating new one
3. **No rollback**: Failure leaves no valid checkpoint (new broken, old gone)
4. **Race conditions**: Concurrent writes can interleave bytes

### Transient Failure Issues

Network filesystems (NFS, CIFS) and cloud storage have transient failures:
- Temporary network hiccups (5-10 seconds)
- Brief permission errors
- Filesystem metadata delays
- Lock contention

Current code fails immediately on first error, losing checkpoint data unnecessarily.

### Real-World Scenarios

**Scenario 1: Process Crash During Write**
```
1. Start writing checkpoint (10KB written of 50KB)
2. Process crashes (OOM, SIGKILL, power loss)
3. checkpoint.json contains 10KB of partial data
4. Resume attempts to load checkpoint
5. JSON parse fails (incomplete data)
6. No previous checkpoint to fallback to
7. Workflow cannot resume
```

**Scenario 2: Network Filesystem Hiccup**
```
1. Workflow on step 50 of 100
2. Attempt to save checkpoint after step 50
3. NFS connection hiccup (transient)
4. Write fails immediately
5. Previous checkpoint (step 49) still valid
6. But new progress lost unnecessarily
7. Could have succeeded with retry
```

**Scenario 3: Concurrent Access**
```
1. Workflow A writes to checkpoint.json
2. Workflow B reads checkpoint.json simultaneously
3. Race condition: B reads partial write
4. Corruption detected
5. Both workflows fail
```

## Objective

Implement atomic checkpoint writes with retry resilience:
1. Write to temporary file, rename to final (atomic)
2. Validate checkpoint before committing
3. Preserve previous checkpoint until new one validated
4. Retry transient failures with exponential backoff
5. Prevent concurrent write corruption
6. Enable safe rollback on failure

## Requirements

### Functional Requirements

#### FR1: Atomic Write Operations
- **MUST** write checkpoint to temporary file first
- **MUST** validate temporary file before committing
- **MUST** atomically rename temp file to final location
- **MUST** preserve previous checkpoint until new one committed
- **MUST** clean up temporary files on failure
- **MUST** ensure atomicity even on process crash

#### FR2: Write Validation
- **MUST** compute integrity hash of checkpoint before write
- **MUST** verify integrity hash after write (read back)
- **MUST** validate JSON parsing of written checkpoint
- **MUST** compare written size to expected size
- **MUST** fail write if validation fails

#### FR3: Retry on Transient Failures
- **MUST** detect transient vs permanent errors
- **MUST** retry transient failures up to 3 times (configurable)
- **MUST** use exponential backoff (100ms, 500ms, 2s)
- **MUST** log retry attempts with failure reason
- **MUST** fail permanently after retry exhaustion

#### FR4: Concurrent Access Protection
- **MUST** acquire exclusive write lock before write
- **MUST** release lock after commit or failure
- **MUST** detect stale locks from crashed processes
- **MUST** timeout lock acquisition after 30s
- **MUST** prevent concurrent writes to same checkpoint

#### FR5: Cleanup and Recovery
- **MUST** clean up temporary files after successful write
- **MUST** clean up temporary files after failed write
- **MUST** detect orphaned temporary files on startup
- **MUST** recover from incomplete atomic writes
- **MUST** log cleanup operations

### Non-Functional Requirements

#### NFR1: Performance
- Atomic write MUST NOT increase latency >20ms (P95)
- Lock acquisition MUST complete in <10ms (P95)
- Retry backoff MUST NOT delay success path
- Cleanup MUST run asynchronously

#### NFR2: Reliability
- Atomicity MUST survive process crash
- Atomicity MUST survive power loss
- Locks MUST be released on process death
- Temporary files MUST NOT accumulate unbounded

#### NFR3: Observability
- All retry attempts MUST be logged
- Lock contention MUST be visible in metrics
- Cleanup operations MUST be tracked
- Validation failures MUST be logged with details

## Acceptance Criteria

### Atomic Write Guarantees

- [ ] **AC1**: Write-validate-commit pattern
  - Checkpoint created
  - Written to `checkpoint.tmp.{uuid}.json`
  - Integrity hash computed and verified
  - JSON parsing validated
  - Renamed to `checkpoint.json` atomically
  - Temporary file deleted
  - Write completes successfully

- [ ] **AC2**: Validation failure prevents commit
  - Checkpoint written to temp file
  - Integrity hash mismatch detected
  - Rename NOT performed
  - Previous checkpoint unchanged
  - Temporary file cleaned up
  - Error returned: "Checkpoint validation failed: integrity hash mismatch"

- [ ] **AC3**: Process crash during write
  - Write to temp file starts
  - Process crashes mid-write (SIGKILL)
  - Temp file contains partial data
  - Previous `checkpoint.json` unchanged and valid
  - Next run detects orphaned temp file
  - Orphaned temp cleaned up automatically
  - Resume loads previous valid checkpoint

- [ ] **AC4**: Atomic rename preserves previous on failure
  - Previous checkpoint at `checkpoint.json`
  - New checkpoint written to temp
  - Rename operation fails (permission error)
  - Previous checkpoint still valid and readable
  - No corruption introduced
  - Can retry write or resume from previous

### Retry Resilience

- [ ] **AC5**: Transient failure retried successfully
  - First write attempt fails (network timeout)
  - Error classified as transient
  - Retry attempt 1 after 100ms delay
  - Retry succeeds
  - Checkpoint saved successfully
  - Total attempts: 2

- [ ] **AC6**: Multiple retries with backoff
  - First attempt fails (transient)
  - Retry 1 after 100ms - fails
  - Retry 2 after 500ms - fails
  - Retry 3 after 2s - succeeds
  - Checkpoint saved after 4 attempts
  - Total delay: ~2.6s
  - Logged: "Checkpoint saved after 4 attempts (2.6s)"

- [ ] **AC7**: Permanent failure after retry exhaustion
  - First attempt fails (disk full)
  - Retry 1 after 100ms - fails
  - Retry 2 after 500ms - fails
  - Retry 3 after 2s - fails
  - Error returned: "Checkpoint write failed after 4 attempts: disk full"
  - Previous checkpoint preserved
  - Workflow fails with clear error

- [ ] **AC8**: Transient vs permanent error classification
  - ENOSPC (disk full): permanent, no retry
  - ETIMEDOUT (network): transient, retry
  - EACCES (permission): permanent, no retry
  - EIO (I/O error): transient, retry
  - Correct classification prevents useless retries

### Concurrent Access Protection

- [ ] **AC9**: Exclusive lock prevents concurrent writes
  - Process A acquires write lock
  - Process B attempts to acquire lock
  - Process B blocks for up to 30s
  - Process A completes and releases lock
  - Process B acquires lock and writes
  - No corruption from concurrent access

- [ ] **AC10**: Stale lock recovery
  - Process A acquires lock
  - Process A crashes without releasing lock
  - Process B attempts to acquire lock
  - Stale lock detected (PID not running)
  - Lock forcibly released
  - Process B acquires lock and writes

- [ ] **AC11**: Lock timeout prevents deadlock
  - Process A holds lock indefinitely
  - Process B attempts to acquire lock
  - 30 second timeout expires
  - Error returned: "Checkpoint write timeout: lock held by PID 12345"
  - Process B fails gracefully
  - No indefinite blocking

### Cleanup and Recovery

- [ ] **AC12**: Automatic orphan cleanup on startup
  - Temporary files from previous runs:
    - `checkpoint.tmp.abc123.json` (3 days old)
    - `checkpoint.tmp.def456.json` (1 hour old)
  - CheckpointManager initialized
  - Orphaned files detected
  - Old orphans cleaned up
  - Logged: "Cleaned 2 orphaned temporary checkpoint files"

- [ ] **AC13**: Cleanup after successful write
  - Checkpoint written to temp file
  - Validated successfully
  - Renamed to final location
  - Temporary file deleted immediately
  - No orphans left behind

- [ ] **AC14**: Cleanup after failed write
  - Checkpoint written to temp file
  - Validation fails
  - Write operation aborted
  - Temporary file deleted
  - No orphans left behind

## Technical Details

### Implementation Approach

#### 1. Atomic Write with Stillwater Bracket Pattern

```rust
use stillwater::bracket;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

pub struct AtomicCheckpointWriter {
    checkpoint_path: PathBuf,
    retry_policy: RetryPolicy,
    lock_manager: FileLockManager,
}

impl AtomicCheckpointWriter {
    /// Write checkpoint atomically with validation and retry
    pub async fn write_atomic(&self, checkpoint: &WorkflowCheckpoint) -> Result<()> {
        // Acquire exclusive lock
        let _lock_guard = self.lock_manager.acquire_exclusive(&self.checkpoint_path).await?;

        // Execute atomic write with bracket pattern
        let result = bracket(
            // Acquire: Create temp file
            || self.create_temp_file(),
            // Use: Write and validate
            |temp_path| async move {
                self.write_and_validate(&temp_path, checkpoint).await
            },
            // Release: Commit or cleanup
            |temp_path, write_result| async move {
                match write_result {
                    Ok(()) => {
                        // Commit: atomic rename
                        self.commit_checkpoint(&temp_path).await?;
                        self.cleanup_temp_file(&temp_path).await;
                    }
                    Err(_) => {
                        // Cleanup: remove temp file
                        self.cleanup_temp_file(&temp_path).await;
                    }
                }
                write_result
            },
        )
        .await;

        result
    }

    fn create_temp_file(&self) -> Result<PathBuf> {
        let temp_name = format!("checkpoint.tmp.{}.json", Uuid::new_v4());
        let temp_path = self.checkpoint_path.parent()
            .ok_or_else(|| anyhow!("Invalid checkpoint path"))?
            .join(temp_name);

        Ok(temp_path)
    }

    async fn write_and_validate(
        &self,
        temp_path: &PathBuf,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<()> {
        // Retry wrapper
        self.retry_policy
            .execute(|| async {
                // Serialize checkpoint
                let checkpoint_data = serde_json::to_vec_pretty(checkpoint)?;

                // Compute integrity hash
                let integrity_hash = compute_integrity_hash(checkpoint)?;

                // Create envelope with metadata
                let envelope = CheckpointEnvelope {
                    version: CHECKPOINT_VERSION,
                    data: checkpoint_data.clone(),
                    integrity_hash: integrity_hash.clone(),
                    size: checkpoint_data.len(),
                };

                // Serialize envelope
                let envelope_data = serde_json::to_vec(&envelope)?;

                // Write to temp file
                fs::write(temp_path, &envelope_data).await?;

                // Validate written data
                self.validate_written_checkpoint(temp_path, &envelope).await?;

                Ok(())
            })
            .await
    }

    async fn validate_written_checkpoint(
        &self,
        path: &PathBuf,
        expected: &CheckpointEnvelope,
    ) -> Result<()> {
        // Read back
        let written_data = fs::read(path).await?;

        // Verify size
        if written_data.len() != expected.size {
            return Err(CheckpointError::ValidationFailed {
                reason: format!(
                    "Size mismatch: expected {}, got {}",
                    expected.size,
                    written_data.len()
                ),
            }
            .into());
        }

        // Parse envelope
        let written_envelope: CheckpointEnvelope = serde_json::from_slice(&written_data)?;

        // Verify integrity hash
        if written_envelope.integrity_hash != expected.integrity_hash {
            return Err(CheckpointError::ValidationFailed {
                reason: "Integrity hash mismatch".to_string(),
            }
            .into());
        }

        // Verify checkpoint can be parsed
        let _checkpoint: WorkflowCheckpoint = serde_json::from_slice(&written_envelope.data)?;

        Ok(())
    }

    async fn commit_checkpoint(&self, temp_path: &PathBuf) -> Result<()> {
        // Atomic rename
        fs::rename(temp_path, &self.checkpoint_path).await?;

        tracing::info!("Checkpoint committed atomically: {}", self.checkpoint_path.display());

        Ok(())
    }

    async fn cleanup_temp_file(&self, temp_path: &PathBuf) {
        if let Err(e) = fs::remove_file(temp_path).await {
            tracing::warn!("Failed to cleanup temp file {}: {}", temp_path.display(), e);
        }
    }
}
```

#### 2. Retry Policy with Exponential Backoff

```rust
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: f64,
}

impl RetryPolicy {
    pub fn default() -> Self {
        Self {
            max_attempts: 4,        // Initial + 3 retries
            initial_delay_ms: 100,  // 100ms first retry
            max_delay_ms: 5000,     // Max 5s delay
            multiplier: 5.0,        // 100ms, 500ms, 2500ms
        }
    }

    pub async fn execute<F, T, E>(&self, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> futures::future::BoxFuture<'static, Result<T, E>>,
        E: std::error::Error,
    {
        let mut attempt = 0;
        let mut delay_ms = self.initial_delay_ms;

        loop {
            attempt += 1;

            match operation().await {
                Ok(result) => {
                    if attempt > 1 {
                        tracing::info!("Operation succeeded on attempt {}", attempt);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    // Check if retryable
                    if !self.is_transient_error(&e) {
                        tracing::error!("Permanent error, not retrying: {}", e);
                        return Err(e);
                    }

                    // Check if exhausted retries
                    if attempt >= self.max_attempts {
                        tracing::error!("Operation failed after {} attempts", attempt);
                        return Err(e);
                    }

                    // Log retry
                    tracing::warn!(
                        "Attempt {} failed (transient): {}. Retrying in {}ms...",
                        attempt,
                        e,
                        delay_ms
                    );

                    // Delay before retry
                    sleep(Duration::from_millis(delay_ms)).await;

                    // Exponential backoff
                    delay_ms = (delay_ms as f64 * self.multiplier) as u64;
                    delay_ms = delay_ms.min(self.max_delay_ms);
                }
            }
        }
    }

    fn is_transient_error<E: std::error::Error>(&self, error: &E) -> bool {
        let error_str = error.to_string().to_lowercase();

        // Network-related errors are transient
        if error_str.contains("timeout")
            || error_str.contains("connection")
            || error_str.contains("temporarily unavailable")
            || error_str.contains("resource temporarily unavailable")
        {
            return true;
        }

        // I/O errors may be transient
        if error_str.contains("i/o error") || error_str.contains("io error") {
            return true;
        }

        // Permanent errors
        if error_str.contains("disk full")
            || error_str.contains("no space")
            || error_str.contains("permission denied")
            || error_str.contains("access denied")
        {
            return false;
        }

        // Default: not transient
        false
    }
}
```

#### 3. File Locking for Concurrent Protection

```rust
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::process;
use fs2::FileExt;

pub struct FileLockManager {
    timeout_secs: u64,
}

impl FileLockManager {
    pub async fn acquire_exclusive(&self, checkpoint_path: &PathBuf) -> Result<LockGuard> {
        let lock_path = checkpoint_path.with_extension("lock");

        // Create or open lock file
        let lock_file = File::create(&lock_path)?;

        // Try to acquire exclusive lock with timeout
        let start = Instant::now();

        loop {
            match lock_file.try_lock_exclusive() {
                Ok(()) => {
                    tracing::debug!("Acquired exclusive lock: {}", lock_path.display());

                    return Ok(LockGuard {
                        file: lock_file,
                        path: lock_path,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Lock held by another process

                    // Check timeout
                    if start.elapsed().as_secs() > self.timeout_secs {
                        // Try to detect stale lock
                        if let Ok(stale) = self.is_stale_lock(&lock_path) {
                            if stale {
                                tracing::warn!("Detected stale lock, forcibly releasing");
                                // Remove stale lock file
                                let _ = fs::remove_file(&lock_path);
                                continue;
                            }
                        }

                        return Err(CheckpointError::LockTimeout {
                            path: lock_path,
                            timeout_secs: self.timeout_secs,
                        }
                        .into());
                    }

                    // Wait and retry
                    sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
    }

    fn is_stale_lock(&self, lock_path: &PathBuf) -> Result<bool> {
        // Read PID from lock file
        let pid_str = fs::read_to_string(lock_path)?;
        let pid: u32 = pid_str.trim().parse()?;

        // Check if process is still running
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            match kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                Ok(_) => Ok(false), // Process exists
                Err(_) => Ok(true), // Process doesn't exist
            }
        }

        #[cfg(not(unix))]
        {
            // Windows: check if process exists
            // Simplified - assume stale after 5 minutes
            let metadata = fs::metadata(lock_path)?;
            let age = metadata.modified()?.elapsed()?;
            Ok(age.as_secs() > 300) // 5 minutes
        }
    }
}

pub struct LockGuard {
    file: File,
    path: PathBuf,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Release lock
        let _ = self.file.unlock();

        // Clean up lock file
        let _ = fs::remove_file(&self.path);

        tracing::debug!("Released lock: {}", self.path.display());
    }
}
```

#### 4. Orphaned Temporary File Cleanup

```rust
pub struct OrphanedTempCleaner {
    checkpoint_dir: PathBuf,
    max_age_hours: u64,
}

impl OrphanedTempCleaner {
    pub async fn cleanup_orphaned_temps(&self) -> Result<usize> {
        let mut cleaned = 0;

        let mut entries = fs::read_dir(&self.checkpoint_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Check if temp file
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with("checkpoint.tmp.") && filename.ends_with(".json") {
                    // Check age
                    let metadata = fs::metadata(&path).await?;
                    let modified = metadata.modified()?;
                    let age = modified.elapsed()?;

                    if age.as_secs() > self.max_age_hours * 3600 {
                        tracing::info!("Cleaning orphaned temp file: {} (age: {}h)", path.display(), age.as_secs() / 3600);

                        fs::remove_file(&path).await?;
                        cleaned += 1;
                    }
                }
            }
        }

        if cleaned > 0 {
            tracing::info!("Cleaned {} orphaned temporary checkpoint files", cleaned);
        }

        Ok(cleaned)
    }
}
```

### Architecture Changes

**New modules:**
- `src/cook/workflow/checkpoint/atomic_writer.rs` - Atomic write implementation
- `src/cook/workflow/checkpoint/retry.rs` - Retry policy
- `src/cook/workflow/checkpoint/locking.rs` - File locking
- `src/cook/workflow/checkpoint/cleanup.rs` - Orphan cleanup

**Modified components:**
- `CheckpointManager` - Use atomic writer
- Integration with Stillwater bracket pattern
- Configuration for retry and lock timeouts

### Configuration Schema

```yaml
checkpoint:
  atomic_writes:
    enabled: true
    retry_max_attempts: 4
    retry_initial_delay_ms: 100
    retry_max_delay_ms: 5000
    lock_timeout_secs: 30
    cleanup_orphans_on_startup: true
    orphan_max_age_hours: 24
```

## Dependencies

- **Prerequisites**: Spec 184 (Unified Checkpoint System)
- **External Dependencies**:
  - `fs2` crate for file locking
  - `nix` crate for Unix process checks
- **Affected Components**: CheckpointManager, storage layer

## Testing Strategy

### Unit Tests
- Atomic write sequence
- Retry policy logic
- Error classification (transient vs permanent)
- Lock acquisition and release
- Orphan detection and cleanup

### Integration Tests
- End-to-end atomic write
- Process crash during write (simulated)
- Concurrent write protection
- Retry with actual failures
- Orphan cleanup on startup

### Chaos Tests
- Kill process during write
- Concurrent access from multiple processes
- Filesystem full scenarios
- Network filesystem delays
- Stale lock recovery

## Documentation Requirements

### Code Documentation
- Document atomic write guarantees
- Explain retry policy configuration
- Describe lock behavior and edge cases
- Add examples for error handling

### User Documentation
- Troubleshooting write failures
- Understanding retry behavior
- Configuring retry policies
- Lock timeout tuning

### Architecture Updates
- Document atomic write architecture
- Update reliability guarantees
- Add concurrent access documentation

## Implementation Notes

### Atomicity Guarantees

POSIX rename is atomic on the same filesystem:
- Old file disappears, new file appears atomically
- No intermediate state visible to readers
- Survives process crash mid-rename
- Works across NFS (with caveats)

Limitations:
- Cross-filesystem rename not atomic
- NFS may have caching issues
- Windows rename semantics differ slightly

### Retry Tuning

Default retry schedule:
- Attempt 1: Immediate
- Attempt 2: +100ms delay
- Attempt 3: +500ms delay (600ms total)
- Attempt 4: +2500ms delay (3100ms total)

Total time to exhaustion: ~3.1 seconds

Rationale:
- Quick recovery from brief hiccups
- Reasonable wait for network issues
- Exponential backoff prevents thundering herd
- Fail fast enough to not block workflows excessively

### Lock File Strategy

Lock file contains PID for stale detection:
```
12345
```

Lock release on process exit:
- Normal exit: explicit unlock + file delete
- Crash: OS releases file lock, next process detects stale
- Timeout: force-release after detecting stale PID

## Migration and Compatibility

### Backward Compatibility

- Non-atomic checkpoints still loadable
- Atomic writes transparent to readers
- No checkpoint format changes
- Graceful handling of non-atomic legacy checkpoints

### Migration Path

1. Deploy with atomic writes disabled
2. Enable for new checkpoints only
3. Monitor success/failure rates
4. Tune retry and lock timeouts
5. Enable globally once validated

### Rollback Strategy

- Disable atomic writes via configuration
- Fall back to direct write (non-atomic)
- No data loss on rollback
- Temporary files cleaned up eventually
