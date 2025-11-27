---
number: 183
title: Effect-Based Workflow Execution
category: foundation
priority: critical
status: draft
dependencies: [108, 162]
created: 2025-11-26
---

# Specification 183: Effect-Based Workflow Execution

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
- **Effect<T, E, Env>**: Composable async computations with environment dependency
- **Reader pattern**: Clean dependency injection via `asks`, `ask`, `local`
- **Retry policies**: Built-in retry with exponential backoff, jitter, and hooks
- **Error context**: Context trail for debugging (`ContextError<E>`)
- **Validation**: Error accumulation for comprehensive error reporting

This specification defines how to restructure workflow execution using Stillwater's Effect pattern to achieve the "pure core, imperative shell" architecture.

## Objective

Restructure workflow execution to use Stillwater's Effect pattern, enabling:
1. **Pure business logic** that is testable without I/O
2. **Composable retry** for transient failures (Claude 500/overload)
3. **Automatic checkpointing** via effect composition
4. **Clean dependency injection** via the Reader pattern
5. **Comprehensive error context** for debugging

## Requirements

### Functional Requirements

#### FR1: Effect-Based Step Execution
- **MUST** model each workflow step as an `Effect<StepResult, StepError, WorkflowEnv>`
- **MUST** compose steps using `and_then`, `map`, and other Effect combinators
- **MUST** support both Claude and shell command steps
- **MUST** preserve existing step semantics (variable interpolation, output capture)

#### FR2: Environment-Based Dependency Injection
- **MUST** define `WorkflowEnv` containing all execution dependencies
- **MUST** use `Effect::asks` for accessing environment components
- **MUST** use `Effect::local` for scoped modifications (variable updates)
- **MUST** enable testing with mock environments

#### FR3: Pure Business Logic Extraction
- **MUST** extract step planning logic as pure functions
- **MUST** extract variable interpolation as pure functions
- **MUST** extract resume planning as pure functions
- **MUST** keep all I/O at effect boundaries

#### FR4: Error Context Propagation
- **MUST** use `ContextError<E>` for error context trails
- **MUST** add context at each step boundary
- **MUST** preserve full context trail for debugging
- **MUST** include step index and command info in context

### Non-Functional Requirements

#### NFR1: Testability
- Pure functions MUST be testable without I/O
- Effect-based code MUST be testable with mock environments
- Test coverage MUST exceed 80% for pure functions

#### NFR2: Compatibility
- MUST maintain backward compatibility with existing workflow files
- MUST not change CLI interface
- MUST preserve existing behavior for working features

#### NFR3: Performance
- Effect overhead MUST NOT exceed 5% compared to current implementation
- Boxing overhead MUST be acceptable for workflow execution timescales

## Acceptance Criteria

### Core Effect Architecture

- [ ] **AC1**: WorkflowEnv defined with all execution dependencies
  - Claude executor, subprocess manager, checkpoint storage
  - Session ID, worktree path, variables
  - Accessible via `Effect::asks`

- [ ] **AC2**: Step execution modeled as Effect
  - `execute_step(step) -> Effect<StepResult, StepError, WorkflowEnv>`
  - Claude steps wrapped with retry policy
  - Shell steps executed via subprocess

- [ ] **AC3**: Workflow execution is Effect composition
  - Steps composed with `and_then`
  - Variables updated via `Effect::local`
  - Errors propagated with context

### Pure Function Extraction

- [ ] **AC4**: Step planning is pure
  - `plan_steps(workflow) -> Vec<StepPlan>` (no I/O)
  - Testable without environment

- [ ] **AC5**: Variable interpolation is pure
  - `interpolate(template, vars) -> String` (no I/O)
  - All variable resolution logic extracted

- [ ] **AC6**: Resume planning is pure
  - `plan_resume(checkpoint) -> ResumePlan` (no I/O)
  - Determines which step to retry/continue from

### Testing

- [ ] **AC7**: Pure functions have unit tests
  - Step planning tests
  - Variable interpolation tests
  - Resume planning tests

- [ ] **AC8**: Effects testable with mock environment
  - MockWorkflowEnv for testing
  - Verify effect composition behavior

## Technical Details

### Implementation Approach

#### 1. Define WorkflowEnv

