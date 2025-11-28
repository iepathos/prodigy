---
number: 183
title: Effect-Based Workflow Execution
category: foundation
priority: critical
status: draft
dependencies: [108, 162]
created: 2025-11-26
revised: 2025-11-28
---

# Specification 183: Effect-Based Workflow Execution (Revised)

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 108 (Functional Programming Adoption), Spec 162 (MapReduce Incremental Checkpoint System)

## Context

Prodigy's workflow execution currently uses an imperative pattern with mixed concerns:
- Business logic intertwined with I/O operations
- Checkpoint logic scattered across execution code
- No built-in retry composition for transient failures
- Two separate state systems (session state vs checkpoint files) that don't synchronize
- Difficult to test due to tight coupling with I/O

The Stillwater library provides functional programming primitives that solve these problems:
- **Effect<Output, Error, Env>**: Composable async computations with environment dependency
- **Reader pattern**: Clean dependency injection via `asks`, `local`
- **Error context**: Context trail for debugging (`ContextError<E>`)
- **Validation**: Error accumulation for comprehensive error reporting

**Note**: Stillwater 0.11.0 includes built-in retry via `Effect::retry()` with `RetryPolicy`.

### Existing Infrastructure

This spec builds on existing Effect-based infrastructure:

| Component | Location | Status |
|-----------|----------|--------|
| `WorkflowEnv` | `src/cook/workflow/effects/environment.rs` | Exists |
| `execute_claude_command_effect` | `src/cook/workflow/effects/claude.rs` | Exists |
| `execute_shell_command_effect` | `src/cook/workflow/effects/shell.rs` | Exists |
| Pure command building | `src/cook/workflow/pure/command_builder.rs` | Exists |
| Pure output parsing | `src/cook/workflow/pure/output_parser.rs` | Exists |
| Variable expansion | `src/cook/workflow/pure/variable_expansion.rs` | Exists |
| Checkpoint management | `src/cook/workflow/checkpoint.rs` | Exists |
| Variable store | `src/cook/workflow/variables.rs` | Exists |

## Objective

Extend workflow execution to use Stillwater's Effect pattern for step composition, enabling:
1. **Pure business logic** that is testable without I/O
2. **Composable retry** for transient failures (Claude 500/overload)
3. **Automatic checkpointing** via effect composition
4. **Clean dependency injection** via the Reader pattern
5. **Comprehensive error context** for debugging

## Requirements

### Functional Requirements

#### FR1: Effect-Based Step Execution
- **MUST** model each workflow step as an Effect using existing `execute_claude_command_effect` and `execute_shell_command_effect`
- **MUST** compose steps using `and_then`, `map` combinators
- **MUST** support both Claude and shell command steps
- **MUST** preserve existing step semantics (variable interpolation, output capture)

#### FR2: Environment Extension
- **MUST** extend existing `WorkflowEnv` with checkpoint storage and session context
- **MUST** use `asks` for accessing environment components
- **MUST** use `local` for scoped modifications (variable updates)
- **MUST** enable testing with mock environments

#### FR3: Pure Business Logic Extraction
- **MUST** add step planning logic as pure functions
- **MUST** add resume planning as pure functions
- **MUST** keep all I/O at effect boundaries
- **SHOULD** reuse existing pure functions (`build_command`, `parse_output_variables`)

#### FR4: Error Context Propagation
- **MUST** use `ContextError<E>` for error context trails
- **MUST** add context at each step boundary
- **MUST** preserve full context trail for debugging
- **MUST** include step index and command info in context

#### FR5: Retry for Transient Errors
- **MUST** implement retry combinator for transient Claude errors (500, overload)
- **MUST** support exponential backoff with configurable parameters
- **MUST** distinguish retryable vs non-retryable errors
- **MUST** track retry attempts in error context

#### FR6: Step Idempotency Awareness
- **MUST** document idempotency requirements for resume safety
- **SHOULD** provide `idempotent: bool` annotation in workflow YAML
- **MUST** log warning when resuming non-idempotent steps
- **MUST** handle `commit_required` validation correctly on resume

