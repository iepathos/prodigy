---
number: 184
title: Unified Checkpoint System
category: storage
priority: critical
status: draft
dependencies: [162, 183]
created: 2025-11-26
updated: 2025-11-29
version: 2.0
---

# Specification 184: Unified Checkpoint System (v2)

**Category**: storage
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 162 (MapReduce Incremental Checkpoint System), Spec 183 (Effect-Based Workflow Execution)

**Version 2.0 Changes**:
- Fixed integrity hash implementation bugs
- Added workflow versioning for resume safety
- Incorporated Stillwater bracket pattern for atomic operations
- Added retry policies for resilient checkpoint I/O
- Clarified storage location (global vs per-repo)
- Added checkpoint validation on resume
- Integrated Stillwater testing utilities
- Added observability requirements
- Defined cleanup policies
- Added configuration specification

## Context

Prodigy currently has **two separate checkpoint systems** that don't synchronize:

### Current State (Broken)

1. **Session State System** (`SessionUpdate::UpdateWorkflowState`)
   - Updated by `workflow_execution.rs` before/after steps
   - Stores in `UnifiedSession` via `SessionManager`
   - Format: `WorkflowState { current_step, completed_steps, ... }`

2. **Checkpoint File System** (`CheckpointManager::save_checkpoint`)
   - Expected by `resume.rs` for workflow resume
   - Looks for files at: `~/.prodigy/state/{repo}/checkpoints/*.checkpoint.json`
   - Format: `WorkflowCheckpoint { execution_state, completed_steps, ... }`

**The Problem**: Standard workflows update Session State, but Resume looks for Checkpoint Files. The checkpoint files are **never created** for non-MapReduce workflows!

Additionally:
- `should_save_checkpoint()` returns `true` only for `ExecutionOutcome::Interrupted`, **not** for `Failed`
- Even when checkpoints would be saved, the format/location doesn't match what resume expects

### MapReduce vs Standard Workflows

| Aspect | MapReduce | Standard Workflow |
|--------|-----------|-------------------|
| Checkpoint creation | ✅ During execution | ❌ Never |
| Checkpoint on failure | ✅ DLQ integration | ❌ No checkpoint |
| Resume finds checkpoint | ✅ Works | ❌ Fails |
| Storage location | `~/.prodigy/state/{repo}/mapreduce/jobs/` | (none) |

## Objective

Create a **single unified checkpoint system** that:
1. Works for both MapReduce and standard workflows
2. Creates checkpoints BEFORE and AFTER each step
3. Creates checkpoints on FAILURE (critical for resume)
4. Uses a single canonical storage format and location
5. Enables reliable resume for all workflow types
6. Validates workflow compatibility on resume
7. Provides resilient I/O with automatic retry
8. Ensures atomic operations with proper cleanup

## Requirements

### Functional Requirements

#### FR1: Unified Checkpoint Format
- **MUST** use single `WorkflowCheckpoint` struct for all workflow types
- **MUST** include workflow path and hash for re-execution validation
- **MUST** include all variables/context for state restoration
- **MUST** support versioning for future compatibility
- **MUST** include integrity hash for corruption detection (excluding hash field itself)
- **MUST** store workflow file hash to detect modifications

#### FR2: Checkpoint Lifecycle
- **MUST** create checkpoint BEFORE each step (with `BeforeStep` state)
- **MUST** create checkpoint AFTER each successful step (with `Completed` state)
- **MUST** create checkpoint on step FAILURE (with `Failed` state) **without ignoring errors**
- **MUST** create checkpoint on signal (SIGINT/SIGTERM) during graceful shutdown
- **MUST** support configurable checkpoint interval for long workflows
- **MUST** fail workflow if checkpoint write fails (no silent failures)

#### FR3: Storage Location Consistency
- **MUST** use canonical path: `~/.prodigy/sessions/{session_id}/checkpoint.json` (global storage)
- **MUST** maintain session ↔ checkpoint bidirectional lookup via `UnifiedSession`
- **MUST** create directory structure automatically
- **MUST** support checkpoint history (for fallback on corruption)
- **MUST** unify with existing `UnifiedSession` storage (checkpoints are part of session data)

**Rationale**: Global storage (`~/.prodigy/sessions/`) matches existing `SessionManager` location and simplifies session discovery. Per-repo checkpoints add complexity without clear benefits.

#### FR4: Atomic Checkpoint Operations
- **MUST** write checkpoints atomically using Stillwater bracket pattern
- **MUST** validate checkpoint integrity before committing
- **MUST** handle concurrent checkpoint writes safely with file locking
- **MUST** recover from interrupted checkpoint writes
- **MUST** guarantee cleanup on failure (no partial writes left behind)
- **MUST** use retry policy for transient I/O failures

#### FR5: Checkpoint Discovery
- **MUST** find checkpoints by session ID
- **MUST** find checkpoints by workflow path (for "resume latest" feature)
- **MUST** list all resumable checkpoints
- **MUST** report clear errors when checkpoints missing/corrupt
- **MUST** validate workflow compatibility before resume

#### FR6: Workflow Versioning
- **MUST** compute SHA-256 hash of workflow file content at checkpoint time
- **MUST** validate workflow hash matches on resume
- **MUST** provide clear error if workflow modified since checkpoint
- **MUST** support optional user-specified workflow version
- **MUST** allow resume override for compatible workflow changes

#### FR7: Observability
- **MUST** emit checkpoint events for monitoring (created, loaded, corrupted, cleaned)
- **MUST** log checkpoint operations at appropriate verbosity levels
- **MUST** track checkpoint metrics (size, write duration, failure rate)
- **MUST** preserve checkpoint metadata in session for debugging

### Non-Functional Requirements

#### NFR1: Performance
- Checkpoint save MUST complete in <100ms for typical workflows (P95)
- Checkpoint load MUST complete in <50ms (P95)
- Checkpoint overhead MUST NOT noticeably impact step execution
- Large checkpoints (>1MB) SHOULD be compressed

