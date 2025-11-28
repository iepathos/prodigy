---
number: 162
title: MapReduce Incremental Checkpoint System
category: parallel
priority: critical
status: draft
dependencies: [134]
created: 2025-11-13
---

# Specification 162: MapReduce Incremental Checkpoint System

**Category**: parallel
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 134 (MapReduce Checkpoint and Resume)

## Context

MapReduce workflow resume functionality is currently **non-functional** due to missing incremental checkpoint creation during execution. Testing revealed three critical failures:

### Current Behavior (Broken)
- Sequential workflows: ✅ Resume works perfectly
- MapReduce workflows: ❌ Resume completely broken

### Test Evidence

**Test Configuration:**
- 5 work items, max_parallel: 2, each item takes ~2 seconds
- Interrupted after 5 seconds (2 items completed, 2 in-progress, 1 pending)

**Actual Results:**
```bash
$ ls ~/.prodigy/state/session-UUID/mapreduce/
# Directory exists but is EMPTY - no checkpoints created

$ prodigy resume session-UUID
Error: No checkpoints found for session
```

**Expected Results:**
- Checkpoint files should exist: `map-checkpoint-*.json`, `job-state.json`
- Resume should continue from where it left off
- Only pending/failed items should be re-processed

### Root Cause Analysis

1. **No Incremental Checkpoints**: MapReduce only creates checkpoints at major milestones (setup complete, all map items done, reduce complete), NOT during map phase execution
2. **Sequential vs MapReduce**: Sequential workflows checkpoint after EVERY step (works), MapReduce doesn't checkpoint during parallel execution (broken)
3. **All Work Lost**: When interrupted mid-map-phase with 40% completion, all progress is lost

## Objective

Implement an incremental checkpoint system for MapReduce workflows that:
1. **Preserves progress** by creating checkpoints during map phase execution
2. **Enables true resume** from any interruption point
3. **Minimizes checkpoint overhead** while maximizing recovery granularity
4. **Handles all phases** (setup, map, reduce) consistently

## Requirements

### Functional Requirements

#### FR1: Incremental Map Phase Checkpointing
- **MUST** create checkpoints periodically during map phase execution
- **MUST** capture current state: completed items, in-progress items, pending items
- **MUST** preserve partial results from completed agents
- **MUST** save checkpoint after every N agent completions (configurable)
- **MUST** support time-based checkpointing (e.g., every 30 seconds)
- **MUST** handle concurrent agent completion and checkpoint creation safely

#### FR2: Signal-Based Checkpoint Creation
- **MUST** create checkpoint on SIGINT/SIGTERM before graceful shutdown
- **MUST** complete checkpoint save before process termination
- **MUST** mark in-progress items as pending in final checkpoint
- **MUST** preserve all completed work in signal-triggered checkpoint
- **MUST** handle Ctrl+C gracefully with state preservation

#### FR3: Checkpoint Storage Consistency
- **MUST** use consistent storage location across all MapReduce phases
- **MUST** create checkpoints in global storage: `~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/`
- **MUST** avoid session-specific checkpoint paths that cause resume failures
- **MUST** maintain backward compatibility with existing checkpoint format
- **MUST** create necessary directory structure automatically

#### FR4: Work Item State Tracking
- **MUST** track each work item's status: pending, in_progress, completed, failed
- **MUST** save agent results for completed items
- **MUST** reset in-progress items to pending on interrupt
- **MUST** preserve retry counts and failure history
- **MUST** maintain DLQ consistency with checkpoint state

#### FR5: Resume State Reconstruction
- **MUST** load latest checkpoint when resuming MapReduce job
- **MUST** skip setup phase if setup checkpoint exists
- **MUST** only process pending and failed items in map phase
- **MUST** preserve variables and context from setup phase
- **MUST** aggregate results from pre-interrupt + post-resume agents

### Non-Functional Requirements

#### NFR1: Performance
- Checkpoint creation MUST complete in <500ms for 1000 items
- Checkpoint overhead MUST NOT impact agent execution throughput
- Checkpoint I/O MUST be asynchronous and non-blocking
- Resume MUST load checkpoint in <2 seconds for 10,000 items

#### NFR2: Reliability
- Checkpoints MUST be atomic (all-or-nothing writes)
- Checkpoint integrity MUST be verifiable (hash validation)
- Corrupt checkpoints MUST be detectable and reported
- System MUST fall back to previous checkpoint if latest is corrupt

#### NFR3: Scalability
- MUST support checkpointing for jobs with 100,000+ work items
- MUST handle 100+ concurrent agents without checkpoint contention
- MUST cleanup old checkpoints to prevent disk space exhaustion
- MUST compress large checkpoints to reduce storage overhead

#### NFR4: Observability
- MUST log checkpoint creation with timestamp and reason
- MUST expose checkpoint metrics (count, size, save duration)
- MUST track checkpoint success/failure rates
- MUST provide visibility into checkpoint storage usage

## Acceptance Criteria

### Critical Functionality

- [x] **AC1**: Interrupt MapReduce during map phase after 2/5 items complete
  - Resume from checkpoint
  - Only 3 remaining items are processed
  - All 5 items marked complete after resume
  - Reduce phase receives results from all 5 items

- [x] **AC2**: Checkpoint created every N agent completions (default N=5)
  - Start MapReduce with 20 items, max_parallel=5
  - Interrupt after 12 items complete
  - Checkpoint file exists with 12 completed items
  - Resume processes exactly 8 remaining items

- [x] **AC3**: Time-based checkpoint triggers (default 30s)
  - Start long-running MapReduce (items take 5s each)
  - Interrupt after 35 seconds
  - Checkpoint exists despite fewer than N completions
  - Resume continues from time-based checkpoint

- [x] **AC4**: SIGINT checkpoint creation
  - Start MapReduce workflow
  - Send SIGINT (Ctrl+C) during map phase
  - Checkpoint created before process exits
  - Resume successfully from SIGINT checkpoint
  - No work items lost

- [x] **AC5**: Storage location consistency
  - Create MapReduce job with ID `mapreduce-20250101_120000`
  - Interrupt during map phase
  - Checkpoint exists at: `~/.prodigy/state/{repo}/mapreduce/jobs/mapreduce-20250101_120000/map-checkpoint-*.json`
  - `prodigy resume-job mapreduce-20250101_120000` finds and loads checkpoint

- [x] **AC6**: Work item state preservation
  - Interrupt with items in states: 10 completed, 5 in-progress, 15 pending
  - Checkpoint accurately reflects: 10 completed, 0 in-progress, 20 pending
  - Resume processes 20 items (in-progress reset to pending)