```rust
/// Environment for workflow execution effects
pub struct WorkflowEnv {
    /// Session identifier
    pub session_id: SessionId,
    /// Worktree path for execution
    pub worktree_path: PathBuf,
    /// Workflow file path (for checkpoint)
    pub workflow_path: PathBuf,
    /// Claude command executor
    pub claude_executor: Arc<dyn ClaudeExecutor>,
    /// Subprocess manager for shell commands
    pub subprocess: SubprocessManager,
    /// Checkpoint storage
    pub checkpoint_storage: Arc<dyn CheckpointStorage>,
    /// Current variables
    pub variables: HashMap<String, Value>,
    /// Verbosity level
    pub verbosity: u8,
}
```

#### 2. Define Step Execution Effect

```rust
use stillwater::{Effect, EffectContext, RetryPolicy};

/// Execute a single workflow step
pub fn execute_step(step: &WorkflowStep) -> Effect<StepResult, StepError, WorkflowEnv> {
    match step {
        WorkflowStep::Claude(cmd) => execute_claude_step(cmd),
        WorkflowStep::Shell(cmd) => execute_shell_step(cmd),
    }
    .context(format!("Executing step: {}", step.summary()))
}

/// Execute Claude command with retry for transient errors
fn execute_claude_step(cmd: &str) -> Effect<StepResult, StepError, WorkflowEnv> {
    // Get interpolated command
    let interpolated = Effect::asks(|env: &WorkflowEnv| {
        interpolate_command(cmd, &env.variables)
    });

    interpolated.and_then(|cmd| {
        Effect::retry_if(
            move || raw_claude_effect(&cmd),
            RetryPolicy::exponential(Duration::from_secs(5))
                .with_max_retries(5)
                .with_jitter(0.25),
            |err| err.is_transient(), // 500, overload = true
        )
        .map(|retry_result| retry_result.into_value())
        .map_err(|retry_exhausted| StepError::ClaudeRetryExhausted {
            attempts: retry_exhausted.attempts(),
            last_error: retry_exhausted.into_error(),
        })
    })
}

/// Execute shell command
fn execute_shell_step(cmd: &str) -> Effect<StepResult, StepError, WorkflowEnv> {
    Effect::asks(|env: &WorkflowEnv| {
        interpolate_command(cmd, &env.variables)
    })
    .and_then(|cmd| {
        Effect::from_async(move |env: &WorkflowEnv| async move {
            let output = env.subprocess
                .runner()
                .run(ProcessCommand::shell(&cmd, &env.worktree_path))
                .await
                .map_err(|e| StepError::ShellFailed(e))?;

            if output.status.success() {
                Ok(StepResult::success(output.stdout))
            } else {
                Err(StepError::ShellNonZeroExit {
                    code: output.status.code(),
                    stderr: output.stderr,
                })
            }
        })
    })
}
```

#### 3. Compose Workflow Execution

```rust
/// Execute entire workflow as composed Effect
pub fn execute_workflow(
    steps: Vec<WorkflowStep>,
) -> Effect<WorkflowResult, WorkflowError, WorkflowEnv> {
    steps.into_iter()
        .enumerate()
        .fold(
            Effect::pure(WorkflowProgress::new()),
            |acc, (idx, step)| {
                acc.and_then(move |progress| {
                    with_checkpointing(idx, &step)
                        .map(|result| progress.with_step_result(idx, result))
                })
            },
        )
        .map(|progress| progress.into_result())
}

/// Wrap step with checkpoint side-effects
fn with_checkpointing(
    step_index: usize,
    step: &WorkflowStep,
) -> Effect<StepResult, StepError, WorkflowEnv> {
    // Save checkpoint BEFORE step (so we know where to resume)
    save_checkpoint(CheckpointState::BeforeStep { step_index })
        .and_then(move |_| execute_step(step))
        .tap(move |result| {
            // Save checkpoint AFTER success
            save_checkpoint(CheckpointState::Completed {
                step_index,
                output: result.output.clone(),
            })
        })
        .or_else(move |error| {
            // Save checkpoint on FAILURE (critical for resume!)
            save_checkpoint(CheckpointState::Failed {
                step_index,
                error: error.to_string(),
                retryable: error.is_retryable(),
            })
            .and_then(|_| Effect::fail(error))
        })
}
```

#### 4. Environment Access Helpers