#### NFR2: Reliability
- Checkpoints MUST survive process crashes (atomic writes)
- Checkpoints MUST be recoverable after power loss
- Corrupt checkpoints MUST be detected via integrity hash
- Checkpoint write failures MUST NOT be silently ignored
- Transient I/O failures MUST be retried with exponential backoff

#### NFR3: Maintainability
- Single code path for all checkpoint operations
- Clear separation between checkpoint data and I/O (pure core, imperative shell)
- Comprehensive logging for debugging
- Test coverage >90% for checkpoint operations

#### NFR4: Storage Management
- Old checkpoints MUST be cleaned per policy (configurable)
- Checkpoint history MUST be bounded (default: 10 per session)
- Large checkpoints (>10MB) MUST be handled gracefully (compression or error)
- Disk space exhaustion MUST be detected and reported

## Acceptance Criteria

### Unified Format

- [ ] **AC1**: Single WorkflowCheckpoint struct
  - Used by both MapReduce and standard workflows
  - Contains workflow path, hash, variables, step state
  - Version field for future compatibility
  - Integrity hash (computed excluding hash field)

- [ ] **AC2**: Checkpoint state enum covers all cases
  - `BeforeStep { step_index }` - about to execute
  - `Completed { step_index, output }` - step succeeded
  - `Failed { step_index, error, retryable }` - step failed
  - `Interrupted { step_index, in_progress }` - signal received

### Checkpoint Creation

- [ ] **AC3**: Checkpoint before step execution
  - Standard workflow step 3 about to run
  - Checkpoint created with `BeforeStep { step_index: 3 }`
  - Checkpoint exists at canonical location
  - Workflow hash stored in checkpoint

- [ ] **AC4**: Checkpoint after successful step
  - Step 3 completes successfully
  - Checkpoint updated to `Completed { step_index: 3, output }`
  - Previous checkpoint preserved in history
  - History bounded to configured limit

- [ ] **AC5**: Checkpoint on step failure
  - Step 3 fails with error
  - Checkpoint created with `Failed { step_index: 3, error, retryable: true }`
  - Checkpoint write failure FAILS the workflow (no silent ignore)
  - Resume will RETRY this step

- [ ] **AC6**: Checkpoint on SIGINT
  - User presses Ctrl+C during step 3
  - Signal handler triggers graceful shutdown
  - Checkpoint created with `Interrupted` state
  - Step 3 marked as in-progress → pending for resume

### Storage Consistency

- [ ] **AC7**: Canonical storage location
  - Session `session-abc123` creates checkpoint at:
    `~/.prodigy/sessions/session-abc123/checkpoint.json`
  - Resume finds checkpoint at this location
  - Unified with `UnifiedSession` storage

- [ ] **AC8**: Checkpoint history maintained
  - Each checkpoint write creates timestamped backup
  - `~/.prodigy/sessions/{id}/history/checkpoint-{timestamp}.json`
  - Fallback to previous checkpoint if latest corrupt
  - History limited to 10 most recent (configurable)

### Resume Integration

- [ ] **AC9**: Resume finds unified checkpoint
  - `prodigy resume session-abc123` finds checkpoint
  - Loads checkpoint from canonical location
  - Extracts workflow path, variables, failed step
  - Validates workflow hash matches

- [ ] **AC10**: Resume retries failed step
  - Checkpoint shows `Failed { step_index: 3, retryable: true }`
  - Resume starts execution from step 3 (RETRY, not skip)
  - Variables restored from checkpoint
  - Workflow hash validated before execution

- [ ] **AC11**: Workflow modification detection
  - Workflow file modified since checkpoint
  - Resume detects hash mismatch
  - Clear error: "Workflow modified since checkpoint (expected: abc..., got: def...)"
  - Option to override with `--force-resume` flag

- [ ] **AC12**: Checkpoint write failures handled
  - Checkpoint write fails (disk full, permissions, etc.)
  - Workflow execution FAILS immediately (no silent ignore)
  - Error message includes failure reason
  - Previous checkpoint preserved (atomic write)

### Resilience

- [ ] **AC13**: Transient I/O failures retried
  - Network filesystem hiccup during checkpoint write
  - Retry with exponential backoff (3 attempts)
  - Success on retry 2
  - Checkpoint saved successfully

- [ ] **AC14**: Atomic write guarantees
  - Process crashes during checkpoint write
  - Temp file left behind, main checkpoint unchanged
  - Resume loads previous valid checkpoint
  - Orphaned temp files cleaned on next startup

### Cleanup

- [ ] **AC15**: Checkpoint cleanup policy
  - Session completes successfully
  - Checkpoint deleted (configurable: keep/delete)
  - History preserved for audit (configurable duration)
  - Old sessions cleaned per policy (default: 30 days)

- [ ] **AC16**: Large checkpoint handling
  - Checkpoint size >10MB detected
  - Compression applied automatically
  - Warning logged if compressed size >50MB
  - Error if uncompressed size >100MB

## Technical Details

### Implementation Approach

#### 1. Unified Checkpoint Data Structure

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde_json::Value;

/// Unified checkpoint format for all workflow types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    /// Checkpoint format version (for migration)
    pub version: u32,

    /// Session identifier
    pub session_id: SessionId,

    /// Workflow file path (required for re-execution)
    pub workflow_path: PathBuf,

    /// Workflow file hash (SHA-256, for modification detection)
    pub workflow_hash: String,

    /// Optional user-specified workflow version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_version: Option<String>,

    /// Working directory (worktree)
    pub worktree_path: PathBuf,

    /// Current checkpoint state
    pub state: CheckpointState,

    /// Completed steps with their results
    pub completed_steps: Vec<CompletedStepRecord>,

    /// All variables (for state restoration)
    pub variables: HashMap<String, Value>,

    /// Workflow type indicator
    pub workflow_type: WorkflowType,

    /// MapReduce-specific data (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapreduce_state: Option<MapReduceCheckpointState>,

    /// Checkpoint creation timestamp
    pub created_at: DateTime<Utc>,

    /// Checkpoint reason
    pub reason: CheckpointReason,

    /// Integrity hash (SHA-256 of checkpoint content, EXCLUDING this field)
    /// Computed during save, verified during load
    #[serde(default)]
    pub integrity_hash: String,
}