### Non-Functional Requirements

#### NFR1: Testability
- Pure functions MUST be testable without I/O
- Effect-based code MUST be testable with mock environments
- Test coverage MUST exceed 80% for new pure functions

#### NFR2: Compatibility
- MUST maintain backward compatibility with existing workflow files
- MUST not change CLI interface
- MUST preserve existing behavior for working features
- MUST integrate with existing `WorkflowCheckpoint` format

#### NFR3: Performance
- Effect overhead MUST NOT exceed 5% compared to current implementation
- Boxing overhead MUST be acceptable for workflow execution timescales

## Acceptance Criteria

### Core Effect Architecture

- [ ] **AC1**: Extended WorkflowEnv with execution context
  - Checkpoint storage added via extension trait or new struct
  - Session ID, workflow path accessible
  - Existing fields preserved

- [ ] **AC2**: Step execution uses existing effects
  - Composes `execute_claude_command_effect` and `execute_shell_command_effect`
  - Claude steps wrapped with retry combinator
  - Shell steps executed via existing effect

- [ ] **AC3**: Workflow execution is Effect composition
  - Steps composed with `and_then`
  - Variables propagated via `WorkflowProgress`
  - Errors propagated with context

### Retry Integration

- [ ] **AC4**: Stillwater retry integrated
  - `retry_helpers.rs` with `claude_retry_policy()` and `shell_retry_policy()`
  - `is_transient_error()` predicate for retryable errors
  - `execute_claude_step_with_retry()` using `Effect::retry()`

### Pure Function Extraction

- [ ] **AC5**: Step planning is pure
  - `plan_steps(workflow) -> Vec<StepPlan>` (no I/O)
  - Testable without environment

- [ ] **AC6**: Resume planning is pure
  - `plan_resume(checkpoint) -> ResumePlan` (no I/O)
  - Determines which step to retry/continue from
  - Handles idempotency warnings

### Testing

- [ ] **AC7**: Pure functions have unit tests
  - Step planning tests
  - Resume planning tests

- [ ] **AC8**: Effects testable with mock environment
  - MockWorkflowEnv for testing
  - Verify effect composition behavior

## Technical Details

### Implementation Approach

#### 1. Extend WorkflowEnv

Rather than replacing the existing `WorkflowEnv`, extend it for execution context:

```rust
// src/cook/workflow/effects/execution_env.rs
use super::environment::WorkflowEnv;
use crate::cook::workflow::checkpoint::CheckpointManager;
use crate::cook::workflow::variables::VariableStore;
use std::path::PathBuf;
use std::sync::Arc;

/// Extended environment for workflow execution with checkpoint support
#[derive(Clone)]
pub struct ExecutionEnv {
    /// Base workflow environment (Claude/shell runners, patterns)
    pub workflow_env: WorkflowEnv,
    /// Session identifier
    pub session_id: String,
    /// Workflow file path (for checkpoint)
    pub workflow_path: PathBuf,
    /// Checkpoint manager
    pub checkpoint_manager: Arc<CheckpointManager>,
    /// Variable store for captured outputs
    pub variable_store: VariableStore,
    /// Verbosity level
    pub verbosity: u8,
}

impl ExecutionEnv {
    /// Create builder for ExecutionEnv
    pub fn builder(workflow_env: WorkflowEnv) -> ExecutionEnvBuilder {
        ExecutionEnvBuilder::new(workflow_env)
    }
}
```

#### 2. Use Stillwater's Built-in Retry

Stillwater 0.11.0 provides `Effect::retry()` with `RetryPolicy`:

```rust
// src/cook/workflow/effects/retry_helpers.rs
use stillwater::retry::{RetryPolicy, RetryStrategy, JitterStrategy, RetryExhausted};
use stillwater::{Effect, EffectExt};
use std::time::Duration;

/// Create retry policy for Claude transient errors
pub fn claude_retry_policy() -> RetryPolicy {
    RetryPolicy::exponential(Duration::from_secs(5))
        .with_max_retries(5)
        .with_jitter(0.25)  // Requires "jitter" feature
}

/// Create retry policy for shell commands (fewer retries)
pub fn shell_retry_policy() -> RetryPolicy {
    RetryPolicy::exponential(Duration::from_secs(2))
        .with_max_retries(2)
}

/// Check if an error is transient (should retry)
pub fn is_transient_error(err: &StepError) -> bool {
    match err {
        StepError::CommandError(CommandError::ExecutionFailed { message, .. }) => {
            message.contains("500")
                || message.contains("overloaded")
                || message.contains("rate limit")
                || message.contains("ECONNRESET")
        }
        _ => false,
    }
}
```

Usage with `Effect::retry()`:

```rust
use stillwater::retry::RetryPolicy;
use stillwater::Effect;

// Retry a Claude command
let effect = Effect::retry(
    || execute_claude_command_effect(&cmd, &vars),
    claude_retry_policy(),
);

// Handle retry exhaustion
let result = effect
    .run(&env)
    .await
    .map_err(|exhausted: RetryExhausted<StepError>| {
        StepError::ClaudeRetryExhausted {
            attempts: exhausted.attempts(),
            last_error: exhausted.error().to_string(),
        }
    });
```

#### 3. Define WorkflowProgress for Variable Propagation

```rust
// src/cook/workflow/effects/progress.rs
use crate::cook::workflow::effects::{CommandOutput, CommandError};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Result from executing a single step
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Whether step succeeded
    pub success: bool,
    /// Captured output
    pub output: Option<String>,
    /// Variables captured from this step
    pub captured_variables: HashMap<String, String>,
    /// Execution duration
    pub duration: Duration,
    /// JSON log location (Claude commands only)
    pub json_log_location: Option<String>,
}

impl StepResult {
    pub fn from_command_output(output: CommandOutput, duration: Duration) -> Self {
        Self {
            success: output.success,
            output: Some(output.stdout),
            captured_variables: output.variables,
            duration,
            json_log_location: output.json_log_location,
        }
    }
}

/// Accumulated progress through workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowProgress {
    /// Completed steps with results
    pub completed_steps: Vec<(usize, StepResult)>,
    /// Accumulated variables from all steps
    pub variables: HashMap<String, Value>,
    /// Current step index
    pub current_step: usize,
    /// Total execution duration
    pub total_duration: Duration,
}

impl WorkflowProgress {
    pub fn new() -> Self {
        Self {
            completed_steps: Vec::new(),
            variables: HashMap::new(),
            current_step: 0,
            total_duration: Duration::ZERO,
        }
    }

    /// Add a completed step result and capture its variables
    pub fn with_step_result(mut self, idx: usize, result: StepResult) -> Self {
        // Capture variables from step output
        for (k, v) in &result.captured_variables {
            self.variables.insert(k.clone(), Value::String(v.clone()));
        }
        self.total_duration += result.duration;
        self.completed_steps.push((idx, result));
        self.current_step = idx + 1;
        self
    }

    /// Convert to final workflow result
    pub fn into_result(self) -> WorkflowResult {
        WorkflowResult {
            success: self.completed_steps.iter().all(|(_, r)| r.success),
            steps_completed: self.completed_steps.len(),
            final_variables: self.variables,
            total_duration: self.total_duration,
        }
    }
}

impl Default for WorkflowProgress {
    fn default() -> Self {
        Self::new()
    }
}

/// Final result of workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    pub success: bool,
    pub steps_completed: usize,
    pub final_variables: HashMap<String, Value>,
    pub total_duration: Duration,
}
```

#### 4. Define Step Error Types

