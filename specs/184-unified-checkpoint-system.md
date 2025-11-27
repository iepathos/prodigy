---
number: 184
title: Unified Checkpoint System
category: storage
priority: critical
status: draft
dependencies: [162, 183]
created: 2025-11-26
---

# Specification 184: Unified Checkpoint System

**Category**: storage
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 162 (MapReduce Incremental Checkpoint System), Spec 183 (Effect-Based Workflow Execution)

## Context

Prodigy currently has **two separate checkpoint systems** that don't synchronize:

### Current State (Broken)

1. **Session State System** (`SessionUpdate::UpdateWorkflowState`)
   - Updated by `workflow_execution.rs` before/after steps
   - Stores in `UnifiedSession` via `SessionManager`
   - Format: `WorkflowState { current_step, completed_steps, ... }`

2. **Checkpoint File System** (`CheckpointManager::save_checkpoint`)
   - Expected by `resume.rs` for workflow resume
   - Looks for files at: `~/.prodigy/state/{session_id}/checkpoints/*.checkpoint.json`
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

## Requirements

### Functional Requirements

#### FR1: Unified Checkpoint Format
- **MUST** use single `WorkflowCheckpoint` struct for all workflow types
- **MUST** include workflow path for re-execution
- **MUST** include all variables/context for state restoration
- **MUST** support versioning for future compatibility
- **MUST** include integrity hash for corruption detection

#### FR2: Checkpoint Lifecycle
- **MUST** create checkpoint BEFORE each step (with `BeforeStep` state)
- **MUST** create checkpoint AFTER each successful step (with `Completed` state)
- **MUST** create checkpoint on step FAILURE (with `Failed` state)
- **MUST** create checkpoint on signal (SIGINT/SIGTERM)
- **MUST** support configurable checkpoint interval for long workflows

#### FR3: Storage Location Consistency
- **MUST** use canonical path: `~/.prodigy/state/{repo}/sessions/{session_id}/checkpoint.json`
- **MUST** maintain session ↔ checkpoint bidirectional lookup
- **MUST** create directory structure automatically
- **MUST** support checkpoint history (for fallback on corruption)

#### FR4: Atomic Checkpoint Operations
- **MUST** write checkpoints atomically (temp file + rename)
- **MUST** validate checkpoint integrity before committing
- **MUST** handle concurrent checkpoint writes safely
- **MUST** recover from interrupted checkpoint writes

#### FR5: Checkpoint Discovery
- **MUST** find checkpoints by session ID
- **MUST** find checkpoints by workflow path
- **MUST** list all resumable checkpoints
- **MUST** report clear errors when checkpoints missing/corrupt

### Non-Functional Requirements

#### NFR1: Performance
- Checkpoint save MUST complete in <100ms for typical workflows
- Checkpoint load MUST complete in <50ms
- Checkpoint overhead MUST NOT noticeably impact step execution

#### NFR2: Reliability
- Checkpoints MUST survive process crashes
- Checkpoints MUST be recoverable after power loss (atomic writes)
- Corrupt checkpoints MUST be detected and reported

#### NFR3: Maintainability
- Single code path for all checkpoint operations
- Clear separation between checkpoint data and I/O
- Comprehensive logging for debugging

## Acceptance Criteria

### Unified Format

- [ ] **AC1**: Single WorkflowCheckpoint struct
  - Used by both MapReduce and standard workflows
  - Contains workflow path, variables, step state
  - Version field for future compatibility

- [ ] **AC2**: Checkpoint state enum covers all cases
  - `BeforeStep { step_index }` - about to execute
  - `Completed { step_index, output }` - step succeeded
  - `Failed { step_index, error, retryable }` - step failed
  - `Interrupted` - signal received

### Checkpoint Creation

- [ ] **AC3**: Checkpoint before step execution
  - Standard workflow step 3 about to run
  - Checkpoint created with `BeforeStep { step_index: 3 }`
  - Checkpoint exists at canonical location

- [ ] **AC4**: Checkpoint after successful step
  - Step 3 completes successfully
  - Checkpoint updated to `Completed { step_index: 3, output }`
  - Previous checkpoint preserved in history

- [ ] **AC5**: Checkpoint on step failure
  - Step 3 fails with error
  - Checkpoint created with `Failed { step_index: 3, error, retryable: true }`
  - Resume will RETRY this step

- [ ] **AC6**: Checkpoint on SIGINT
  - User presses Ctrl+C during step 3
  - Checkpoint created with `Interrupted` state
  - Step 3 marked as in-progress → pending for resume

### Storage Consistency

- [ ] **AC7**: Canonical storage location
  - Session `session-abc123` creates checkpoint at:
    `~/.prodigy/state/{repo}/sessions/session-abc123/checkpoint.json`
  - Resume finds checkpoint at this location