```rust
/// Reader pattern helpers for WorkflowEnv
pub mod env_helpers {
    use super::*;

    pub fn get_variables() -> Effect<HashMap<String, Value>, WorkflowError, WorkflowEnv> {
        Effect::asks(|env| env.variables.clone())
    }

    pub fn get_worktree() -> Effect<PathBuf, WorkflowError, WorkflowEnv> {
        Effect::asks(|env| env.worktree_path.clone())
    }

    pub fn get_checkpoint_storage() -> Effect<Arc<dyn CheckpointStorage>, WorkflowError, WorkflowEnv> {
        Effect::asks(|env| env.checkpoint_storage.clone())
    }

    /// Update variables for nested effect
    pub fn with_updated_variables<T, E>(
        updates: HashMap<String, Value>,
        effect: Effect<T, E, WorkflowEnv>,
    ) -> Effect<T, E, WorkflowEnv>
    where
        T: Send + 'static,
        E: Send + 'static,
    {
        Effect::local(
            move |env| {
                let mut new_vars = env.variables.clone();
                new_vars.extend(updates);
                WorkflowEnv { variables: new_vars, ..env.clone() }
            },
            effect,
        )
    }
}
```

### Architecture Changes

#### Directory Structure

```
src/cook/workflow/
├── pure/                      # Pure functions (no I/O)
│   ├── mod.rs
│   ├── step_planning.rs       # plan_steps(), determine step order
│   ├── interpolation.rs       # Variable interpolation
│   └── resume_planning.rs     # plan_resume() from checkpoint
├── effects/                   # I/O wrapped in Effects
│   ├── mod.rs
│   ├── claude.rs              # Claude command execution with retry
│   ├── shell.rs               # Shell command execution
│   ├── checkpoint.rs          # Checkpoint save/load effects
│   └── worktree.rs            # Worktree operations
├── environment.rs             # WorkflowEnv definition
├── executor.rs                # Effect composition for workflow
├── error.rs                   # StepError, WorkflowError definitions
└── resume.rs                  # Resume orchestration
```

#### Modified Components

1. **WorkflowExecutor** - Rewritten to use Effect composition
2. **ResumeExecutor** - Uses pure resume planning + effects

#### New Components

1. **WorkflowEnv** - Environment for dependency injection
2. **Pure modules** - Extracted pure business logic
3. **Effect modules** - I/O wrapped in Effects

### Data Structures

#### StepResult

```rust
#[derive(Debug, Clone)]
pub struct StepResult {
    pub success: bool,
    pub output: Option<String>,
    pub captured_variables: HashMap<String, String>,
    pub duration: Duration,
}
```

#### StepError

```rust
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
}

impl StepError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, StepError::ClaudeRetryExhausted { .. })
    }
}
```

## Dependencies

### Prerequisites
- **Spec 108**: Functional Programming Adoption (patterns and practices)
- **Stillwater crate**: Already a dependency, provides Effect pattern

### Affected Components
- `src/cook/workflow/` - Major restructuring
- `src/cook/orchestrator/` - Uses new workflow effects
- Tests throughout - Updated for new architecture

### External Dependencies
- `stillwater` - Effect, RetryPolicy, ContextError (already in Cargo.toml)

## Testing Strategy

### Unit Tests