```rust
// src/cook/workflow/effects/step_error.rs
use crate::cook::workflow::effects::CommandError;
use stillwater::ContextError;

/// Errors that can occur during step execution
#[derive(Debug, Clone, thiserror::Error)]
pub enum StepError {
    #[error("Claude command failed after {attempts} attempts: {last_error}")]
    ClaudeRetryExhausted {
        attempts: u32,
        last_error: String,
    },

    #[error("Claude command failed (non-retryable): {0}")]
    ClaudeNonRetryable(String),

    #[error("Shell command exited with code {code:?}: {stderr}")]
    ShellNonZeroExit {
        code: Option<i32>,
        stderr: String,
    },

    #[error("Shell command failed: {0}")]
    ShellFailed(String),

    #[error("Variable interpolation failed: {0}")]
    InterpolationFailed(String),

    #[error("Checkpoint save failed: {0}")]
    CheckpointFailed(String),

    #[error("Command error: {0}")]
    CommandError(#[from] CommandError),
}

impl StepError {
    /// Check if this error is retryable (transient)
    pub fn is_retryable(&self) -> bool {
        matches!(self, StepError::ClaudeRetryExhausted { .. })
    }

    /// Check if the underlying error is transient (for retry decision)
    pub fn is_transient(&self) -> bool {
        match self {
            StepError::CommandError(CommandError::ExecutionFailed { message, .. }) => {
                // Claude 500 errors and overload are transient
                message.contains("500")
                    || message.contains("overloaded")
                    || message.contains("rate limit")
            }
            _ => false,
        }
    }
}

/// Workflow-level errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum WorkflowError {
    #[error("Step {step_index} failed: {error}")]
    StepFailed {
        step_index: usize,
        error: ContextError<StepError>,
    },

    #[error("Workflow validation failed: {0}")]
    ValidationFailed(String),

    #[error("Resume failed: {0}")]
    ResumeFailed(String),
}
```

#### 5. Compose Workflow Execution

```rust
// src/cook/workflow/effects/executor.rs
use super::{
    execute_claude_command_effect, execute_shell_command_effect,
    progress::{StepResult, WorkflowProgress, WorkflowResult},
    retry_helpers::{claude_retry_policy, is_transient_error},
    step_error::{StepError, WorkflowError},
    ExecutionEnv,
};
use crate::cook::workflow::normalized::NormalizedStep;
use crate::cook::workflow::pure::build_command;
use stillwater::{asks, from_async, local, Effect, EffectExt};
use stillwater::retry::{RetryPolicy, RetryExhausted};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Execute a single workflow step
pub fn execute_step(
    step: &NormalizedStep,
    variables: &HashMap<String, String>,
) -> impl Effect<Output = StepResult, Error = StepError, Env = ExecutionEnv> {
    let step = step.clone();
    let variables = variables.clone();

    from_async(move |env: &ExecutionEnv| {
        let step = step.clone();
        let variables = variables.clone();
        let workflow_env = env.workflow_env.clone();

        async move {
            let start = Instant::now();

            let result = match &step {
                NormalizedStep::Claude { command, .. } => {
                    let cmd = build_command(command, &variables);
                    execute_claude_command_effect(&cmd, &variables)
                        .run(&workflow_env)
                        .await
                        .map_err(|e| StepError::CommandError(e))
                }
                NormalizedStep::Shell { command, .. } => {
                    let cmd = build_command(command, &variables);
                    execute_shell_command_effect(&cmd, &variables, None)
                        .run(&workflow_env)
                        .await
                        .map_err(|e| StepError::CommandError(e))
                }
                _ => Ok(super::CommandOutput::success(String::new()))
            }?;

            let duration = start.elapsed();
            Ok(StepResult::from_command_output(result, duration))
        }
    })
}

/// Execute Claude step with built-in retry for transient errors
///
/// Uses Stillwater's Effect::retry() with exponential backoff.
pub fn execute_claude_step_with_retry(
    command: &str,
    variables: &HashMap<String, String>,
) -> impl Effect<Output = StepResult, Error = StepError, Env = ExecutionEnv> {
    let command = command.to_string();
    let variables = variables.clone();

    from_async(move |env: &ExecutionEnv| {
        let command = command.clone();
        let variables = variables.clone();
        let workflow_env = env.workflow_env.clone();

        async move {
            let start = Instant::now();
            let cmd = build_command(&command, &variables);
            let vars = variables.clone();

            // Use Stillwater's built-in retry
            let retry_effect = Effect::retry(
                move || execute_claude_command_effect(&cmd, &vars),
                claude_retry_policy(),
            );

            let output = retry_effect
                .run(&workflow_env)
                .await
                .map_err(|exhausted: RetryExhausted<_>| {
                    StepError::ClaudeRetryExhausted {
                        attempts: exhausted.attempts() as u32,
                        last_error: exhausted.error().to_string(),
                    }
                })?;

            let duration = start.elapsed();
            Ok(StepResult::from_command_output(output, duration))
        }
    })
}

/// Execute entire workflow as composed Effect
pub fn execute_workflow(
    steps: Vec<NormalizedStep>,
    initial_variables: HashMap<String, String>,
) -> impl Effect<Output = WorkflowResult, Error = WorkflowError, Env = ExecutionEnv> {
    from_async(move |env: &ExecutionEnv| {
        let steps = steps.clone();
        let mut variables = initial_variables.clone();
        let env = env.clone();

        async move {
            let mut progress = WorkflowProgress::new();

            for (idx, step) in steps.iter().enumerate() {
                // Convert variables to string map for interpolation
                let var_map: HashMap<String, String> = variables
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                let result = execute_step(&step, &var_map)
                    .run(&env)
                    .await
                    .map_err(|e| WorkflowError::StepFailed {
                        step_index: idx,
                        error: stillwater::ContextError::new(e)
                            .with_context(format!("Executing step {}", idx)),
                    })?;

                // Update variables with captured outputs
                for (k, v) in &result.captured_variables {
                    variables.insert(k.clone(), v.clone());
                }

                progress = progress.with_step_result(idx, result);
            }

            Ok(progress.into_result())
        }
    })
}
```