### Resume Correctness

- [x] **AC7**: Setup phase skip on resume
  - MapReduce with setup phase
  - Interrupt after setup completes, during map phase
  - Resume does NOT re-execute setup
  - Setup results available to resumed map agents

- [x] **AC8**: Partial map results aggregation
  - 10 items total, 6 complete before interrupt
  - Resume processes 4 remaining items
  - Reduce phase receives aggregated results from all 10 items
  - Variables like `${map.successful}` = 10

- [x] **AC9**: DLQ consistency
  - 5 items fail before interrupt (added to DLQ)
  - Interrupt, then resume
  - DLQ still contains 5 failed items
  - Resume with `--include-dlq-items` retries all 5

- [x] **AC10**: Multiple resume cycles
  - Start MapReduce with 30 items
  - Interrupt after 10 items → Resume
  - Interrupt again after 15 more items → Resume
  - Final resume completes last 5 items
  - All 30 items processed exactly once

### Performance & Reliability

- [x] **AC11**: Checkpoint performance
  - MapReduce with 1000 items
  - Checkpoint creation completes in <500ms
  - Agent throughput unchanged (within 5%)

- [x] **AC12**: Checkpoint integrity
  - Simulate power loss during checkpoint write
  - Resume detects corrupt checkpoint
  - Falls back to previous valid checkpoint
  - Reports corruption error clearly

- [x] **AC13**: Concurrent checkpoint safety
  - 50 agents complete simultaneously
  - Checkpoint creation handles concurrent updates
  - No race conditions or data corruption
  - All 50 completions reflected in checkpoint

### Integration & Usability

- [x] **AC14**: Resume command integration
  - Both `prodigy resume <session-id>` and `prodigy resume-job <job-id>` work
  - Commands auto-detect MapReduce sessions
  - Find checkpoints in correct storage location
  - Report clear progress during resume

- [x] **AC15**: Error handling
  - Resume with no checkpoint → clear error message
  - Resume with corrupt checkpoint → fallback behavior
  - Resume with missing workflow file → helpful error
  - Resume with deleted worktree → reconstruction guidance

## Technical Details

### Stillwater Functional Patterns

This implementation follows the "pure core, imperative shell" pattern using Stillwater's functional abstractions. All checkpoint logic is organized into:
- **Pure functions**: Validation, state transitions, checkpoint preparation
- **Effects**: I/O operations wrapped in composable Effect types
- **Validation**: Error accumulation for comprehensive error reporting

#### Core Patterns Used

| Pattern | Use Case | Benefit |
|---------|----------|---------|
| `Validation<T, Vec<E>>` | Checkpoint validation | Accumulates ALL errors |
| `Effect<T, E, Env>` | Checkpoint I/O | Testable, composable |
| `bracket` | Signal handling | Guaranteed cleanup |
| `asks`/`local` | Configuration access | Clean dependency injection |
| `Semigroup` | Result aggregation | Composable checkpoint merging |

### Implementation Approach

#### 1. Checkpoint Trigger System

```rust
/// Checkpoint trigger configuration
pub struct CheckpointTriggers {
    /// Create checkpoint after every N agent completions
    pub agent_completion_interval: Option<usize>,
    /// Create checkpoint every N seconds
    pub time_interval: Option<Duration>,
    /// Create checkpoint on signal (SIGINT/SIGTERM)
    pub on_signal: bool,
    /// Create checkpoint after each phase completes
    pub on_phase_completion: bool,
}

impl Default for CheckpointTriggers {
    fn default() -> Self {
        Self {
            agent_completion_interval: Some(5),  // Every 5 agents
            time_interval: Some(Duration::from_secs(30)),  // Every 30s
            on_signal: true,
            on_phase_completion: true,
        }
    }
}
```

#### 2. Checkpoint Manager Integration

**Location**: `src/cook/execution/mapreduce/coordination/executor.rs`

```rust
impl MapReduceCoordinator {
    /// Handle agent completion with checkpoint triggering
    async fn on_agent_complete(&mut self, agent_id: &str, result: AgentResult) -> Result<()> {
        // Update state
        self.update_work_item_state(agent_id, result).await?;

        // Check completion-based trigger
        let completed_count = self.get_completed_count().await;
        if let Some(interval) = self.checkpoint_triggers.agent_completion_interval {
            if completed_count % interval == 0 {
                self.save_checkpoint(CheckpointReason::AgentInterval).await?;
            }
        }

        Ok(())
    }

    /// Background task for time-based checkpointing
    async fn checkpoint_timer_task(self: Arc<Self>) {
        if let Some(interval) = self.checkpoint_triggers.time_interval {
            loop {
                tokio::time::sleep(interval).await;
                if let Err(e) = self.save_checkpoint(CheckpointReason::TimeInterval).await {
                    warn!("Time-based checkpoint failed: {}", e);
                }
            }
        }
    }

    /// Save checkpoint with current state
    async fn save_checkpoint(&self, reason: CheckpointReason) -> Result<()> {
        let state = self.capture_current_state().await?;

        let checkpoint = MapReduceCheckpoint {
            job_id: self.job_id.clone(),
            phase: self.current_phase,
            total_items: state.total_items,
            completed_items: state.completed_items,
            failed_items: state.failed_items,
            pending_items: state.pending_items,
            in_progress_items: vec![], // Reset in-progress to pending
            agent_results: state.agent_results,
            setup_output: state.setup_output,
            variables: state.variables,
            created_at: Utc::now(),
            checkpoint_reason: reason,
        };

        self.checkpoint_manager.save_checkpoint(&checkpoint).await?;

        info!(
            "Checkpoint saved: {} items ({} completed, {} failed, {} pending)",
            state.total_items,
            state.completed_items.len(),
            state.failed_items.len(),
            state.pending_items.len()
        );

        Ok(())
    }
}
```

#### 3. Signal Handler Integration with Bracket Pattern

**Stillwater Pattern**: Use `bracket` for guaranteed checkpoint on interrupt:

```rust
use stillwater::effect::bracket::bracket;
use stillwater::prelude::*;

/// Run MapReduce job with guaranteed checkpoint on interrupt
pub fn run_job_with_checkpoint_guarantee<E>(
    job: MapReduceJob,
) -> impl Effect<Output = JobResult, Error = JobError, Env = E>
where
    E: HasCheckpointEnv + HasJobEnv,
{
    bracket(
        // Acquire: initialize job state
        initialize_job_effect(&job),

        // Release: ALWAYS save checkpoint (even on interrupt/error)
        |job_state| async move {
            save_checkpoint_on_shutdown(job_state).await.ok();
        },

        // Use: execute the job phases
        |job_state| execute_job_phases_effect(job_state),
    )
}

/// Effect for saving checkpoint on shutdown
fn save_checkpoint_on_shutdown(
    state: JobState,
) -> impl Effect<Output = CheckpointId, Error = CheckpointError, Env = CheckpointEnv> {
    from_fn(move |env: &CheckpointEnv| async move {
        let checkpoint = prepare_checkpoint(&state, CheckpointReason::BeforeShutdown);
        env.storage.save_checkpoint(&checkpoint).await
    })
}
```

**Legacy Signal Handler (for compatibility)**:

```rust
use tokio::signal;

/// Register signal handlers for graceful shutdown with checkpointing
pub async fn register_signal_handlers(coordinator: Arc<MapReduceCoordinator>) {
    let coordinator_int = coordinator.clone();
    let coordinator_term = coordinator.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("SIGINT received, creating checkpoint before shutdown...");

        if let Err(e) = coordinator_int.save_checkpoint(CheckpointReason::Signal).await {
            warn!("Failed to save checkpoint on SIGINT: {}", e);
        }

        std::process::exit(0);
    });

    #[cfg(unix)]
    tokio::spawn(async move {
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to listen for SIGTERM");

        sigterm.recv().await;
        info!("SIGTERM received, creating checkpoint before shutdown...");

        if let Err(e) = coordinator_term.save_checkpoint(CheckpointReason::Signal).await {
            warn!("Failed to save checkpoint on SIGTERM: {}", e);
        }

        std::process::exit(0);
    });
}
```

#### 4. Checkpoint Validation with Error Accumulation

**Stillwater Pattern**: Use `Validation` to accumulate ALL errors:

```rust
use stillwater::Validation;

/// Checkpoint validation errors
#[derive(Debug, Clone)]
pub enum CheckpointValidationError {
    WorkItemCountMismatch { expected: usize, actual: usize },
    OrphanedAgentAssignment { agent_id: String },
    IntegrityHashMismatch { expected: String, actual: String },
    InvalidPhaseState { phase: PhaseType, reason: String },
    MissingRequiredField { field: String },
    InconsistentTimestamps { field: String, issue: String },
}

/// Validate checkpoint with error accumulation (reports ALL issues)
pub fn validate_checkpoint(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    Validation::all((
        validate_work_item_counts(checkpoint),
        validate_agent_consistency(checkpoint),
        validate_integrity_hash(checkpoint),
        validate_phase_state(checkpoint),
        validate_timestamps(checkpoint),
    ))
    .map(|_| ())
}

/// Pure: validate work item counts
fn validate_work_item_counts(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    let total_accounted = checkpoint.work_item_state.completed_items.len()
        + checkpoint.work_item_state.failed_items.len()
        + checkpoint.work_item_state.pending_items.len()
        + checkpoint.work_item_state.in_progress_items.len();

    if total_accounted == checkpoint.metadata.total_work_items {
        Validation::Success(())
    } else {
        Validation::Failure(vec![CheckpointValidationError::WorkItemCountMismatch {
            expected: checkpoint.metadata.total_work_items,
            actual: total_accounted,
        }])
    }
}

/// Pure: validate agent assignment consistency
fn validate_agent_consistency(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    let orphaned: Vec<_> = checkpoint
        .agent_state
        .agent_assignments
        .keys()
        .filter(|agent_id| !checkpoint.agent_state.active_agents.contains_key(*agent_id))
        .map(|agent_id| CheckpointValidationError::OrphanedAgentAssignment {
            agent_id: agent_id.clone(),
        })
        .collect();

    if orphaned.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(orphaned)
    }
}

/// Pure: validate integrity hash
fn validate_integrity_hash(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    let calculated = calculate_integrity_hash(checkpoint);

    if calculated == checkpoint.metadata.integrity_hash {
        Validation::Success(())
    } else {
        Validation::Failure(vec![CheckpointValidationError::IntegrityHashMismatch {
            expected: checkpoint.metadata.integrity_hash.clone(),
            actual: calculated,
        }])
    }
}
```

#### 5. Pure Checkpoint Preparation (Separated from I/O)

**Stillwater Pattern**: Pure functions for checkpoint creation, Effects for I/O:

```rust
use stillwater::prelude::*;

/// Pure: prepare checkpoint for saving (no I/O)
pub fn prepare_checkpoint(
    state: &MapReduceCheckpoint,
    reason: CheckpointReason,
) -> MapReduceCheckpoint {
    let checkpoint_id = CheckpointId::new();
    let mut checkpoint = state.clone();

    checkpoint.metadata.checkpoint_id = checkpoint_id.to_string();
    checkpoint.metadata.created_at = Utc::now();
    checkpoint.metadata.checkpoint_reason = reason;
    checkpoint.metadata.integrity_hash = calculate_integrity_hash(&checkpoint);

    // Reset in-progress items to pending (safe for resume)
    for (_, progress) in checkpoint.work_item_state.in_progress_items.drain() {
        checkpoint.work_item_state.pending_items.push(progress.work_item);
    }

    checkpoint
}

/// Pure: calculate integrity hash
pub fn calculate_integrity_hash(checkpoint: &MapReduceCheckpoint) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(checkpoint.metadata.job_id.as_bytes());
    hasher.update(checkpoint.metadata.version.to_string().as_bytes());
    hasher.update(format!("{:?}", checkpoint.metadata.phase).as_bytes());
    hasher.update(checkpoint.metadata.total_work_items.to_string().as_bytes());
    hasher.update(checkpoint.work_item_state.completed_items.len().to_string().as_bytes());
    hasher.update(checkpoint.work_item_state.failed_items.len().to_string().as_bytes());

    format!("{:x}", hasher.finalize())
}

/// Effect: save checkpoint to storage
pub fn save_checkpoint_effect(
    checkpoint: MapReduceCheckpoint,
) -> impl Effect<Output = CheckpointId, Error = CheckpointError, Env = CheckpointEnv> {
    from_fn(move |env: &CheckpointEnv| async move {
        env.storage.save_checkpoint(&checkpoint).await?;
        Ok(CheckpointId::from_string(checkpoint.metadata.checkpoint_id.clone()))
    })
}

/// Composed Effect: validate, prepare, and save checkpoint
pub fn create_checkpoint_effect(
    state: &MapReduceCheckpoint,
    reason: CheckpointReason,
) -> impl Effect<Output = CheckpointId, Error = CheckpointError, Env = CheckpointEnv> {
    // Validate first (pure, accumulates all errors)
    let validation_result = validate_checkpoint(state);

    from_validation(validation_result.map_err(CheckpointError::ValidationFailed))
        .and_then(move |_| {
            let checkpoint = prepare_checkpoint(state, reason);
            save_checkpoint_effect(checkpoint)
        })
        .and_then(|id| {
            cleanup_old_checkpoints_effect().map(move |_| id)
        })
        .context("Creating checkpoint")
}
```