```rust
mod pure_tests {
    use super::pure::*;

    #[test]
    fn test_step_planning_sequential() {
        let workflow = create_test_workflow(3);
        let plan = plan_steps(&workflow);

        assert_eq!(plan.len(), 3);
        assert!(plan.iter().all(|s| s.order.is_sequential()));
    }

    #[test]
    fn test_interpolation() {
        let vars = hashmap! {
            "name" => "prodigy",
            "version" => "1.0",
        };

        let result = interpolate("Hello ${name} v${version}", &vars);
        assert_eq!(result, "Hello prodigy v1.0");
    }

    #[test]
    fn test_resume_planning_from_failed_step() {
        let checkpoint = Checkpoint {
            state: CheckpointState::Failed { step_index: 2, .. },
            ..
        };

        let plan = plan_resume(&checkpoint);

        // Should RETRY the failed step
        assert_eq!(plan.start_index, 2);
        assert!(plan.retry_failed);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_workflow_effect_composition() {
    let env = MockWorkflowEnvBuilder::new()
        .with_claude_success("output 1")
        .with_shell_success("output 2")
        .build();

    let workflow = vec![
        WorkflowStep::claude("/test-cmd"),
        WorkflowStep::shell("echo done"),
    ];

    let result = execute_workflow(workflow)
        .run(&env)
        .await
        .unwrap();

    assert_eq!(result.steps_completed, 2);
}

#[tokio::test]
async fn test_checkpoint_saved_on_failure() {
    let env = MockWorkflowEnvBuilder::new()
        .with_claude_failure("500 error", /* transient */ true)
        .with_retry_exhausted_after(3)
        .build();

    let result = execute_workflow(vec![WorkflowStep::claude("/test")])
        .run(&env)
        .await;

    assert!(result.is_err());

    // Checkpoint should have been saved with failed state
    let checkpoint = env.checkpoint_storage.latest().await.unwrap();
    assert!(matches!(checkpoint.state, CheckpointState::Failed { .. }));
}
```

### Performance Tests

```rust
#[tokio::test]
async fn test_effect_overhead_acceptable() {
    let workflow = generate_workflow_with_steps(100);

    // Measure Effect-based execution
    let start = Instant::now();
    execute_workflow(workflow.clone()).run(&env).await.unwrap();
    let effect_duration = start.elapsed();

    // Compare to direct execution (baseline)
    let baseline_duration = measure_baseline_execution(&workflow).await;

    // Effect overhead should be < 5%
    let overhead_pct = (effect_duration.as_secs_f64() / baseline_duration.as_secs_f64() - 1.0) * 100.0;
    assert!(overhead_pct < 5.0, "Effect overhead too high: {}%", overhead_pct);
}
```

## Documentation Requirements

### Code Documentation
- Document WorkflowEnv fields and usage
- Document Effect composition patterns
- Document testing with mock environments
- Document pure function contracts

### Architecture Updates
- Update ARCHITECTURE.md with Effect-based execution diagram
- Document pure core / imperative shell separation

## Implementation Notes

### Effect Composition Patterns

**Sequential composition** (and_then):
```rust
step1.and_then(|r1| step2.map(|r2| (r1, r2)))
```

**Parallel composition** (par_all):
```rust
Effect::par_all(vec![step1, step2, step3])
```

**Conditional composition**:
```rust
step1.and_then(|result| {
    if result.should_continue() {
        step2
    } else {
        Effect::pure(result)
    }
})
```

### Mock Environment Pattern

```rust
pub struct MockWorkflowEnvBuilder {
    claude_responses: VecDeque<Result<String, String>>,
    shell_responses: VecDeque<Result<ProcessOutput, String>>,
    checkpoint_storage: Arc<InMemoryCheckpointStorage>,
}

impl MockWorkflowEnvBuilder {
    pub fn with_claude_success(mut self, output: &str) -> Self {
        self.claude_responses.push_back(Ok(output.to_string()));
        self
    }

    pub fn with_claude_failure(mut self, error: &str, transient: bool) -> Self {
        let err = if transient {
            format!("500: {}", error)
        } else {
            format!("400: {}", error)
        };
        self.claude_responses.push_back(Err(err));
        self
    }

    pub fn build(self) -> MockWorkflowEnv {
        MockWorkflowEnv {
            claude_responses: Arc::new(Mutex::new(self.claude_responses)),
            shell_responses: Arc::new(Mutex::new(self.shell_responses)),
            checkpoint_storage: self.checkpoint_storage,
            // ... other fields
        }
    }
}
```

## Migration and Compatibility

### Breaking Changes
- Internal API changes only
- No changes to workflow YAML format
- No changes to CLI interface

### Migration Path
1. Introduce Effect-based execution alongside existing code
2. Add feature flag for Effect-based execution
3. Migrate step-by-step with tests
4. Remove old implementation once validated

### Compatibility
- Existing workflows continue to work unchanged
- Existing session state formats preserved
- Checkpoint format may change (versioned migration)

## Success Metrics

### Quantitative
- Test coverage > 80% for pure functions
- Effect overhead < 5% vs current implementation
- Zero regressions in existing functionality

### Qualitative
- Code is more testable (pure functions)
- Error messages have full context trail
- Retry behavior is consistent and configurable
