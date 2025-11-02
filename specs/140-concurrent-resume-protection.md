---
number: 140
title: Concurrent Resume Protection with Locking
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-11
---

# Specification 140: Concurrent Resume Protection with Locking

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

Currently, multiple `prodigy resume` processes can run simultaneously on the same session/job without any protection mechanism. This creates severe problems:

**Data Corruption**:
- Multiple processes load same checkpoint
- Concurrent state updates conflict
- Race conditions in worktree creation
- Checkpoint files overwritten inconsistently

**Resource Waste**:
- Duplicate work item processing
- Multiple agents for same item
- Competing worktrees for same session

**Unpredictable Behavior**:
- Which process "wins" is undefined
- User sees confusing error messages
- State becomes inconsistent

There is currently:
- ❌ No locking mechanism
- ❌ No "resume in progress" flag
- ❌ No conflict detection
- ❌ No stale lock cleanup

## Objective

Implement atomic, RAII-based resume locking to ensure only one resume process can execute per session/job at a time, with automatic cleanup and stale lock detection.

## Requirements

### Functional Requirements

- **FR1**: Acquire exclusive lock before resuming session/job
- **FR2**: Atomically create lock file (fail if exists)
- **FR3**: Store lock metadata (process ID, hostname, timestamp)
- **FR4**: Detect and clean up stale locks (dead process)
- **FR5**: Automatically release lock when resume completes or fails
- **FR6**: Provide clear error message when lock held by another process
- **FR7**: Support both session IDs and job IDs

### Non-Functional Requirements

- **NFR1**: RAII pattern for automatic lock cleanup
- **NFR2**: Platform-specific process detection (Unix/Windows)
- **NFR3**: Lock acquisition <100ms in common case
- **NFR4**: Stale lock detection <500ms
- **NFR5**: No orphaned locks under normal conditions
- **NFR6**: Thread-safe lock operations

## Acceptance Criteria

- [ ] `ResumeLockManager` struct implemented
- [ ] `acquire_lock()` atomically creates lock file
- [ ] Lock file contains process ID, hostname, timestamp
- [ ] `ResumeLock` RAII guard auto-releases on drop
- [ ] Stale lock detection using process existence check
- [ ] Platform-specific `is_process_running()` for Unix and Windows
- [ ] Clear error when resume blocked by active lock
- [ ] Unit test: Lock acquisition succeeds when no lock exists
- [ ] Unit test: Lock acquisition fails when lock exists
- [ ] Unit test: Stale lock cleaned up automatically
- [ ] Unit test: Lock released on Drop
- [ ] Integration test: Concurrent resume attempts blocked
- [ ] Integration test: Second resume succeeds after first completes
- [ ] Integration test: Resume succeeds after stale lock cleanup
- [ ] All existing tests pass without modification
- [ ] No unwrap() or panic!() in production code

## Technical Details

### Implementation Approach

**Step 1: Lock Data Structure**

```rust
// src/cook/execution/resume_lock.rs

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

/// Metadata stored in lock file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeLockData {
    pub job_id: String,
    pub process_id: u32,
    pub hostname: String,
    pub acquired_at: DateTime<Utc>,
}

impl ResumeLockData {
    pub fn new(job_id: String) -> Self {
        Self {
            job_id,
            process_id: std::process::id(),
            hostname: get_hostname(),
            acquired_at: Utc::now(),
        }
    }
}

/// Get current hostname
fn get_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}
```

**Step 2: Lock Manager**