#### 6. Pure Step Planning

```rust
// src/cook/workflow/pure/step_planning.rs

use crate::cook::workflow::normalized::{NormalizedStep, NormalizedWorkflow};

/// Plan for executing a step
#[derive(Debug, Clone)]
pub struct StepPlan {
    /// Step index
    pub index: usize,
    /// Step to execute
    pub step: NormalizedStep,
    /// Whether this step requires a commit
    pub commit_required: bool,
    /// Whether this step is idempotent (safe to retry)
    pub idempotent: bool,
    /// Dependencies (step indices that must complete first)
    pub depends_on: Vec<usize>,
}

/// Plan steps for workflow execution (pure function)
///
/// Analyzes the workflow and produces a sequence of step plans.
/// This is a pure function with no I/O.
pub fn plan_steps(workflow: &NormalizedWorkflow) -> Vec<StepPlan> {
    workflow
        .steps
        .iter()
        .enumerate()
        .map(|(idx, step)| {
            let commit_required = step.commit_required().unwrap_or(false);
            let idempotent = step.is_idempotent().unwrap_or(true);

            StepPlan {
                index: idx,
                step: step.clone(),
                commit_required,
                idempotent,
                // Sequential by default - all previous steps are dependencies
                depends_on: (0..idx).collect(),
            }
        })
        .collect()
}

/// Determine if a step is safe to resume (idempotent)
pub fn is_safe_to_resume(plan: &StepPlan, was_completed: bool) -> ResumeDecision {
    if was_completed {
        ResumeDecision::Skip
    } else if plan.idempotent {
        ResumeDecision::Execute
    } else {
        ResumeDecision::WarnAndExecute {
            warning: format!(
                "Step {} is not marked as idempotent. Re-execution may have side effects.",
                plan.index
            ),
        }
    }
}

/// Decision for resuming a step
#[derive(Debug, Clone)]
pub enum ResumeDecision {
    /// Skip this step (already completed)
    Skip,
    /// Execute this step
    Execute,
    /// Execute with warning
    WarnAndExecute { warning: String },
}
```

#### 7. Pure Resume Planning