#### 6. Reader Pattern for Checkpoint Environment

**Stillwater Pattern**: Use `asks` and `local` for clean configuration access:

```rust
use stillwater::prelude::*;

/// Checkpoint environment for Reader pattern
#[derive(Clone)]
pub struct CheckpointEnv {
    pub storage: Arc<dyn CheckpointStorage>,
    pub config: CheckpointConfig,
    pub job_id: String,
}

/// Get checkpoint interval from environment
pub fn get_checkpoint_interval() -> impl Effect<Output = Option<usize>, Error = Never, Env = CheckpointEnv> {
    asks(|env: &CheckpointEnv| env.config.interval_items)
}

/// Get retention policy from environment
pub fn get_retention_policy() -> impl Effect<Output = Option<RetentionPolicy>, Error = Never, Env = CheckpointEnv> {
    asks(|env: &CheckpointEnv| env.config.retention_policy.clone())
}

/// Override retention policy for specific operation
pub fn with_extended_retention<E, T>(
    effect: E,
) -> impl Effect<Output = T, Error = E::Error, Env = CheckpointEnv>
where
    E: Effect<Env = CheckpointEnv>,
{
    local(
        |env: &CheckpointEnv| CheckpointEnv {
            config: CheckpointConfig {
                retention_policy: Some(RetentionPolicy {
                    max_checkpoints: Some(100), // Extended for migration
                    max_age: None,
                    keep_final: true,
                }),
                ..env.config.clone()
            },
            ..env.clone()
        },
        effect,
    )
}

/// Pure: determine if checkpoint should be created
pub fn should_checkpoint(
    items_processed: usize,
    last_checkpoint_time: DateTime<Utc>,
    current_time: DateTime<Utc>,
    config: &CheckpointConfig,
) -> bool {
    let item_trigger = config.interval_items
        .map(|interval| items_processed >= interval)
        .unwrap_or(false);

    let time_trigger = config.interval_duration
        .map(|interval| {
            let elapsed = current_time.signed_duration_since(last_checkpoint_time);
            elapsed >= chrono::Duration::from_std(interval).unwrap_or_default()
        })
        .unwrap_or(false);

    item_trigger || time_trigger
}
```

#### 7. Pure Work Item State Transitions

**Stillwater Pattern**: Pure state machine, separate from persistence:

```rust
/// Work item status for state machine
#[derive(Debug, Clone, PartialEq)]
pub enum WorkItemStatus {
    Pending,
    InProgress { agent_id: String, started_at: DateTime<Utc> },
    Completed { result: AgentResult },
    Failed { error: String, retry_count: usize },
}

/// Events that trigger state transitions
#[derive(Debug, Clone)]
pub enum WorkItemEvent {
    AgentStart { agent_id: String },
    AgentComplete { result: AgentResult },
    AgentFailed { error: String },
    Interrupt,
    Retry,
}

/// Pure: transition work item state
pub fn transition_work_item(
    current: WorkItemStatus,
    event: WorkItemEvent,
) -> Result<WorkItemStatus, TransitionError> {
    match (current, event) {
        (WorkItemStatus::Pending, WorkItemEvent::AgentStart { agent_id }) => {
            Ok(WorkItemStatus::InProgress {
                agent_id,
                started_at: Utc::now(),
            })
        }
        (WorkItemStatus::InProgress { .. }, WorkItemEvent::AgentComplete { result }) => {
            Ok(WorkItemStatus::Completed { result })
        }
        (WorkItemStatus::InProgress { .. }, WorkItemEvent::AgentFailed { error }) => {
            Ok(WorkItemStatus::Failed { error, retry_count: 1 })
        }
        (WorkItemStatus::InProgress { .. }, WorkItemEvent::Interrupt) => {
            Ok(WorkItemStatus::Pending) // Reset on interrupt
        }
        (WorkItemStatus::Failed { retry_count, .. }, WorkItemEvent::Retry) => {
            Ok(WorkItemStatus::Pending) // Back to pending for retry
        }
        (current, event) => Err(TransitionError::Invalid {
            current: format!("{:?}", current),
            event: format!("{:?}", event),
        }),
    }
}

/// Pure: apply transition to checkpoint work item state
pub fn apply_work_item_transition(
    mut state: WorkItemState,
    item_id: &str,
    event: WorkItemEvent,
) -> Result<WorkItemState, TransitionError> {
    // Find item and apply transition
    // Returns new state without mutation
    // ...
    Ok(state)
}
```

#### 8. Storage Location Unification

**Current (Broken)**: Mixed storage locations causing resume failures
- Some checkpoints: `~/.prodigy/state/session-UUID/mapreduce/`
- Resume looks in: `~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/`

**Fixed**: Single canonical location for all MapReduce state

```rust
/// Pure: get the canonical checkpoint storage directory for a MapReduce job
pub fn get_checkpoint_directory(job_id: &str, prodigy_home: &Path, repo_name: &str) -> PathBuf {
    prodigy_home
        .join("state")
        .join(repo_name)
        .join("mapreduce")
        .join("jobs")
        .join(job_id)
}

/// Effect: ensure checkpoint directory exists
pub fn ensure_checkpoint_directory_effect(
    job_id: &str,
) -> impl Effect<Output = PathBuf, Error = IoError, Env = CheckpointEnv> {
    from_fn(move |env: &CheckpointEnv| async move {
        let dir = get_checkpoint_directory(job_id, &env.prodigy_home, &env.repo_name);
        tokio::fs::create_dir_all(&dir).await?;
        Ok(dir)
    })
}
```

#### 9. Work Item State Machine Diagram

```
┌─────────┐  agent_start   ┌────────────┐
│ Pending │ ───────────────> │ InProgress │
└────┬────┘                 └──────┬─────┘
     ^                              │
     │                              │ agent_complete
     │ interrupt/                   v
     │ resume        ┌──────────┐  │    ┌────────┐
     └───────────────┤ Completed│ <─────┤ Failed │
                     └──────────┘       └────────┘
                                              ^
                                              │ retry_exhausted
                                              │
                                        ┌─────┴────┐
                                        │ Retrying │
                                        └──────────┘
```