```rust
/// Manager for resume lock acquisition and release
#[derive(Clone)]
pub struct ResumeLockManager {
    locks_dir: PathBuf,
}

impl ResumeLockManager {
    /// Create new lock manager
    pub fn new(storage_dir: PathBuf) -> Result<Self> {
        let locks_dir = storage_dir.join("resume_locks");

        // Ensure locks directory exists
        std::fs::create_dir_all(&locks_dir)
            .with_context(|| format!("Failed to create locks directory: {:?}", locks_dir))?;

        Ok(Self { locks_dir })
    }

    /// Acquire exclusive lock for job/session
    ///
    /// Returns Ok(ResumeLock) if lock acquired successfully.
    /// Returns Err if lock already held by active process.
    pub async fn acquire_lock(&self, job_id: &str) -> Result<ResumeLock> {
        let lock_path = self.get_lock_path(job_id);

        // Try to create lock file atomically
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)  // Atomic: fails if file exists
            .open(&lock_path)
            .await
        {
            Ok(mut file) => {
                // Write lock metadata
                let lock_data = ResumeLockData::new(job_id.to_string());
                let json = serde_json::to_string_pretty(&lock_data)?;

                tokio::io::AsyncWriteExt::write_all(&mut file, json.as_bytes())
                    .await
                    .context("Failed to write lock data")?;

                info!("Acquired resume lock for {}", job_id);

                Ok(ResumeLock {
                    job_id: job_id.to_string(),
                    lock_path,
                    manager: self.clone(),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Lock exists - check if stale
                match self.check_and_cleanup_stale_lock(job_id).await {
                    Ok(true) => {
                        // Stale lock removed, retry
                        warn!("Removed stale lock for {}, retrying", job_id);
                        return self.acquire_lock(job_id).await;
                    }
                    Ok(false) => {
                        // Active lock
                        let lock_info = self.read_lock_info(job_id).await
                            .unwrap_or_else(|_| "unknown process".to_string());

                        Err(anyhow!(
                            "Resume already in progress for job {}\n\
                             Lock held by: {}\n\
                             Please wait for the other process to complete, or use --force to override.",
                            job_id,
                            lock_info
                        ))
                    }
                    Err(cleanup_err) => {
                        Err(anyhow!(
                            "Failed to check lock status for {}: {}",
                            job_id,
                            cleanup_err
                        ))
                    }
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Check if lock is stale and clean up if so
    ///
    /// Returns Ok(true) if stale lock was removed
    /// Returns Ok(false) if lock is active
    async fn check_and_cleanup_stale_lock(&self, job_id: &str) -> Result<bool> {
        let lock_path = self.get_lock_path(job_id);

        // Read lock data
        let contents = tokio::fs::read_to_string(&lock_path)
            .await
            .context("Failed to read lock file")?;

        let lock_data: ResumeLockData = serde_json::from_str(&contents)
            .context("Failed to parse lock data")?;

        // Check if process is still running
        if !is_process_running(lock_data.process_id) {
            warn!(
                "Removing stale lock for {} (PID {} no longer running)",
                job_id, lock_data.process_id
            );

            tokio::fs::remove_file(&lock_path)
                .await
                .context("Failed to remove stale lock")?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Read human-readable lock information
    async fn read_lock_info(&self, job_id: &str) -> Result<String> {
        let lock_path = self.get_lock_path(job_id);
        let contents = tokio::fs::read_to_string(&lock_path).await?;
        let lock_data: ResumeLockData = serde_json::from_str(&contents)?;

        Ok(format!(
            "PID {} on {} (acquired {})",
            lock_data.process_id,
            lock_data.hostname,
            lock_data.acquired_at.format("%Y-%m-%d %H:%M:%S UTC")
        ))
    }

    fn get_lock_path(&self, job_id: &str) -> PathBuf {
        self.locks_dir.join(format!("{}.lock", job_id))
    }
}
```

**Step 3: RAII Lock Guard**

```rust
/// RAII guard for resume lock
///
/// Automatically releases lock when dropped
pub struct ResumeLock {
    job_id: String,
    lock_path: PathBuf,
    manager: ResumeLockManager,
}

impl Drop for ResumeLock {
    fn drop(&mut self) {
        // Clean up lock file
        if let Err(e) = std::fs::remove_file(&self.lock_path) {
            warn!(
                "Failed to release lock for {}: {}",
                self.job_id, e
            );
        } else {
            info!("Released resume lock for {}", self.job_id);
        }
    }
}
```

**Step 4: Process Detection (Platform-Specific)**

```rust
/// Check if a process with given PID is running
///
/// Platform-specific implementation
pub fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use std::process::Command;

        // Use kill -0 to check process existence without killing it
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        use std::process::Command;

        // Use tasklist to check process existence
        Command::new("tasklist")
            .args(&["/FI", &format!("PID eq {}", pid), "/NH"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.contains(&pid.to_string()))
            })
            .unwrap_or(false)
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Unsupported platform - assume process is running to be safe
        warn!("Process detection not supported on this platform");
        true
    }
}
```

**Step 5: Integration into Resume Commands**