```rust
// src/cook/workflow/pure/resume_planning.rs

use crate::cook::workflow::checkpoint::{WorkflowCheckpoint, WorkflowStatus};

/// Plan for resuming workflow execution
#[derive(Debug, Clone)]
pub struct ResumePlan {
    /// Step index to start from
    pub start_index: usize,
    /// Whether to retry the failed step
    pub retry_failed: bool,
    /// Steps to skip (already completed)
    pub skip_indices: Vec<usize>,
    /// Variables to restore
    pub restore_variables: bool,
    /// Warnings about non-idempotent steps
    pub warnings: Vec<String>,
}

/// Plan resume from checkpoint (pure function)
///
/// Determines where to resume execution based on checkpoint state.
/// This is a pure function with no I/O.
pub fn plan_resume(checkpoint: &WorkflowCheckpoint) -> ResumePlan {
    let completed_indices: Vec<usize> = checkpoint
        .completed_steps
        .iter()
        .map(|s| s.step_index)
        .collect();

    let start_index = checkpoint.execution_state.current_step_index;

    let retry_failed = matches!(
        checkpoint.execution_state.status,
        WorkflowStatus::Failed | WorkflowStatus::Interrupted
    );

    let mut warnings = Vec::new();

    // Check for non-idempotent steps being resumed
    if retry_failed {
        // Note: In production, we'd check step metadata for idempotency
        warnings.push(format!(
            "Resuming from step {}. Verify step is safe to retry.",
            start_index
        ));
    }

    ResumePlan {
        start_index,
        retry_failed,
        skip_indices: completed_indices,
        restore_variables: true,
        warnings,
    }
}

/// Validate checkpoint is compatible with workflow
pub fn validate_checkpoint_compatibility(
    checkpoint: &WorkflowCheckpoint,
    workflow_hash: &str,
    total_steps: usize,
) -> Result<(), String> {
    if checkpoint.workflow_hash != workflow_hash {
        return Err(format!(
            "Workflow has changed since checkpoint (hash mismatch). \
             Checkpoint has {} steps, current workflow has {} steps.",
            checkpoint.total_steps, total_steps
        ));
    }

    if checkpoint.execution_state.current_step_index > total_steps {
        return Err(format!(
            "Checkpoint step index {} exceeds workflow steps {}",
            checkpoint.execution_state.current_step_index, total_steps
        ));
    }

    Ok(())
}
```

### Architecture Changes

#### Directory Structure

```
src/cook/workflow/
├── pure/                      # Pure functions (no I/O)
│   ├── mod.rs                 # EXISTS - extend
│   ├── command_builder.rs     # EXISTS - keep
│   ├── output_parser.rs       # EXISTS - keep
│   ├── variable_expansion.rs  # EXISTS - keep
│   ├── step_planning.rs       # NEW - step planning
│   └── resume_planning.rs     # NEW - resume planning
├── effects/                   # I/O wrapped in Effects
│   ├── mod.rs                 # EXISTS - extend exports
│   ├── environment.rs         # EXISTS - keep WorkflowEnv
│   ├── execution_env.rs       # NEW - ExecutionEnv
│   ├── claude.rs              # EXISTS - keep
│   ├── shell.rs               # EXISTS - keep
│   ├── handler.rs             # EXISTS - keep
│   ├── retry_helpers.rs       # NEW - retry policy helpers (uses stillwater::retry)
│   ├── progress.rs            # NEW - WorkflowProgress
│   ├── step_error.rs          # NEW - StepError types
│   └── executor.rs            # NEW - workflow composition
├── checkpoint.rs              # EXISTS - keep
├── variables.rs               # EXISTS - keep
└── ...
```

#### Modified Components

1. **effects/mod.rs** - Extended exports for new modules
2. **pure/mod.rs** - Extended exports for new pure functions

#### New Components

1. **ExecutionEnv** - Extended environment for execution context
2. **retry_helpers** - Configures `stillwater::retry::RetryPolicy` for Claude/shell
3. **WorkflowProgress** - Variable propagation state
4. **StepError/WorkflowError** - Rich error types with transient detection
5. **Step planning** - Pure step planning functions
6. **Resume planning** - Pure resume planning functions