**State Transitions:**
- **Pending → InProgress**: Agent starts processing
- **InProgress → Completed**: Agent succeeds
- **InProgress → Failed**: Agent fails
- **InProgress → Pending**: Workflow interrupted (reset)
- **Failed → Retrying**: Retry attempt
- **Failed → DLQ**: Max retries exhausted

#### 6. Checkpoint Data Structure

```rust
/// MapReduce checkpoint format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceCheckpoint {
    /// Job identifier
    pub job_id: String,
    /// Current phase (Setup, Map, Reduce)
    pub phase: Phase,
    /// Total work items
    pub total_items: usize,
    /// Completed work items with results
    pub completed_items: Vec<CompletedItem>,
    /// Failed work items with error details
    pub failed_items: Vec<FailedItem>,
    /// Pending work items (not yet started or reset from in-progress)
    pub pending_items: Vec<PendingItem>,
    /// Setup phase output (if setup completed)
    pub setup_output: Option<SetupOutput>,
    /// Variables captured from execution
    pub variables: HashMap<String, Value>,
    /// Checkpoint creation timestamp
    pub created_at: DateTime<Utc>,
    /// Reason for checkpoint creation
    pub checkpoint_reason: CheckpointReason,
    /// Checkpoint integrity hash
    pub integrity_hash: String,
    /// Checkpoint format version
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckpointReason {
    /// Periodic checkpoint based on agent completion count
    AgentInterval,
    /// Periodic checkpoint based on time
    TimeInterval,
    /// Checkpoint triggered by signal (SIGINT/SIGTERM)
    Signal,
    /// Checkpoint at phase boundary
    PhaseCompletion,
    /// Manual checkpoint request
    Manual,
}
```

### Architecture Changes

Following the "pure core, imperative shell" pattern, checkpoint logic is organized into:

```
src/cook/execution/mapreduce/checkpoint/
├── pure/                           # Pure functions (no I/O)
│   ├── preparation.rs              # Checkpoint preparation
│   ├── validation.rs               # Validation with error accumulation
│   ├── state_transitions.rs        # Work item state machine
│   ├── triggers.rs                 # Checkpoint trigger predicates
│   └── hash.rs                     # Integrity hash calculation
├── effects/                        # I/O effects (Effect<T, E, Env>)
│   ├── storage.rs                  # Save/load checkpoint effects
│   ├── cleanup.rs                  # Retention policy effects
│   └── signal.rs                   # Signal handler effects
├── environment.rs                  # CheckpointEnv for Reader pattern
├── manager.rs                      # High-level checkpoint manager
├── types.rs                        # Data structures
└── mod.rs                          # Module exports
```

#### Modified Components

1. **MapReduceCoordinator** (`src/cook/execution/mapreduce/coordination/executor.rs`)
   - Use `bracket` pattern for guaranteed checkpoint on interrupt
   - Integrate checkpoint effects
   - Track work item states via pure state machine
   - Compose effects for checkpoint triggering

2. **CheckpointManager** (`src/cook/execution/mapreduce/checkpoint/manager.rs`)
   - Refactored to use Effect composition
   - Validation with `Validation<T, Vec<E>>` for error accumulation
   - Pure checkpoint preparation separated from I/O
   - Reader pattern for environment access

3. **Resume Commands** (`src/cli/commands/resume.rs`)
   - Fix storage location paths
   - Unify checkpoint discovery logic
   - Use Validation for comprehensive error reporting
   - Add resume progress reporting

4. **StateManager** (`src/cook/execution/mapreduce/state/`)
   - Pure state transitions (`transition_work_item`)
   - Effects for persistence
   - Sync state to checkpoints via Effect composition
   - Handle concurrent state updates safely

#### New Components (Pure Functions)

1. **CheckpointValidation** (`src/cook/execution/mapreduce/checkpoint/pure/validation.rs`)
   - `validate_checkpoint()` - returns `Validation<(), Vec<CheckpointValidationError>>`
   - `validate_work_item_counts()` - pure predicate
   - `validate_agent_consistency()` - pure predicate
   - `validate_integrity_hash()` - pure predicate

2. **CheckpointPreparation** (`src/cook/execution/mapreduce/checkpoint/pure/preparation.rs`)
   - `prepare_checkpoint()` - pure checkpoint creation
   - `calculate_integrity_hash()` - pure hash calculation
   - `reset_in_progress_items()` - pure state transformation

3. **CheckpointTriggers** (`src/cook/execution/mapreduce/checkpoint/pure/triggers.rs`)
   - `should_checkpoint()` - pure predicate
   - `CheckpointTriggerConfig` - configuration data structure

4. **WorkItemStateMachine** (`src/cook/execution/mapreduce/checkpoint/pure/state_transitions.rs`)
   - `transition_work_item()` - pure state transition
   - `apply_work_item_transition()` - pure state application
   - `WorkItemStatus` / `WorkItemEvent` - state machine types

#### New Components (Effects)

1. **CheckpointEffects** (`src/cook/execution/mapreduce/checkpoint/effects/storage.rs`)
   - `save_checkpoint_effect()` - Effect for saving
   - `load_checkpoint_effect()` - Effect for loading
   - `create_checkpoint_effect()` - Composed validate + prepare + save

2. **SignalEffects** (`src/cook/execution/mapreduce/checkpoint/effects/signal.rs`)
   - `run_job_with_checkpoint_guarantee()` - bracket-based execution
   - `save_checkpoint_on_shutdown()` - guaranteed cleanup effect

3. **CleanupEffects** (`src/cook/execution/mapreduce/checkpoint/effects/cleanup.rs`)
   - `cleanup_old_checkpoints_effect()` - retention policy application
   - `with_extended_retention()` - local environment override

#### New Components (Environment)

1. **CheckpointEnv** (`src/cook/execution/mapreduce/checkpoint/environment.rs`)
   - Reader pattern environment for checkpoint operations
   - `get_checkpoint_interval()` - asks helper
   - `get_retention_policy()` - asks helper
   - `with_extended_retention()` - local helper

### Data Structures

#### Checkpoint Storage Layout

```
~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/
├── setup-checkpoint.json              # Setup phase completion
├── map-checkpoint-1730123456.json     # Map checkpoint #1 (timestamp)
├── map-checkpoint-1730123486.json     # Map checkpoint #2 (timestamp)
├── map-checkpoint-1730123516.json     # Map checkpoint #3 (timestamp)
├── reduce-checkpoint-1730123600.json  # Reduce checkpoint
├── job-state.json                     # Overall job metadata
└── checkpoints/                       # Archive of old checkpoints
    ├── map-checkpoint-1730123426.json # Older checkpoints (retained for fallback)
    └── ...
```