```rust
// src/cook/execution/mapreduce_resume.rs

use super::resume_lock::ResumeLockManager;

pub struct MapReduceResumeManager {
    state_manager: Arc<dyn JobStateManager>,
    event_logger: Arc<EventLogger>,
    dlq: Arc<DeadLetterQueue>,
    executor: Option<Arc<MapReduceExecutor>>,
    lock_manager: ResumeLockManager,  // NEW
}

impl MapReduceResumeManager {
    pub async fn new(
        job_id: String,
        state_manager: Arc<dyn JobStateManager>,
        event_logger: Arc<EventLogger>,
        project_root: PathBuf,
    ) -> anyhow::Result<Self> {
        // ... existing code ...

        let storage_dir = get_default_storage_dir()?;
        let lock_manager = ResumeLockManager::new(storage_dir)?;

        Ok(Self {
            state_manager,
            event_logger,
            dlq,
            executor: None,
            lock_manager,
        })
    }

    pub async fn resume_job(
        &self,
        job_id: &str,
        options: EnhancedResumeOptions,
        env: &ExecutionEnvironment,
    ) -> MRResult<EnhancedResumeResult> {
        // Acquire lock first (RAII - auto-released on drop)
        let _lock = self
            .lock_manager
            .acquire_lock(job_id)
            .await
            .context("Failed to acquire resume lock")?;

        info!("Starting enhanced resume for job {}", job_id);

        // ... rest of resume logic ...
        // Lock is automatically released when _lock goes out of scope
    }
}
```

### Architecture Changes

**New Module**: `src/cook/execution/resume_lock.rs`
- `ResumeLockManager` - Lock acquisition and management
- `ResumeLock` - RAII guard
- `ResumeLockData` - Lock metadata
- `is_process_running()` - Platform-specific process detection

**Modified Modules**:
- `src/cook/execution/mapreduce_resume.rs` - Add lock manager
- `src/cli/commands/resume.rs` - Add lock to regular workflow resume

**Lock Storage**:
```
~/.prodigy/resume_locks/
├── mapreduce-123.lock
├── session-abc.lock
└── ...
```

### Data Structures

```rust
struct ResumeLockData {
    job_id: String,
    process_id: u32,
    hostname: String,
    acquired_at: DateTime<Utc>,
}

struct ResumeLockManager {
    locks_dir: PathBuf,
}

struct ResumeLock {
    job_id: String,
    lock_path: PathBuf,
    manager: ResumeLockManager,
}
```

### APIs and Interfaces

**New Public API**:

```rust
pub struct ResumeLockManager;
impl ResumeLockManager {
    pub fn new(storage_dir: PathBuf) -> Result<Self>;
    pub async fn acquire_lock(&self, job_id: &str) -> Result<ResumeLock>;
}

pub struct ResumeLock;  // RAII guard

pub fn is_process_running(pid: u32) -> bool;
```

**Modified Behavior**:
- `resume_job()` acquires lock before resuming
- `try_resume_regular_workflow()` acquires lock before resuming
- Concurrent resume attempts fail with clear error

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - MapReduce resume manager
  - Regular workflow resume
  - CLI resume commands
- **External Dependencies**:
  - `hostname` crate (for hostname detection)
  - Platform-specific process commands

## Testing Strategy

### Unit Tests

**Test File**: `src/cook/execution/resume_lock_tests.rs`

```rust
#[tokio::test]
async fn test_acquire_lock_success() {
    let temp_dir = TempDir::new().unwrap();
    let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

    let lock = manager.acquire_lock("test-job").await;
    assert!(lock.is_ok());
}

#[tokio::test]
async fn test_acquire_lock_fails_when_held() {
    let temp_dir = TempDir::new().unwrap();
    let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

    let _lock1 = manager.acquire_lock("test-job").await.unwrap();
    let lock2 = manager.acquire_lock("test-job").await;

    assert!(lock2.is_err());
    assert!(lock2.unwrap_err().to_string().contains("already in progress"));
}

#[tokio::test]
async fn test_lock_released_on_drop() {
    let temp_dir = TempDir::new().unwrap();
    let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

    {
        let _lock = manager.acquire_lock("test-job").await.unwrap();
        // Lock is held
    }  // Lock dropped here

    // Should be able to acquire again
    let lock2 = manager.acquire_lock("test-job").await;
    assert!(lock2.is_ok());
}

#[tokio::test]
async fn test_stale_lock_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

    // Create lock with fake PID (guaranteed not running)
    let lock_path = temp_dir.path().join("resume_locks/test-job.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

    let stale_lock = ResumeLockData {
        job_id: "test-job".to_string(),
        process_id: 999999,  // Fake PID
        hostname: "test-host".to_string(),
        acquired_at: Utc::now(),
    };
    std::fs::write(&lock_path, serde_json::to_string(&stale_lock).unwrap()).unwrap();

    // Try to acquire - should clean up stale lock and succeed
    let lock = manager.acquire_lock("test-job").await;
    assert!(lock.is_ok());
}

#[test]
fn test_is_process_running_current_process() {
    let current_pid = std::process::id();
    assert!(is_process_running(current_pid));
}

#[test]
fn test_is_process_running_fake_process() {
    let fake_pid = 999999;
    assert!(!is_process_running(fake_pid));
}

#[test]
fn test_lock_data_serialization() {
    let lock_data = ResumeLockData::new("test-job".to_string());
    let json = serde_json::to_string(&lock_data).unwrap();
    let deserialized: ResumeLockData = serde_json::from_str(&json).unwrap();

    assert_eq!(lock_data.job_id, deserialized.job_id);
    assert_eq!(lock_data.process_id, deserialized.process_id);
}
```