### Stillwater API Reference

The following Stillwater 0.11.0 APIs are used:

| API | Usage |
|-----|-------|
| `from_async(fn)` | Wrap async closures in Effects |
| `asks(fn)` | Reader pattern - access environment |
| `local(transform, effect)` | Scoped environment modification |
| `pure(value)` | Wrap pure values |
| `fail(error)` | Create failing effects |
| `Effect::retry(factory, policy)` | Built-in retry with exponential backoff |
| `effect.map(fn)` | Transform success value (via `EffectExt`) |
| `effect.and_then(fn)` | Chain effects (via `EffectExt`) |
| `effect.or_else(fn)` | Handle errors with alternative (via `EffectExt`) |
| `effect.tap(fn)` | Side effects without changing value (via `EffectExt`) |
| `effect.map_err(fn)` | Transform errors (via `EffectExt`) |
| `effect.run(&env)` | Execute effect with environment |
| `par_all(effects)` | Parallel execution |
| `par_all_limit(effects, n)` | Bounded parallel execution |

### Integration with Existing Systems

#### Checkpoint Integration (Spec 162)

The new Effect-based execution integrates with existing checkpoints using `tap` for side effects:

```rust
// Integration point: save checkpoint after each step using tap
fn with_checkpointing(
    step_index: usize,
    step: &NormalizedStep,
) -> impl Effect<Output = StepResult, Error = StepError, Env = ExecutionEnv> {
    let step = step.clone();

    execute_step(&step, &HashMap::new())
        // Use tap to save checkpoint without changing the result
        .tap(move |result| {
            from_async(move |env: &ExecutionEnv| {
                let result = result.clone();
                let session_id = env.session_id.clone();
                let checkpoint_manager = env.checkpoint_manager.clone();

                async move {
                    let checkpoint = create_step_checkpoint(&session_id, step_index, &result);
                    checkpoint_manager.save_checkpoint(&checkpoint).await.ok();
                    Ok::<(), StepError>(())
                }
            })
        })
        // Use or_else to save failure checkpoint before propagating error
        .or_else(move |error| {
            from_async(move |env: &ExecutionEnv| {
                let error = error.clone();
                let session_id = env.session_id.clone();
                let checkpoint_manager = env.checkpoint_manager.clone();

                async move {
                    let checkpoint = create_failure_checkpoint(&session_id, step_index, &error);
                    checkpoint_manager.save_checkpoint(&checkpoint).await.ok();
                    Err(error)
                }
            })
        })
}
```

#### Event System Integration

Events are emitted at step boundaries:

```rust
// Events emitted during execution
pub enum WorkflowEvent {
    StepStarted { step_index: usize, command_summary: String },
    StepCompleted { step_index: usize, duration: Duration, success: bool },
    StepFailed { step_index: usize, error: String },
    WorkflowCompleted { total_duration: Duration, steps_completed: usize },
    WorkflowFailed { failed_at_step: usize, error: String },
}
```

#### MapReduce Integration (Spec 173)

`ExecutionEnv` is distinct from `MapEnv`:
- `MapEnv` - For parallel agent execution in MapReduce
- `ExecutionEnv` - For sequential workflow step execution

Both use the same Stillwater patterns (`asks`, `local`, `from_async`).

## Testing Strategy

### Unit Tests for Pure Functions