/// State of the checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckpointState {
    /// About to execute step
    BeforeStep { step_index: usize },

    /// Step completed successfully
    Completed {
        step_index: usize,
        output: Option<String>,
    },

    /// Step failed
    Failed {
        step_index: usize,
        error: String,
        retryable: bool,
    },

    /// Workflow interrupted (signal)
    Interrupted {
        step_index: usize,
        in_progress: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedStepRecord {
    pub step_index: usize,
    pub command: String,
    pub output: Option<String>,
    pub captured_variables: HashMap<String, String>,
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowType {
    Standard,
    Iterative { max_iterations: u32, current_iteration: u32 },
    MapReduce { job_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckpointReason {
    BeforeStep,
    AfterStep,
    StepFailed,
    Signal,
    TimeInterval,
    Manual,
}

/// Helper to compute workflow file hash
pub fn compute_workflow_hash(path: &Path) -> Result<String, std::io::Error> {
    use sha2::{Digest, Sha256};
    let content = std::fs::read(path)?;
    let hash = Sha256::digest(&content);
    Ok(format!("{:x}", hash))
}
```

#### 2. Checkpoint Storage Trait

```rust
use stillwater::Either;

/// Storage abstraction for checkpoints
#[async_trait]
pub trait CheckpointStorage: Send + Sync {
    /// Save checkpoint atomically with retry policy
    async fn save(&self, checkpoint: &WorkflowCheckpoint) -> Result<(), CheckpointError>;

    /// Load latest checkpoint for session
    async fn load(&self, session_id: &SessionId) -> Result<Option<WorkflowCheckpoint>, CheckpointError>;

    /// Load checkpoint from history (for fallback)
    async fn load_from_history(&self, session_id: &SessionId, index: usize) -> Result<Option<WorkflowCheckpoint>, CheckpointError>;

    /// List all checkpoints for session (newest first)
    async fn list_history(&self, session_id: &SessionId) -> Result<Vec<CheckpointInfo>, CheckpointError>;

    /// Delete checkpoint and history
    async fn delete(&self, session_id: &SessionId) -> Result<(), CheckpointError>;

    /// Find all resumable sessions
    async fn find_resumable(&self) -> Result<Vec<ResumableSession>, CheckpointError>;

    /// Validate checkpoint integrity and workflow compatibility
    async fn validate(&self, checkpoint: &WorkflowCheckpoint) -> Result<ValidationResult, CheckpointError>;
}

/// Validation result for checkpoint resume
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Clone)]
pub enum ValidationError {
    IntegrityMismatch { expected: String, computed: String },
    WorkflowHashMismatch { expected: String, computed: String },
    WorkflowNotFound { path: PathBuf },
    WorktreeNotFound { path: PathBuf },
    IncompatibleVersion { checkpoint_version: u32, current_version: u32 },
    CorruptedData { reason: String },
}

#[derive(Debug, Clone)]
pub enum ValidationWarning {
    WorkflowModified { old_hash: String, new_hash: String },
    LargeCheckpoint { size_mb: f64 },
    OldCheckpoint { age_days: u32 },
}
```

#### 3. File-Based Storage with Bracket Pattern

```rust
use stillwater::{bracket, bracket_sync, Effect, RetryPolicy, BracketError};
use std::time::Duration;

/// File-based checkpoint storage implementation
pub struct FileCheckpointStorage {
    base_path: PathBuf,
    history_limit: usize,
    retry_policy: RetryPolicy,
    compression_threshold_mb: f64,
}

impl FileCheckpointStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            history_limit: 10,
            retry_policy: RetryPolicy::exponential(Duration::from_millis(100))
                .with_max_retries(3)
                .with_jitter(0.1),
            compression_threshold_mb: 1.0,
        }
    }

    /// Get canonical checkpoint path
    fn checkpoint_path(&self, session_id: &SessionId) -> PathBuf {
        self.base_path
            .join(session_id.as_str())
            .join("checkpoint.json")
    }

    /// Get history directory
    fn history_path(&self, session_id: &SessionId) -> PathBuf {
        self.base_path
            .join(session_id.as_str())
            .join("history")
    }

    /// Compute integrity hash (excluding the hash field itself)
    fn compute_integrity_hash(checkpoint: &WorkflowCheckpoint) -> Result<String, serde_json::Error> {
        use sha2::{Digest, Sha256};

        // Create copy with empty hash field for consistent hashing
        let mut checkpoint_for_hash = checkpoint.clone();
        checkpoint_for_hash.integrity_hash = String::new();

        let json = serde_json::to_string(&checkpoint_for_hash)?;
        let hash = Sha256::digest(json.as_bytes());
        Ok(format!("{:x}", hash))
    }

    /// Verify integrity hash
    fn verify_integrity(checkpoint: &WorkflowCheckpoint) -> Result<(), CheckpointError> {
        let stored_hash = checkpoint.integrity_hash.clone();
        let computed_hash = Self::compute_integrity_hash(checkpoint)
            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;

        if computed_hash != stored_hash {
            return Err(CheckpointError::IntegrityError {
                session_id: checkpoint.session_id.clone(),
                expected: stored_hash,
                computed: computed_hash,
            });
        }

        Ok(())
    }
}