#### Session-Job Mapping

```
~/.prodigy/state/{repo_name}/mappings/
├── session-UUID.json                  # Session → Job mapping
│   {
│     "session_id": "session-UUID",
│     "job_id": "mapreduce-20250101_120000",
│     "created_at": "2025-01-01T12:00:00Z"
│   }
└── job-mapreduce-20250101_120000.json # Job → Session mapping
    {
      "job_id": "mapreduce-20250101_120000",
      "session_id": "session-UUID",
      "created_at": "2025-01-01T12:00:00Z"
    }
```

## Dependencies

### Prerequisites
- **Spec 134**: MapReduce Checkpoint and Resume (foundational checkpoint system)
- **Stillwater**: Functional programming patterns library

### Affected Components
- `MapReduceCoordinator` - execution coordination
- `CheckpointManager` - checkpoint creation and loading
- `StateManager` - work item state tracking
- `Resume commands` - CLI resume interface
- `EventLogger` - checkpoint event tracking

### External Dependencies
- `tokio::signal` - for SIGINT/SIGTERM handling
- `serde_json` - for checkpoint serialization
- `sha2` - for checkpoint integrity hashing
- `stillwater` - for Effect, Validation, and Semigroup patterns

## Testing Strategy

### Pure Function Tests (No I/O, No Mocking)

Pure functions can be tested directly without any async runtime or mocking:

```rust
#[cfg(test)]
mod pure_tests {
    use super::pure::*;
    use stillwater::Validation;

    #[test]
    fn test_should_checkpoint_item_interval() {
        let config = CheckpointConfig {
            interval_items: Some(5),
            interval_duration: None,
            ..Default::default()
        };
        let now = Utc::now();
        let last_checkpoint = now - chrono::Duration::seconds(10);

        // Below threshold
        assert!(!should_checkpoint(3, last_checkpoint, now, &config));

        // At threshold
        assert!(should_checkpoint(5, last_checkpoint, now, &config));

        // Above threshold
        assert!(should_checkpoint(10, last_checkpoint, now, &config));
    }

    #[test]
    fn test_should_checkpoint_time_interval() {
        let config = CheckpointConfig {
            interval_items: None,
            interval_duration: Some(Duration::from_secs(30)),
            ..Default::default()
        };
        let now = Utc::now();

        // Within time interval
        let recent = now - chrono::Duration::seconds(20);
        assert!(!should_checkpoint(100, recent, now, &config));

        // Past time interval
        let old = now - chrono::Duration::seconds(35);
        assert!(should_checkpoint(0, old, now, &config));
    }

    #[test]
    fn test_work_item_state_transitions() {
        // Pending -> InProgress
        let result = transition_work_item(
            WorkItemStatus::Pending,
            WorkItemEvent::AgentStart { agent_id: "agent-1".to_string() },
        );
        assert!(matches!(result, Ok(WorkItemStatus::InProgress { .. })));

        // InProgress -> Completed
        let result = transition_work_item(
            WorkItemStatus::InProgress { agent_id: "agent-1".to_string(), started_at: Utc::now() },
            WorkItemEvent::AgentComplete { result: mock_result() },
        );
        assert!(matches!(result, Ok(WorkItemStatus::Completed { .. })));

        // InProgress -> Pending on interrupt
        let result = transition_work_item(
            WorkItemStatus::InProgress { agent_id: "agent-1".to_string(), started_at: Utc::now() },
            WorkItemEvent::Interrupt,
        );
        assert!(matches!(result, Ok(WorkItemStatus::Pending)));

        // Invalid transition: Pending -> Completed
        let result = transition_work_item(
            WorkItemStatus::Pending,
            WorkItemEvent::AgentComplete { result: mock_result() },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_checkpoint_validation_accumulates_all_errors() {
        // Create checkpoint with multiple issues
        let checkpoint = create_checkpoint_with_issues();

        let result = validate_checkpoint(&checkpoint);

        match result {
            Validation::Success(_) => panic!("Expected validation failures"),
            Validation::Failure(errors) => {
                // Should have accumulated ALL errors, not just the first
                assert!(errors.len() >= 2, "Expected multiple errors, got {}", errors.len());

                // Check specific error types are present
                assert!(errors.iter().any(|e| matches!(e, CheckpointValidationError::WorkItemCountMismatch { .. })));
                assert!(errors.iter().any(|e| matches!(e, CheckpointValidationError::OrphanedAgentAssignment { .. })));
            }
        }
    }

    #[test]
    fn test_prepare_checkpoint_resets_in_progress() {
        let mut state = create_test_checkpoint_state();
        state.work_item_state.in_progress_items.insert(
            "item-1".to_string(),
            WorkItemProgress { /* ... */ },
        );

        let prepared = prepare_checkpoint(&state, CheckpointReason::Manual);

        // In-progress items should be moved to pending
        assert!(prepared.work_item_state.in_progress_items.is_empty());
        assert!(prepared.work_item_state.pending_items.iter().any(|i| i.id == "item-1"));
    }

    #[test]
    fn test_integrity_hash_deterministic() {
        let checkpoint = create_test_checkpoint("job-1");

        let hash1 = calculate_integrity_hash(&checkpoint);
        let hash2 = calculate_integrity_hash(&checkpoint);

        assert_eq!(hash1, hash2, "Hash should be deterministic");
    }

    #[test]
    fn test_integrity_hash_changes_on_state_change() {
        let mut checkpoint = create_test_checkpoint("job-1");
        let hash1 = calculate_integrity_hash(&checkpoint);

        checkpoint.metadata.completed_items += 1;
        let hash2 = calculate_integrity_hash(&checkpoint);

        assert_ne!(hash1, hash2, "Hash should change when state changes");
    }
}
```

### Effect Tests (With Mock Environment)

Effects are tested with mock environments:

```rust
#[cfg(test)]
mod effect_tests {
    use super::effects::*;
    use stillwater::prelude::*;

    #[tokio::test]
    async fn test_save_checkpoint_effect() {
        let mock_storage = MockCheckpointStorage::new();
        let env = CheckpointEnv {
            storage: Arc::new(mock_storage),
            config: CheckpointConfig::default(),
            job_id: "test-job".to_string(),
        };

        let checkpoint = create_test_checkpoint("test-job");
        let effect = save_checkpoint_effect(checkpoint);

        let result = effect.run(&env).await;
        assert!(result.is_ok());
        assert!(env.storage.was_called_with("save_checkpoint"));
    }

    #[tokio::test]
    async fn test_create_checkpoint_effect_validates_first() {
        let mock_storage = MockCheckpointStorage::new();
        let env = CheckpointEnv {
            storage: Arc::new(mock_storage),
            config: CheckpointConfig::default(),
            job_id: "test-job".to_string(),
        };

        // Create invalid checkpoint
        let invalid_checkpoint = create_invalid_checkpoint();
        let effect = create_checkpoint_effect(&invalid_checkpoint, CheckpointReason::Manual);

        let result = effect.run(&env).await;

        // Should fail validation before attempting I/O
        assert!(result.is_err());
        assert!(!env.storage.was_called(), "Should not save invalid checkpoint");
    }

    #[tokio::test]
    async fn test_reader_pattern_environment_access() {
        let env = CheckpointEnv {
            config: CheckpointConfig {
                interval_items: Some(10),
                ..Default::default()
            },
            ..mock_env()
        };

        let effect = get_checkpoint_interval();
        let result = effect.run(&env).await.unwrap();

        assert_eq!(result, Some(10));
    }

    #[tokio::test]
    async fn test_local_override() {
        let env = CheckpointEnv {
            config: CheckpointConfig {
                retention_policy: Some(RetentionPolicy {
                    max_checkpoints: Some(5),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..mock_env()
        };

        // Inner effect reads retention policy
        let inner = get_retention_policy();

        // Wrap with extended retention
        let effect = with_extended_retention(inner);

        let result = effect.run(&env).await.unwrap();

        // Should see extended retention (100), not original (5)
        assert_eq!(result.unwrap().max_checkpoints, Some(100));
    }
}
```