```rust
#[cfg(test)]
mod step_planning_tests {
    use super::*;

    #[test]
    fn test_plan_steps_sequential() {
        let workflow = create_test_workflow(3);
        let plans = plan_steps(&workflow);

        assert_eq!(plans.len(), 3);
        assert_eq!(plans[0].depends_on, vec![]);
        assert_eq!(plans[1].depends_on, vec![0]);
        assert_eq!(plans[2].depends_on, vec![0, 1]);
    }

    #[test]
    fn test_is_safe_to_resume_completed() {
        let plan = StepPlan {
            index: 0,
            idempotent: true,
            ..Default::default()
        };

        let decision = is_safe_to_resume(&plan, true);
        assert!(matches!(decision, ResumeDecision::Skip));
    }

    #[test]
    fn test_is_safe_to_resume_non_idempotent() {
        let plan = StepPlan {
            index: 0,
            idempotent: false,
            ..Default::default()
        };

        let decision = is_safe_to_resume(&plan, false);
        assert!(matches!(decision, ResumeDecision::WarnAndExecute { .. }));
    }
}

#[cfg(test)]
mod resume_planning_tests {
    use super::*;

    #[test]
    fn test_plan_resume_from_failed() {
        let checkpoint = WorkflowCheckpoint {
            execution_state: ExecutionState {
                current_step_index: 2,
                status: WorkflowStatus::Failed,
                ..Default::default()
            },
            completed_steps: vec![
                CompletedStep { step_index: 0, .. },
                CompletedStep { step_index: 1, .. },
            ],
            ..Default::default()
        };

        let plan = plan_resume(&checkpoint);

        assert_eq!(plan.start_index, 2);
        assert!(plan.retry_failed);
        assert_eq!(plan.skip_indices, vec![0, 1]);
    }
}
```

### Integration Tests with Mock Environment

```rust
#[tokio::test]
async fn test_workflow_execution_success() {
    let mock_claude = MockClaudeRunner::new()
        .with_response(RunnerOutput::success("output 1".to_string()))
        .with_response(RunnerOutput::success("output 2".to_string()));

    let env = ExecutionEnvBuilder::new(
        WorkflowEnv::builder()
            .with_claude_runner(Arc::new(mock_claude))
            .build()
    )
    .with_session_id("test-session")
    .build();

    let steps = vec![
        NormalizedStep::claude("/task1"),
        NormalizedStep::claude("/task2"),
    ];

    let result = execute_workflow(steps, HashMap::new())
        .run(&env)
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.steps_completed, 2);
}

#[tokio::test]
async fn test_retry_on_transient_error() {
    let mock_claude = MockClaudeRunner::new()
        .with_response(RunnerOutput::failure("500 Internal Error".to_string(), 1))
        .with_response(RunnerOutput::failure("500 Internal Error".to_string(), 1))
        .with_response(RunnerOutput::success("success".to_string()));

    let env = create_test_env(mock_claude);

    let result = execute_claude_step_with_retry(
        "/task",
        &HashMap::new(),
        RetryPolicy::exponential(Duration::from_millis(10))
            .with_max_retries(3),
    )
    .run(&env)
    .await
    .unwrap();

    assert!(result.success);
    // Verify 3 attempts were made
}
```

## Migration Path

### Phase 1: Add New Modules
1. Add `effects/retry.rs`, `effects/progress.rs`, `effects/step_error.rs`
2. Add `pure/step_planning.rs`, `pure/resume_planning.rs`
3. Add `effects/execution_env.rs`, `effects/executor.rs`
4. Update module exports

### Phase 2: Integration
1. Create adapter from existing executor to new Effect-based executor
2. Add feature flag `effect_workflow_execution`
3. Run both paths in parallel for validation

### Phase 3: Migration
1. Migrate workflow executor to use new Effect-based implementation
2. Update tests
3. Remove feature flag and old implementation

### Compatibility

- Existing workflows continue to work unchanged
- CLI interface unchanged
- Checkpoint format unchanged (extends existing)
- Session state formats preserved

## Success Metrics

### Quantitative
- Test coverage > 80% for new pure functions
- Effect overhead < 5% vs current implementation
- Zero regressions in existing functionality

### Qualitative
- Code is more testable (pure functions)
- Error messages have full context trail
- Retry behavior is consistent and configurable
- Clear separation between pure logic and I/O

## Open Questions

1. **Retry scope**: Should retry be per-step or configurable per-workflow?
2. **Variable scoping**: Should `local` be used for step-level variable isolation?
3. **Parallel steps**: Future extension to support parallel step execution?