#[async_trait]
impl CheckpointStorage for FileCheckpointStorage {
    async fn save(&self, checkpoint: &WorkflowCheckpoint) -> Result<(), CheckpointError> {
        let path = self.checkpoint_path(&checkpoint.session_id);
        let history_dir = self.history_path(&checkpoint.session_id);

        // Ensure directories exist
        tokio::fs::create_dir_all(path.parent().unwrap()).await
            .map_err(|e| CheckpointError::IoError(e.to_string()))?;
        tokio::fs::create_dir_all(&history_dir).await
            .map_err(|e| CheckpointError::IoError(e.to_string()))?;

        // Archive existing checkpoint to history BEFORE writing new one
        if path.exists() {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
            let history_file = history_dir.join(format!("checkpoint-{}.json", timestamp));
            tokio::fs::copy(&path, &history_file).await
                .map_err(|e| CheckpointError::IoError(e.to_string()))?;
        }

        // Compute integrity hash
        let hash = Self::compute_integrity_hash(checkpoint)
            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;

        let mut checkpoint_with_hash = checkpoint.clone();
        checkpoint_with_hash.integrity_hash = hash;

        // Serialize
        let json = serde_json::to_string_pretty(&checkpoint_with_hash)
            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;

        // Check if compression needed
        let size_mb = json.len() as f64 / (1024.0 * 1024.0);
        let data = if size_mb > self.compression_threshold_mb {
            // TODO: Compress with zstd or similar
            json.into_bytes()
        } else {
            json.into_bytes()
        };

        // Write atomically with retry - use bracket for guaranteed cleanup
        let result = Effect::retry(
            || {
                let path = path.clone();
                let data = data.clone();

                Effect::from_async(move |_: &()| async move {
                    // Temp file for atomic write
                    let temp_path = path.with_extension("tmp");

                    // Use bracket to ensure temp file cleanup on failure
                    bracket(
                        // Acquire: Create temp file
                        Effect::from_async({
                            let temp_path = temp_path.clone();
                            let data = data.clone();
                            move |_: &()| async move {
                                tokio::fs::write(&temp_path, &data).await
                                    .map_err(|e| CheckpointError::IoError(e.to_string()))?;
                                Ok::<_, CheckpointError>(temp_path)
                            }
                        }),
                        // Release: Remove temp file if still exists
                        |temp_path: PathBuf| async move {
                            let _ = tokio::fs::remove_file(&temp_path).await;
                        },
                        // Use: Atomic rename
                        {
                            let path = path.clone();
                            move |temp_path: PathBuf| {
                                Effect::from_async(move |_: &()| async move {
                                    tokio::fs::rename(&temp_path, &path).await
                                        .map_err(|e| CheckpointError::IoError(e.to_string()))?;
                                    Ok::<_, CheckpointError>(())
                                })
                            }
                        }
                    )
                })
            },
            self.retry_policy.clone()
        )
        .run(&())
        .await;

        // Map retry error to checkpoint error
        result.map_err(|retry_err| {
            CheckpointError::IoError(format!("Failed after retries: {:?}", retry_err))
        })?;

        // Cleanup old history
        self.cleanup_history(&checkpoint.session_id).await?;

        Ok(())
    }

    async fn load(&self, session_id: &SessionId) -> Result<Option<WorkflowCheckpoint>, CheckpointError> {
        let path = self.checkpoint_path(session_id);

        if !path.exists() {
            return Ok(None);
        }

        let json = tokio::fs::read_to_string(&path).await
            .map_err(|e| CheckpointError::IoError(e.to_string()))?;

        let checkpoint: WorkflowCheckpoint = serde_json::from_str(&json)
            .map_err(|e| CheckpointError::DeserializationError(e.to_string()))?;

        // Verify integrity
        Self::verify_integrity(&checkpoint)?;

        Ok(Some(checkpoint))
    }

    async fn validate(&self, checkpoint: &WorkflowCheckpoint) -> Result<ValidationResult, CheckpointError> {
        use stillwater::traverse::traverse;

        let validators: Vec<Box<dyn Fn(&WorkflowCheckpoint) -> Validation<(), ValidationError>>> = vec![
            Box::new(|cp| {
                // Verify integrity hash
                Self::verify_integrity(cp)
                    .map(|_| Validation::success(()))
                    .unwrap_or_else(|e| match e {
                        CheckpointError::IntegrityError { expected, computed, .. } => {
                            Validation::failure(vec![ValidationError::IntegrityMismatch { expected, computed }])
                        }
                        _ => Validation::failure(vec![ValidationError::CorruptedData { reason: e.to_string() }])
                    })
            }),
            Box::new(|cp| {
                // Verify workflow file exists
                if !cp.workflow_path.exists() {
                    Validation::failure(vec![ValidationError::WorkflowNotFound { path: cp.workflow_path.clone() }])
                } else {
                    Validation::success(())
                }
            }),
            Box::new(|cp| {
                // Verify workflow hash matches (unless force resume)
                match compute_workflow_hash(&cp.workflow_path) {
                    Ok(current_hash) if current_hash != cp.workflow_hash => {
                        Validation::failure(vec![ValidationError::WorkflowHashMismatch {
                            expected: cp.workflow_hash.clone(),
                            computed: current_hash,
                        }])
                    }
                    Ok(_) => Validation::success(()),
                    Err(e) => Validation::failure(vec![ValidationError::CorruptedData {
                        reason: format!("Failed to compute workflow hash: {}", e)
                    }])
                }
            }),
            Box::new(|cp| {
                // Verify worktree exists
                if !cp.worktree_path.exists() {
                    Validation::failure(vec![ValidationError::WorktreeNotFound { path: cp.worktree_path.clone() }])
                } else {
                    Validation::success(())
                }
            }),
        ];

        // Run all validators and accumulate errors
        let results: Vec<_> = validators.iter().map(|v| v(checkpoint)).collect();
        let combined = Validation::all_vec(results);

        match combined {
            Validation::Success(_) => Ok(ValidationResult {
                is_valid: true,
                errors: vec![],
                warnings: vec![],
            }),
            Validation::Failure(errors) => Ok(ValidationResult {
                is_valid: false,
                errors: errors.into_iter().flatten().collect(),
                warnings: vec![],
            }),
        }
    }
}
```

#### 4. Effect-Based Checkpoint Operations

```rust
use stillwater::Effect;