### Legacy Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_checkpoint_on_agent_completion_interval() {
        // Create coordinator with interval=5
        let coordinator = create_test_coordinator(5);

        // Complete 4 agents - no checkpoint
        for i in 0..4 {
            coordinator.on_agent_complete(&format!("agent-{}", i), success_result()).await;
        }
        assert_eq!(checkpoint_count(), 0);

        // Complete 5th agent - checkpoint triggered
        coordinator.on_agent_complete("agent-4", success_result()).await;
        assert_eq!(checkpoint_count(), 1);
    }

    #[tokio::test]
    async fn test_checkpoint_on_time_interval() {
        let coordinator = create_test_coordinator_with_time_interval(Duration::from_secs(5));

        // Wait 6 seconds
        tokio::time::sleep(Duration::from_secs(6)).await;

        // Checkpoint should have been created
        assert_ge!(checkpoint_count(), 1);
    }

    #[tokio::test]
    async fn test_in_progress_reset_on_checkpoint() {
        let coordinator = create_test_coordinator(1);

        // Start 3 agents (in-progress)
        coordinator.start_agent("agent-1").await;
        coordinator.start_agent("agent-2").await;
        coordinator.start_agent("agent-3").await;

        // Create checkpoint
        coordinator.save_checkpoint(CheckpointReason::Manual).await;

        // Load checkpoint
        let checkpoint = load_latest_checkpoint().await;

        // In-progress items should be reset to pending
        assert_eq!(checkpoint.in_progress_items.len(), 0);
        assert_eq!(checkpoint.pending_items.len(), 3);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_mapreduce_resume_after_interrupt() {
    // Start MapReduce with 10 items
    let job_id = start_mapreduce_job("test-mapreduce-resume.yml").await;

    // Wait for 5 items to complete
    wait_for_completed_items(&job_id, 5).await;

    // Interrupt the job
    send_sigint_to_job(&job_id).await;

    // Verify checkpoint created
    let checkpoint = load_latest_checkpoint(&job_id).await.unwrap();
    assert_eq!(checkpoint.completed_items.len(), 5);
    assert_eq!(checkpoint.pending_items.len(), 5);

    // Resume the job
    let result = resume_job(&job_id).await.unwrap();

    // Verify only 5 remaining items processed
    assert_eq!(result.items_processed, 5);
    assert_eq!(result.total_completed, 10);
}

#[tokio::test]
async fn test_multiple_resume_cycles() {
    let job_id = start_mapreduce_job_with_items(30).await;

    // Interrupt after 10 items
    wait_and_interrupt(&job_id, 10).await;

    // Resume
    resume_job(&job_id).await.unwrap();

    // Interrupt again after 15 more items
    wait_and_interrupt(&job_id, 25).await;

    // Resume again
    let result = resume_job(&job_id).await.unwrap();

    // Verify all 30 items processed exactly once
    assert_eq!(result.total_completed, 30);
    assert_no_duplicate_processing(&job_id);
}
```

### Performance Tests

```rust
#[tokio::test]
async fn test_checkpoint_performance_large_job() {
    let coordinator = create_coordinator_with_items(1000);

    let start = Instant::now();
    coordinator.save_checkpoint(CheckpointReason::Manual).await.unwrap();
    let duration = start.elapsed();

    // Checkpoint should complete in <500ms for 1000 items
    assert!(duration < Duration::from_millis(500));
}

#[tokio::test]
async fn test_concurrent_agent_completion_checkpoint_safety() {
    let coordinator = Arc::new(create_test_coordinator(1));

    // Spawn 50 agents that complete simultaneously
    let handles: Vec<_> = (0..50)
        .map(|i| {
            let coord = coordinator.clone();
            tokio::spawn(async move {
                coord.on_agent_complete(&format!("agent-{}", i), success_result()).await
            })
        })
        .collect();

    // Wait for all completions
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify checkpoint integrity
    let checkpoint = load_latest_checkpoint().await.unwrap();
    assert_eq!(checkpoint.completed_items.len(), 50);

    // No duplicates or missing items
    assert_no_duplicates(&checkpoint);
}
```

### User Acceptance Tests

1. **UAT1: Basic Resume**
   - User starts MapReduce workflow with 10 items
   - User presses Ctrl+C after 5 items complete
   - User runs `prodigy sessions list` to find session ID
   - User runs `prodigy resume <session-id>`
   - System resumes and processes remaining 5 items
   - User verifies all 10 items completed in reduce phase

2. **UAT2: Resume Discovery**
   - User interrupts MapReduce job
   - User runs `prodigy sessions list` → sees session
   - User runs `prodigy resume-job list` → sees job
   - User can resume using either session ID or job ID
   - Both commands work identically

3. **UAT3: Progress Visibility**
   - User resumes interrupted MapReduce job
   - System shows: "Resuming from checkpoint (15/50 items completed)"
   - System shows: "Processing 35 remaining items..."
   - User sees clear progress throughout resume

## Documentation Requirements

### Code Documentation

- Document checkpoint trigger configuration options
- Explain signal handling and graceful shutdown
- Document checkpoint data format and versioning
- Explain work item state machine transitions

### User Documentation

**Update**: `docs/mapreduce/checkpoint-and-resume.md`

Add sections:
- How checkpoints are created during execution
- What happens when you interrupt a MapReduce job
- How to resume from checkpoints
- Troubleshooting checkpoint issues

**Update**: `docs/reference/troubleshooting.md`

Add entries:
- "MapReduce job resume fails" → check checkpoint location
- "All work lost after interrupt" → verify checkpoint creation
- "Resume processes all items again" → checkpoint integrity issue

### Architecture Updates

**Update**: `ARCHITECTURE.md`

Add:
- Checkpoint trigger system architecture
- Signal handling flow diagram
- Work item state machine diagram
- Checkpoint storage layout specification

## Implementation Notes

### Checkpoint Frequency Tuning

**Too Frequent** (e.g., every agent):
- High I/O overhead
- Reduced throughput
- Unnecessary disk usage

**Too Infrequent** (e.g., only on phase completion):
- Large work loss on interrupt
- Poor user experience
- Defeats purpose of checkpointing

**Recommended Defaults**:
- Agent interval: 5 agents (balances overhead and granularity)
- Time interval: 30 seconds (catches long-running items)
- Always: on phase completion and signals

### Atomic Checkpoint Writes

```rust
async fn save_checkpoint_atomic(checkpoint: &MapReduceCheckpoint, path: &Path) -> Result<()> {
    // Write to temporary file first
    let temp_path = path.with_extension("tmp");
    let json = serde_json::to_string_pretty(checkpoint)?;
    tokio::fs::write(&temp_path, json).await?;

    // Atomic rename (on Unix)
    tokio::fs::rename(&temp_path, path).await?;

    Ok(())
}
```

### Checkpoint Cleanup Policy

```rust
pub struct CheckpointRetentionPolicy {
    /// Keep last N checkpoints
    pub keep_last: usize,
    /// Keep checkpoints from last N hours
    pub keep_duration: Duration,
    /// Always keep phase completion checkpoints
    pub keep_phase_checkpoints: bool,
}

impl Default for CheckpointRetentionPolicy {
    fn default() -> Self {
        Self {
            keep_last: 5,
            keep_duration: Duration::from_secs(24 * 3600),  // 24 hours
            keep_phase_checkpoints: true,
        }
    }
}
```

## Migration and Compatibility

### Breaking Changes

**None** - This is additive functionality. Existing checkpoints (if any) remain compatible.

### Migration Requirements

**For Existing Jobs**: No migration needed. New checkpoint system applies to new jobs only.

**For Resume Commands**: Commands now search in correct location - existing broken resume attempts will now work.

### Compatibility Considerations

1. **Checkpoint Format Versioning**: Include `version` field in checkpoint JSON for future compatibility
2. **Fallback Behavior**: If no checkpoint exists, behavior unchanged (start from beginning)
3. **Legacy Storage**: Old checkpoint locations (if any) are ignored; new canonical location used

### Rollback Plan

If issues arise:
1. Disable incremental checkpointing via config flag
2. Fall back to phase-boundary-only checkpoints
3. Resume still works (just less granular)
4. No data loss or corruption risk

## Success Metrics

### Quantitative Metrics

1. **Resume Success Rate**: >95% of MapReduce resume attempts succeed
2. **Work Preservation**: >90% of completed work preserved on interrupt
3. **Checkpoint Overhead**: <5% throughput reduction from checkpointing
4. **Resume Time**: <5 seconds to resume jobs with 1000+ items

### Qualitative Metrics

1. **User Confidence**: Users trust they can interrupt and resume MapReduce jobs
2. **Error Clarity**: Resume error messages guide users to resolution
3. **Consistency**: Behavior matches sequential workflow resume experience

## Stillwater Migration Summary

### Pattern Usage

| Pattern | Location | Purpose |
|---------|----------|---------|
| `Validation<T, Vec<E>>` | `pure/validation.rs` | Accumulate ALL validation errors |
| `Effect<T, E, Env>` | `effects/storage.rs` | Composable I/O operations |
| `bracket` | `effects/signal.rs` | Guaranteed checkpoint on interrupt |
| `asks`/`local` | `environment.rs` | Reader pattern for config |
| Pure functions | `pure/*.rs` | Testable business logic |

### Migration Benefits

1. **Testability**: Pure functions test without async runtime or mocking
2. **Error Reporting**: Users see ALL checkpoint validation errors at once
3. **Composability**: Effects chain together cleanly with `and_then`, `map`
4. **Reliability**: `bracket` guarantees checkpoint on any exit path
5. **Flexibility**: Reader pattern enables scoped configuration overrides

### Key Refactoring Steps

1. **Extract Pure Functions**:
   - `validate_checkpoint()` → returns `Validation<(), Vec<E>>`
   - `prepare_checkpoint()` → no I/O
   - `should_checkpoint()` → pure predicate
   - `transition_work_item()` → pure state machine

2. **Wrap I/O in Effects**:
   - `save_checkpoint_effect()` → Effect for storage
   - `load_checkpoint_effect()` → Effect for loading
   - `cleanup_old_checkpoints_effect()` → Effect for cleanup

3. **Add Reader Pattern**:
   - `CheckpointEnv` → environment type
   - `get_*()` → asks helpers
   - `with_*()` → local overrides

4. **Apply Bracket for Safety**:
   - `run_job_with_checkpoint_guarantee()` → guaranteed cleanup

### Existing Code That Can Be Reused

- `src/cook/execution/mapreduce/pure/` - existing pure module structure
- `src/cook/execution/variables/semigroup.rs` - Semigroup implementations
- `src/cook/execution/mapreduce/checkpoint/types.rs` - data structures (unchanged)
- `src/cook/execution/mapreduce/checkpoint/storage.rs` - storage trait (wrap in Effect)

## References

- **RESUME_TEST_RESULTS.md**: Comprehensive test analysis and failure documentation
- **test-mapreduce-resume.yml**: Test workflow for validation
- **Spec 134**: MapReduce Checkpoint and Resume (foundation)
- **Stillwater README**: Functional programming patterns library
- **CLAUDE.md**: Resume functionality documentation