- [ ] **AC8**: Checkpoint history maintained
  - Each checkpoint write creates timestamped backup
  - `~/.prodigy/state/{repo}/sessions/{id}/history/checkpoint-{timestamp}.json`
  - Fallback to previous checkpoint if latest corrupt

### Resume Integration

- [ ] **AC9**: Resume finds unified checkpoint
  - `prodigy resume session-abc123` finds checkpoint
  - Loads checkpoint from canonical location
  - Extracts workflow path, variables, failed step

- [ ] **AC10**: Resume retries failed step
  - Checkpoint shows `Failed { step_index: 3, retryable: true }`
  - Resume starts execution from step 3 (RETRY, not skip)
  - Variables restored from checkpoint

## Technical Details

### Implementation Approach

#### 1. Unified Checkpoint Data Structure

```rust
/// Unified checkpoint format for all workflow types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    /// Checkpoint format version (for migration)
    pub version: u32,

    /// Session identifier
    pub session_id: SessionId,

    /// Workflow file path (required for re-execution)
    pub workflow_path: PathBuf,

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
    pub mapreduce_state: Option<MapReduceCheckpointState>,

    /// Checkpoint creation timestamp
    pub created_at: DateTime<Utc>,

    /// Checkpoint reason
    pub reason: CheckpointReason,

    /// Integrity hash (SHA-256 of checkpoint content)
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
```

#### 2. Checkpoint Storage Trait

```rust
/// Storage abstraction for checkpoints
#[async_trait]
pub trait CheckpointStorage: Send + Sync {
    /// Save checkpoint atomically
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
}

/// File-based checkpoint storage implementation
pub struct FileCheckpointStorage {
    base_path: PathBuf,
    history_limit: usize,
}

impl FileCheckpointStorage {
    /// Get canonical checkpoint path
    fn checkpoint_path(&self, session_id: &SessionId) -> PathBuf {
        self.base_path
            .join("sessions")
            .join(session_id.as_str())
            .join("checkpoint.json")
    }

    /// Get history directory
    fn history_path(&self, session_id: &SessionId) -> PathBuf {
        self.base_path
            .join("sessions")
            .join(session_id.as_str())
            .join("history")
    }
}

#[async_trait]
impl CheckpointStorage for FileCheckpointStorage {
    async fn save(&self, checkpoint: &WorkflowCheckpoint) -> Result<(), CheckpointError> {
        let path = self.checkpoint_path(&checkpoint.session_id);
        let history_dir = self.history_path(&checkpoint.session_id);

        // Ensure directories exist
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        tokio::fs::create_dir_all(&history_dir).await?;

        // Archive existing checkpoint to history
        if path.exists() {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
            let history_file = history_dir.join(format!("checkpoint-{}.json", timestamp));
            tokio::fs::copy(&path, &history_file).await?;
        }

        // Compute integrity hash
        let json = serde_json::to_string_pretty(&checkpoint)?;
        let hash = compute_sha256(&json);

        let mut checkpoint_with_hash = checkpoint.clone();
        checkpoint_with_hash.integrity_hash = hash;

        // Write atomically (temp file + rename)
        let temp_path = path.with_extension("tmp");
        tokio::fs::write(&temp_path, &json).await?;
        tokio::fs::rename(&temp_path, &path).await?;

        // Cleanup old history
        self.cleanup_history(&checkpoint.session_id).await?;

        Ok(())
    }

    async fn load(&self, session_id: &SessionId) -> Result<Option<WorkflowCheckpoint>, CheckpointError> {
        let path = self.checkpoint_path(session_id);

        if !path.exists() {
            return Ok(None);
        }

        let json = tokio::fs::read_to_string(&path).await?;
        let checkpoint: WorkflowCheckpoint = serde_json::from_str(&json)?;

        // Verify integrity
        let computed_hash = compute_sha256(&serde_json::to_string(&checkpoint)?);
        if computed_hash != checkpoint.integrity_hash {
            return Err(CheckpointError::IntegrityError {
                session_id: session_id.clone(),
                expected: checkpoint.integrity_hash,
                computed: computed_hash,
            });
        }

        Ok(Some(checkpoint))
    }
}
```

#### 3. Effect-Based Checkpoint Operations