/// Save checkpoint effect with retry and validation
pub fn save_checkpoint(state: CheckpointState) -> Effect<(), CheckpointError, WorkflowEnv> {
    Effect::from_async(|env: &WorkflowEnv| async move {
        // Compute workflow hash
        let workflow_hash = compute_workflow_hash(&env.workflow_path)
            .map_err(|e| CheckpointError::IoError(format!("Failed to hash workflow: {}", e)))?;

        let checkpoint = WorkflowCheckpoint {
            version: CHECKPOINT_VERSION,
            session_id: env.session_id.clone(),
            workflow_path: env.workflow_path.clone(),
            workflow_hash,
            workflow_version: env.workflow_version.clone(),
            worktree_path: env.worktree_path.clone(),
            state,
            completed_steps: env.completed_steps.clone(),
            variables: env.variables.clone(),
            workflow_type: env.workflow_type.clone(),
            mapreduce_state: env.mapreduce_state.clone(),
            created_at: Utc::now(),
            reason: state.to_reason(),
            integrity_hash: String::new(), // Computed during save
        };

        // Emit checkpoint event
        env.events.emit(CheckpointEvent::Creating {
            session_id: env.session_id.clone(),
            state: state.clone(),
        });

        // Save with retry
        let result = env.checkpoint_storage.save(&checkpoint).await;

        match &result {
            Ok(_) => {
                env.events.emit(CheckpointEvent::Created {
                    session_id: env.session_id.clone(),
                    state,
                    size_bytes: 0, // TODO: Track actual size
                });
            }
            Err(e) => {
                env.events.emit(CheckpointEvent::Failed {
                    session_id: env.session_id.clone(),
                    error: e.to_string(),
                });
            }
        }

        result
    })
}

/// Load checkpoint effect with validation
pub fn load_checkpoint(
    session_id: &SessionId,
    force: bool,
) -> Effect<Option<WorkflowCheckpoint>, CheckpointError, WorkflowEnv> {
    let session_id = session_id.clone();
    Effect::from_async(move |env: &WorkflowEnv| async move {
        // Load checkpoint
        let checkpoint = env.checkpoint_storage.load(&session_id).await?;

        if let Some(ref cp) = checkpoint {
            // Validate checkpoint
            let validation = env.checkpoint_storage.validate(cp).await?;

            if !validation.is_valid && !force {
                return Err(CheckpointError::ValidationFailed {
                    errors: validation.errors,
                });
            }

            // Emit warnings
            for warning in validation.warnings {
                warn!("Checkpoint warning: {:?}", warning);
            }

            env.events.emit(CheckpointEvent::Loaded {
                session_id: session_id.clone(),
                state: cp.state.clone(),
                from_history: false,
            });
        }

        Ok(checkpoint)
    })
}

/// Step wrapper with automatic checkpointing
pub fn with_checkpointing(
    step_index: usize,
    step: &WorkflowStep,
) -> Effect<StepResult, StepError, WorkflowEnv> {
    // Save BEFORE step
    save_checkpoint(CheckpointState::BeforeStep { step_index })
        .map_err(StepError::from)
        .and_then(move |_| execute_step(step))
        // Save AFTER success - MUST NOT ignore errors
        .and_then(move |result| {
            save_checkpoint(CheckpointState::Completed {
                step_index,
                output: result.output.clone(),
            })
            .map_err(StepError::from)
            .map(move |_| result)
        })
        // Save on FAILURE
        .or_else(move |error| {
            save_checkpoint(CheckpointState::Failed {
                step_index,
                error: error.to_string(),
                retryable: error.is_retryable(),
            })
            .and_then(|_| Effect::fail(error))
        })
}
```

#### 5. Resume Logic with Validation

```rust
use stillwater::Validation;

/// Pure function: plan resume from checkpoint
pub fn plan_resume(checkpoint: &WorkflowCheckpoint, workflow: &Workflow) -> ResumePlan {
    match &checkpoint.state {
        CheckpointState::BeforeStep { step_index } => {
            // Was about to execute, execute it
            ResumePlan {
                start_index: *step_index,
                retry_current: true,
                skip_steps: checkpoint.completed_steps.iter().map(|s| s.step_index).collect(),
            }
        }
        CheckpointState::Completed { step_index, .. } => {
            // Step completed, continue with next
            ResumePlan {
                start_index: step_index + 1,
                retry_current: false,
                skip_steps: checkpoint.completed_steps.iter().map(|s| s.step_index).collect(),
            }
        }
        CheckpointState::Failed { step_index, retryable, .. } => {
            // Step failed, retry if retryable
            ResumePlan {
                start_index: *step_index,
                retry_current: *retryable,
                skip_steps: checkpoint.completed_steps.iter().map(|s| s.step_index).collect(),
            }
        }
        CheckpointState::Interrupted { step_index, in_progress } => {
            // Interrupted, retry the in-progress step
            ResumePlan {
                start_index: if *in_progress { *step_index } else { *step_index + 1 },
                retry_current: *in_progress,
                skip_steps: checkpoint.completed_steps.iter().map(|s| s.step_index).collect(),
            }
        }
    }
}

/// Resume workflow from checkpoint with validation
pub fn resume_workflow(
    session_id: SessionId,
    force: bool,
) -> Effect<WorkflowResult, WorkflowError, WorkflowEnv> {
    // Load and validate checkpoint
    load_checkpoint(&session_id, force)
        .and_then(|checkpoint_opt| {
            checkpoint_opt
                .ok_or_else(|| WorkflowError::CheckpointNotFound { session_id: session_id.clone() })
                .map(Effect::pure)
        })
        .and_then(|checkpoint| {
            // Load workflow from checkpoint path
            load_workflow(&checkpoint.workflow_path)
                .map(move |workflow| (checkpoint, workflow))
        })
        .and_then(|(checkpoint, workflow)| {
            let plan = plan_resume(&checkpoint, &workflow);

            // Restore environment from checkpoint
            Effect::local(
                move |env| WorkflowEnv {
                    variables: checkpoint.variables.clone(),
                    completed_steps: checkpoint.completed_steps.clone(),
                    ..env.clone()
                },
                execute_workflow_from(plan.start_index, workflow.steps, plan.skip_steps),
            )
        })
}
```

### Architecture Changes

#### Storage Layout

```
~/.prodigy/
├── sessions/                           # Unified session storage (global)
│   ├── session-abc123/
│   │   ├── session.json                # UnifiedSession metadata
│   │   ├── checkpoint.json             # Current checkpoint
│   │   └── history/
│   │       ├── checkpoint-20251126_100000.json
│   │       ├── checkpoint-20251126_100030.json
│   │       └── checkpoint-20251126_100100.json
│   └── session-def456/
│       ├── session.json
│       ├── checkpoint.json
│       └── history/
└── state/{repo}/                       # Repo-specific state (MapReduce, DLQ, etc.)
    ├── mapreduce/jobs/                 # MapReduce jobs (unchanged)
    ├── dlq/                            # Dead letter queue
    └── events/                         # Event logs