### Integration Tests

**Test File**: `tests/concurrent_resume_test.rs`

```rust
#[tokio::test]
async fn test_concurrent_resume_attempts_blocked() {
    // Create job state
    // Spawn two concurrent resume tasks
    // Verify one succeeds, one fails with "already in progress"
}

#[tokio::test]
async fn test_sequential_resume_succeeds() {
    // First resume acquires lock
    // First resume completes
    // Second resume acquires lock (should succeed)
}

#[tokio::test]
async fn test_resume_after_crash_cleans_stale_lock() {
    // Create lock file with non-existent PID
    // Resume should detect stale lock
    // Resume should clean up and succeed
}

#[tokio::test]
async fn test_lock_error_message_helpful() {
    // Start first resume
    // Try second resume
    // Verify error message includes PID, hostname, time
}
```

## Documentation Requirements

### Code Documentation

- Module-level docs for `resume_lock.rs`
- Document RAII pattern for ResumeLock
- Explain platform-specific process detection

### User Documentation

Update `CLAUDE.md`:

```markdown
## Concurrent Resume Protection

Prodigy prevents multiple resume processes from running on the same session/job:

**Lock Behavior:**
- Resume acquires exclusive lock before starting
- Lock automatically released when resume completes or fails
- Stale locks (from crashed processes) automatically cleaned up

**Error Message:**
```bash
$ prodigy resume <job_id>
Error: Resume already in progress for job <job_id>
Lock held by: PID 12345 on hostname (acquired 2025-01-11 10:30:00 UTC)
Please wait for the other process to complete.
```

**Troubleshooting Stuck Locks:**

If a lock persists after a process crash:
1. Lock file: `~/.prodigy/resume_locks/<job_id>.lock`
2. Check if PID is running: `ps aux | grep <PID>`
3. If process dead, manually remove lock: `rm ~/.prodigy/resume_locks/<job_id>.lock`
4. Or wait - stale locks are auto-detected on next resume attempt
```

## Implementation Notes

### RAII Pattern Benefits

- Automatic cleanup on success or failure
- Exception-safe (lock released even on panic)
- No manual unlock needed
- Idiomatic Rust

### Platform Detection

- **Unix**: `kill -0 <PID>` checks process existence
- **Windows**: `tasklist /FI "PID eq <PID>"` checks process existence
- **Fallback**: Assume process running (safe default)

### Testing Checklist

- [ ] Lock acquisition
- [ ] Lock blocking
- [ ] Lock release on drop
- [ ] Stale lock cleanup
- [ ] Process detection (Unix/Windows)
- [ ] Concurrent resume integration test
- [ ] Error message clarity

### Gotchas

- **PID Reuse**: Rare but possible - stale lock with reused PID
- **Cross-Host**: Hostname prevents cross-machine conflicts
- **Atomic Creation**: `create_new(true)` is atomic
- **RAII Drop**: Panics in drop are suppressed (log warning only)

## Migration and Compatibility

### Breaking Changes

None. This adds safety without changing API.

### Compatibility Considerations

- Existing resume commands work unchanged
- New lock files created in `~/.prodigy/resume_locks/`
- Lock cleanup is automatic

### Migration Steps

1. Deploy new code
2. Locks directory auto-created
3. Existing resume operations gain lock protection
4. No manual migration required

### Rollback Plan

If issues arise:
1. Remove lock acquisition from resume functions
2. Delete `~/.prodigy/resume_locks/` directory
3. Resume works without locking (original behavior)
4. No data corruption from rollback