```rust
use stillwater::Effect;

/// Save checkpoint effect
pub fn save_checkpoint(state: CheckpointState) -> Effect<(), CheckpointError, WorkflowEnv> {
    Effect::from_async(|env: &WorkflowEnv| async move {
        let checkpoint = WorkflowCheckpoint {
            version: CHECKPOINT_VERSION,
            session_id: env.session_id.clone(),
            workflow_path: env.workflow_path.clone(),
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

        env.checkpoint_storage.save(&checkpoint).await
    })
}

/// Load checkpoint effect
pub fn load_checkpoint(session_id: &SessionId) -> Effect<Option<WorkflowCheckpoint>, CheckpointError, WorkflowEnv> {
    let session_id = session_id.clone();
    Effect::from_async(move |env: &WorkflowEnv| async move {
        env.checkpoint_storage.load(&session_id).await
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
        // Save AFTER success
        .tap(move |result| {
            save_checkpoint(CheckpointState::Completed {
                step_index,
                output: result.output.clone(),
            })
            .map_err(|e| {
                // Log but don't fail on checkpoint error after success
                warn!("Failed to save completion checkpoint: {}", e);
            })
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

#### 4. Resume Logic

```rust
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

/// Resume workflow from checkpoint
pub fn resume_workflow(
    checkpoint: WorkflowCheckpoint,
    workflow: Workflow,
) -> Effect<WorkflowResult, WorkflowError, WorkflowEnv> {
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
}
```

### Architecture Changes

#### Storage Layout

```
~/.prodigy/state/{repo}/
├── sessions/
│   ├── session-abc123/
│   │   ├── checkpoint.json           # Current checkpoint
│   │   └── history/
│   │       ├── checkpoint-20251126_100000.json
│   │       ├── checkpoint-20251126_100030.json
│   │       └── checkpoint-20251126_100100.json
│   └── session-def456/
│       ├── checkpoint.json
│       └── history/
└── mapreduce/jobs/                   # MapReduce jobs (unchanged)
    └── mapreduce-xxx/
```

#### Modified Components

1. **workflow_execution.rs** - Use effect-based checkpointing
2. **resume.rs** - Use unified checkpoint loading
3. **SessionManager** - Delegate to CheckpointStorage
4. **execution_pipeline.rs** - Remove `should_save_checkpoint` logic

#### New Components

1. **CheckpointStorage trait** - Storage abstraction
2. **FileCheckpointStorage** - File-based implementation
3. **checkpoint_effects.rs** - Effect-based operations

## Dependencies

### Prerequisites
- **Spec 162**: MapReduce Incremental Checkpoint System (pattern reference)
- **Spec 183**: Effect-Based Workflow Execution (Effect infrastructure)

### Affected Components
- Session management
- Resume command
- Workflow execution
- MapReduce execution (minor integration)

### External Dependencies
- `sha2` for integrity hashing
- `tokio::fs` for async file operations

## Testing Strategy

### Unit Tests

```rust
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
fn test_plan_resume_from_completed_step() {
    let checkpoint = WorkflowCheckpoint {
        state: CheckpointState::Completed {
            step_index: 3,
            output: Some("done".to_string()),
        },
        ..
    };

    let plan = plan_resume(&checkpoint, &workflow);

    assert_eq!(plan.start_index, 4); // Continue after completed
    assert!(!plan.retry_current);
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_checkpoint_created_on_failure() {
    let storage = InMemoryCheckpointStorage::new();
    let env = create_test_env_with_storage(storage.clone());

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
    let env = create_test_env_with_storage(storage);

    // Resume should retry step 2
    let result = resume_workflow(checkpoint, workflow)
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
    assert!(verify_integrity(&checkpoint));
}
```

## Documentation Requirements

### Code Documentation
- Document CheckpointStorage trait contract
- Document checkpoint state transitions
- Document atomic write guarantees
- Document history/fallback behavior

### User Documentation
- Update resume troubleshooting guide
- Document checkpoint locations
- Explain checkpoint recovery process

## Migration and Compatibility

### Breaking Changes
- Checkpoint format changes (version field enables migration)
- Storage location changes (migration script provided)

### Migration Script

```rust
/// Migrate from old checkpoint format/location to unified system
pub async fn migrate_checkpoints(old_base: &Path, new_base: &Path) -> Result<MigrationReport> {
    let mut report = MigrationReport::new();

    // Find old session state files
    for entry in walk_dir(old_base.join("sessions"))? {
        if let Some(old_state) = load_old_session_state(&entry)? {
            let new_checkpoint = convert_to_unified_checkpoint(old_state)?;
            let new_storage = FileCheckpointStorage::new(new_base);
            new_storage.save(&new_checkpoint).await?;
            report.migrated.push(new_checkpoint.session_id);
        }
    }

    Ok(report)
}
```

### Compatibility
- Old checkpoints auto-detected and migrated on first access
- Version field enables future format changes
- MapReduce checkpoints continue to work (separate storage)

## Success Metrics

### Quantitative
- 100% of workflow failures create checkpoint
- Resume success rate > 95%
- Checkpoint save < 100ms
- Zero checkpoint corruption in tests

### Qualitative
- Single code path for all checkpoint operations
- Clear error messages for missing/corrupt checkpoints
- Reliable resume for standard workflows (matches MapReduce experience)