```

**Key Changes**:
- Checkpoints are part of session data, not repo-specific
- `UnifiedSession` references checkpoint location
- Session discovery independent of repo
- Simplifies multi-repo workflows

#### Modified Components

1. **workflow_execution.rs** - Use `with_checkpointing()` wrapper
2. **resume.rs** - Use `resume_workflow()` with validation
3. **SessionManager** - Load/save checkpoints as part of session
4. **signal_handler.rs** - Trigger checkpoint on SIGINT/SIGTERM
5. **execution_pipeline.rs** - Remove old checkpoint logic

#### New Components

1. **checkpoint/storage.rs** - `CheckpointStorage` trait
2. **checkpoint/file_storage.rs** - `FileCheckpointStorage` implementation
3. **checkpoint/effects.rs** - Effect-based checkpoint operations
4. **checkpoint/validation.rs** - Checkpoint validation logic
5. **checkpoint/events.rs** - Checkpoint event types
6. **checkpoint/cleanup.rs** - Cleanup policies and execution

### Configuration

#### Workflow-Level Configuration

```yaml
# In workflow YAML
checkpoint:
  enabled: true                 # Enable/disable checkpointing (default: true)
  interval: 10                  # Checkpoint every N steps (default: 1 = every step)
  on_failure: true              # Checkpoint on step failure (default: true)
  on_interrupt: true            # Checkpoint on signal (default: true)
  history_limit: 5              # Keep N most recent checkpoints (default: 10)
  compression_threshold_mb: 1.0 # Compress if larger than N MB (default: 1.0)
  fail_on_write_error: true     # Fail workflow if checkpoint write fails (default: true)
```

#### Global Configuration

```toml
# ~/.prodigy/config.toml
[checkpoints]
default_interval = 1
history_limit = 10
max_size_mb = 100
compression_threshold_mb = 1.0
cleanup_policy = "on_completion"  # "on_completion" | "manual" | "after_days"
cleanup_after_days = 30
retry_attempts = 3
retry_backoff_ms = 100
```

### Observability

#### Checkpoint Events

```rust
#[derive(Debug, Clone, Serialize)]
pub enum CheckpointEvent {
    Creating {
        session_id: SessionId,
        state: CheckpointState,
    },
    Created {
        session_id: SessionId,
        state: CheckpointState,
        size_bytes: usize,
    },
    Loaded {
        session_id: SessionId,
        state: CheckpointState,
        from_history: bool,
    },
    Corrupted {
        session_id: SessionId,
        expected_hash: String,
        computed_hash: String,
    },
    Cleaned {
        session_id: SessionId,
        count: usize,
    },
    Failed {
        session_id: SessionId,
        error: String,
    },
    ValidationFailed {
        session_id: SessionId,
        errors: Vec<ValidationError>,
    },
}
```

#### Logging

- **TRACE**: Individual checkpoint field serialization
- **DEBUG**: Checkpoint save/load operations, validation results
- **INFO**: Checkpoint creation with reason, cleanup operations
- **WARN**: Validation warnings, large checkpoints, retry attempts
- **ERROR**: Checkpoint write failures, corruption detected, validation failures

#### Metrics

```rust
pub struct CheckpointMetrics {
    pub total_saves: Counter,
    pub total_loads: Counter,
    pub save_duration_ms: Histogram,
    pub load_duration_ms: Histogram,
    pub checkpoint_size_bytes: Histogram,
    pub save_failures: Counter,
    pub load_failures: Counter,
    pub integrity_failures: Counter,
    pub validation_failures: Counter,
}
```

## Dependencies

### Prerequisites
- **Spec 162**: MapReduce Incremental Checkpoint System (pattern reference)
- **Spec 183**: Effect-Based Workflow Execution (Effect infrastructure)

### Affected Components
- Session management (UnifiedSession integration)
- Resume command (validation logic)
- Workflow execution (checkpoint wrappers)
- MapReduce execution (unified checkpoint integration)
- Signal handling (graceful shutdown with checkpoint)

### External Dependencies
- `sha2` for integrity hashing and workflow hashing
- `tokio::fs` for async file operations
- `humantime_serde` for Duration serialization
- `stillwater` 0.5+ for Effect, Bracket, Retry, Validation, Traverse
- Optional: `zstd` or `flate2` for checkpoint compression

## Testing Strategy

### Unit Tests

```rust
use stillwater::testing::{MockEnv, TestEffect};
use stillwater::Validation;

#[test]
fn test_plan_resume_from_failed_step() {
    let checkpoint = WorkflowCheckpoint {
        state: CheckpointState::Failed {
            step_index: 3,
            error: "500 error".to_string(),
            retryable: true,
        },
        completed_steps: vec![
            CompletedStepRecord { step_index: 0, .. },
            CompletedStepRecord { step_index: 1, .. },
            CompletedStepRecord { step_index: 2, .. },
        ],
        ..
    };

    let plan = plan_resume(&checkpoint, &workflow);

    assert_eq!(plan.start_index, 3); // Retry failed step
    assert!(plan.retry_current);
    assert_eq!(plan.skip_steps, vec![0, 1, 2]);
}

#[test]
fn test_integrity_hash_computation() {
    let checkpoint = WorkflowCheckpoint {
        integrity_hash: String::new(),
        ..
    };

    let hash1 = FileCheckpointStorage::compute_integrity_hash(&checkpoint).unwrap();
    let hash2 = FileCheckpointStorage::compute_integrity_hash(&checkpoint).unwrap();

    // Hash should be deterministic
    assert_eq!(hash1, hash2);

    // Hash should change with content
    let mut checkpoint2 = checkpoint.clone();
    checkpoint2.session_id = SessionId::new();
    let hash3 = FileCheckpointStorage::compute_integrity_hash(&checkpoint2).unwrap();
    assert_ne!(hash1, hash3);
}

#[test]
fn test_validation_accumulates_all_errors() {
    use stillwater::traverse::traverse;

    let checkpoint = WorkflowCheckpoint {
        workflow_path: PathBuf::from("/nonexistent"),
        workflow_hash: "wrong-hash".to_string(),
        worktree_path: PathBuf::from("/nonexistent"),
        integrity_hash: "wrong-hash".to_string(),
        ..
    };

    let storage = FileCheckpointStorage::new(PathBuf::from("/tmp"));
    let validation = storage.validate(&checkpoint).await.unwrap();

    // Should accumulate ALL errors, not just first
    assert!(!validation.is_valid);
    assert!(validation.errors.len() >= 3); // Integrity, workflow, worktree
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_checkpoint_created_on_failure() {
    let storage = InMemoryCheckpointStorage::new();
    let env = MockEnv::new()
        .with(|| storage.clone())
        .build();

    // Execute workflow that fails at step 2
    let result = execute_workflow(failing_workflow)
        .run(&env)
        .await;

    assert!(result.is_err());

    // Checkpoint should exist with Failed state
    let checkpoint = storage.load(&env.session_id).await.unwrap().unwrap();
    assert!(matches!(checkpoint.state, CheckpointState::Failed { step_index: 2, .. }));
}

#[tokio::test]
async fn test_resume_retries_failed_step() {
    // Create checkpoint with failed step 2
    let checkpoint = WorkflowCheckpoint {
        state: CheckpointState::Failed { step_index: 2, retryable: true, .. },
        completed_steps: vec![step_record(0), step_record(1)],
        ..
    };

    let storage = InMemoryCheckpointStorage::with_checkpoint(checkpoint.clone());
    let env = MockEnv::new()
        .with(|| storage.clone())
        .build();

    // Resume should retry step 2
    let result = resume_workflow(checkpoint.session_id.clone(), false)
        .run(&env)
        .await
        .unwrap();

    // Step 2 was retried (executed again)
    assert!(env.execution_log.contains(&"step-2"));
    // Steps 0, 1 were skipped
    assert!(!env.execution_log.contains(&"step-0"));
    assert!(!env.execution_log.contains(&"step-1"));
}

#[tokio::test]
async fn test_atomic_checkpoint_write() {
    let storage = FileCheckpointStorage::new(temp_dir());

    // Simulate concurrent writes
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let s = storage.clone();
            tokio::spawn(async move {
                let checkpoint = create_checkpoint_with_step(i);
                s.save(&checkpoint).await
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Final checkpoint should be valid (no corruption)
    let checkpoint = storage.load(&session_id).await.unwrap().unwrap();
    FileCheckpointStorage::verify_integrity(&checkpoint).unwrap();
}

#[tokio::test]
async fn test_workflow_modification_detected() {
    let checkpoint = WorkflowCheckpoint {
        workflow_hash: "original-hash".to_string(),
        ..
    };

    // Modify workflow file
    std::fs::write(&checkpoint.workflow_path, "modified content").unwrap();

    let storage = FileCheckpointStorage::new(temp_dir());
    let validation = storage.validate(&checkpoint).await.unwrap();

    assert!(!validation.is_valid);
    assert!(validation.errors.iter().any(|e| matches!(
        e,
        ValidationError::WorkflowHashMismatch { .. }
    )));
}

#[tokio::test]
async fn test_retry_on_transient_failure() {
    let storage = FlakyCheckpointStorage::new(2); // Fail first 2 attempts

    let checkpoint = create_test_checkpoint();
    let result = storage.save(&checkpoint).await;

    assert!(result.is_ok()); // Should succeed on attempt 3
    assert_eq!(storage.attempt_count(), 3);
}

#[tokio::test]
async fn test_checkpoint_write_failure_fails_workflow() {
    let storage = FailingCheckpointStorage::new(); // Always fails

    let env = MockEnv::new()
        .with(|| storage.clone())
        .build();

    let result = execute_workflow_with_checkpointing(workflow)
        .run(&env)
        .await;

    // Workflow should FAIL due to checkpoint error, not silently ignore
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WorkflowError::CheckpointFailed { .. }));
}
```

### Property-Based Tests

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_integrity_hash_changes_with_content(
        session_id in any::<String>(),
        step_index in 0usize..100
    ) {
        let checkpoint1 = WorkflowCheckpoint {
            session_id: SessionId::from(session_id.clone()),
            state: CheckpointState::BeforeStep { step_index },
            ..
        };

        let checkpoint2 = WorkflowCheckpoint {
            session_id: SessionId::from(session_id),
            state: CheckpointState::BeforeStep { step_index: step_index + 1 },
            ..
        };

        let hash1 = FileCheckpointStorage::compute_integrity_hash(&checkpoint1)?;
        let hash2 = FileCheckpointStorage::compute_integrity_hash(&checkpoint2)?;

        prop_assert_ne!(hash1, hash2);
    }
}
```

## Documentation Requirements

### Code Documentation
- Document CheckpointStorage trait contract and implementation requirements
- Document checkpoint state transitions with diagrams
- Document atomic write guarantees and bracket pattern usage
- Document retry behavior and failure modes
- Document validation rules and override mechanism

### User Documentation
- Update resume troubleshooting guide with validation errors
- Document checkpoint locations and structure
- Explain checkpoint recovery process and history fallback
- Document `--force-resume` flag and when to use it
- Add examples of checkpoint configuration

### Architecture Documentation
- Diagram showing checkpoint lifecycle
- Diagram showing storage layout
- Sequence diagram for checkpoint save with retry
- Sequence diagram for resume with validation

## Migration and Compatibility

### Breaking Changes
- Checkpoint format changes (version field enables migration)
- Storage location changes (migration script provided)
- SessionManager integration (checkpoints part of session)

### Migration Script

```rust
use stillwater::traverse::traverse_effect;

/// Migrate from old checkpoint format/location to unified system
pub async fn migrate_checkpoints(old_base: &Path, new_base: &Path) -> Result<MigrationReport> {
    let mut report = MigrationReport::new();

    // Find old session state files
    let old_sessions = find_old_session_states(old_base).await?;

    // Convert to effects for parallel migration with bounded concurrency
    let migration_effects: Vec<_> = old_sessions
        .into_iter()
        .map(|old_state| {
            Effect::from_async(move |_: &()| async move {
                match convert_to_unified_checkpoint(&old_state) {
                    Ok(new_checkpoint) => {
                        let new_storage = FileCheckpointStorage::new(new_base.to_path_buf());
                        new_storage.save(&new_checkpoint).await?;
                        Ok(MigrationSuccess {
                            session_id: new_checkpoint.session_id,
                            old_path: old_state.path,
                            new_path: new_storage.checkpoint_path(&new_checkpoint.session_id),
                        })
                    }
                    Err(e) => Err(MigrationError::ConversionFailed {
                        session_id: old_state.session_id,
                        error: e.to_string(),
                    })
                }
            })
        })
        .collect();

    // Run migrations in parallel with limit
    let results = Effect::par_all_limit(migration_effects, 10)
        .run(&())
        .await;

    for result in results {
        match result {
            Ok(success) => report.migrated.push(success),
            Err(error) => report.failed.push(error),
        }
    }

    Ok(report)
}

#[derive(Debug)]
pub struct MigrationReport {
    pub migrated: Vec<MigrationSuccess>,
    pub failed: Vec<MigrationError>,
    pub duration: Duration,
}

impl MigrationReport {
    pub fn success_rate(&self) -> f64 {
        let total = self.migrated.len() + self.failed.len();
        if total == 0 {
            1.0
        } else {
            self.migrated.len() as f64 / total as f64
        }
    }
}
```

### Rollback Strategy

1. Migration creates backup of old checkpoints
2. New checkpoint includes `migrated_from` field
3. Rollback script converts new checkpoints back to old format
4. Validation ensures no data loss during conversion

### Compatibility
- Old checkpoints auto-detected and migrated on first access
- Version field enables future format changes
- MapReduce checkpoints continue to work (unified storage, same format)
- Graceful degradation if validation fails (clear error, not crash)

## Success Metrics

### Quantitative
- 100% of workflow failures create checkpoint (AC5)
- Resume success rate > 95% for valid checkpoints (AC9, AC10)
- Checkpoint save < 100ms P95 (NFR1)
- Checkpoint load < 50ms P95 (NFR1)
- Zero checkpoint corruption in CI tests (AC8, AC14)
- Test coverage > 90% for checkpoint operations (NFR3)

### Qualitative
- Single code path for all checkpoint operations (NFR3)
- Clear error messages for missing/corrupt checkpoints (FR5)
- Reliable resume for standard workflows (matches MapReduce experience)
- Developers can reason about checkpoint state transitions
- Users understand resume failures and how to fix them

## Open Questions

### Resolved
1. **Storage location**: Global (`~/.prodigy/sessions/`) to match SessionManager
2. **Hash implementation**: Exclude hash field, compute before setting
3. **Checkpoint write failure**: MUST fail workflow, not silently ignore
4. **Workflow versioning**: SHA-256 hash of file content, validated on resume

### Remaining
1. **Compression library**: zstd vs flate2 vs brotli?
2. **Signal handling integration**: How to trigger checkpoint in signal handler safely?
3. **MapReduce migration timeline**: Migrate MapReduce to unified checkpoints immediately or incrementally?
4. **Checkpoint size limits**: Hard limit or just warning for large checkpoints?

## Implementation Phases

### Phase 1: Core Infrastructure (1-2 weeks)
- [ ] Implement `WorkflowCheckpoint` data structure
- [ ] Implement `CheckpointStorage` trait
- [ ] Implement `FileCheckpointStorage` with bracket pattern
- [ ] Add integrity hash computation (fixed implementation)
- [ ] Add workflow hash computation
- [ ] Write unit tests for pure functions

### Phase 2: Effect Integration (1 week)
- [ ] Implement checkpoint effects (`save_checkpoint`, `load_checkpoint`)
- [ ] Implement `with_checkpointing` wrapper
- [ ] Add retry policies for checkpoint I/O
- [ ] Integrate with workflow execution
- [ ] Write integration tests

### Phase 3: Validation & Resume (1 week)
- [ ] Implement checkpoint validation logic
- [ ] Implement `resume_workflow` with validation
- [ ] Add `--force-resume` flag
- [ ] Update resume command
- [ ] Write resume integration tests

### Phase 4: Observability & Cleanup (1 week)
- [ ] Implement checkpoint events
- [ ] Add logging at appropriate levels
- [ ] Implement cleanup policies
- [ ] Add configuration support
- [ ] Write cleanup tests

### Phase 5: Migration & Documentation (1 week)
- [ ] Write migration script
- [ ] Test migration on production-like data
- [ ] Update user documentation
- [ ] Update architecture documentation
- [ ] Create migration guide

### Phase 6: Signal Handling & Polish (1 week)
- [ ] Integrate signal handler with checkpoint system
- [ ] Add compression support
- [ ] Performance optimization
- [ ] Final integration testing
- [ ] Production readiness review

**Total Estimated Duration**: 6-8 weeks

## Appendix

### Related Specifications
- Spec 162: MapReduce Incremental Checkpoint System
- Spec 183: Effect-Based Workflow Execution
- Spec 140: Concurrent Resume Protection
- Spec 171: Variable Aggregation

### References
- [Stillwater Effect System](https://github.com/iepathos/stillwater)
- [Bracket Pattern for Resource Management](https://www.tweag.io/blog/2020-04-09-bracketing/)
- [Retry Patterns in Distributed Systems](https://aws.amazon.com/builders-library/timeouts-retries-and-backoff-with-jitter/)

### Changelog

**v2.0 (2025-11-29)**:
- Fixed integrity hash implementation (exclude hash field)
- Added workflow versioning with SHA-256 hash
- Incorporated Stillwater bracket, retry, validation patterns
- Clarified storage location (global sessions)
- Added checkpoint validation on resume
- Added observability requirements (events, logging, metrics)
- Defined cleanup policies
- Added configuration specification
- Improved testing strategy with Stillwater testing utilities
- Added implementation phases

**v1.0 (2025-11-26)**:
- Initial specification
