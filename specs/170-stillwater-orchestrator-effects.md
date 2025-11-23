---
number: 170
title: Stillwater Effect-Based Orchestrator
category: foundation
priority: medium
status: draft
dependencies: [168, 169]
created: 2025-11-23
---

# Specification 170: Stillwater Effect-Based Orchestrator

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: 168 (Error Context), 169 (Pure State)

## Context

The `DefaultCookOrchestrator` in `src/cook/orchestrator/core.rs` (2,884 lines) suffers from:

- **Mixed Concerns**: Execution, session management, health metrics, argument processing all in one struct
- **Mutable State**: Heavy use of `Arc<Mutex<>>` and `&mut self` patterns (182 instances codebase-wide)
- **Testability**: Requires full database, git repository, and file system setup for testing
- **Complexity**: 12 interdependent fields with complex initialization order

This makes the orchestrator difficult to test, maintain, and reason about.

## Objective

Refactor the orchestrator to use Stillwater's Effect pattern with environment-based dependency injection, separating pure business logic from I/O operations and enabling mockless testing.

## Requirements

### Functional Requirements

1. **Environment-Based Dependencies**
   - Create `OrchestratorEnv` struct with all dependencies
   - Pass environment explicitly to all operations
   - Enable mock environments for testing

2. **Pure Orchestration Logic**
   - Extract workflow classification to pure functions
   - Extract workflow validation to pure functions
   - Separate decision logic from execution

3. **Effect Composition**
   - Wrap all I/O operations in `Effect<T, E, Env>`
   - Compose workflow phases as effect chains
   - Enable lazy evaluation until `.run(env)`

4. **Immutable State**
   - Remove `&mut self` patterns
   - State updates return new state
   - No hidden mutations in orchestrator

### Non-Functional Requirements

1. **Testability**: 80% of orchestration logic testable without I/O
2. **Performance**: No degradation vs current implementation
3. **Maintainability**: Clear module boundaries and responsibilities
4. **Backward Compatibility**: Existing CLI and API unchanged

## Acceptance Criteria

- [ ] `OrchestratorEnv` struct created with all dependencies
- [ ] Pure orchestration module created (`orchestrator/pure.rs`)
- [ ] Effect-based orchestration module created (`orchestrator/effects.rs`)
- [ ] Workflow classification extracted to pure functions
- [ ] Workflow validation uses `Validation<T, E>` (Spec 167)
- [ ] Setup phase wrapped in Effects
- [ ] Execution phase wrapped in Effects
- [ ] Merge phase wrapped in Effects
- [ ] All I/O accessed via environment
- [ ] `Arc<Mutex<>>` usage reduced by 80%
- [ ] Mock environment tests for all phases
- [ ] 50-100 pure function tests (no I/O)
- [ ] Performance benchmarks show <5% overhead
- [ ] Documentation updated with architecture
- [ ] Migration guide for downstream code

## Technical Details

### Implementation Approach

**Phase 1: Environment Definition**
```rust
// src/cook/orchestrator/environment.rs

/// Environment providing all orchestrator dependencies
#[derive(Clone)]
pub struct OrchestratorEnv {
    pub session_manager: Arc<dyn SessionManager>,
    pub command_executor: Arc<dyn CommandExecutor>,
    pub claude_executor: Arc<dyn ClaudeExecutor>,
    pub user_interaction: Arc<dyn UserInteraction>,
    pub git_operations: Arc<dyn GitOperations>,
    pub subprocess_manager: SubprocessManager,
}

impl OrchestratorEnv {
    /// Create production environment
    pub fn production(/* dependencies */) -> Self {
        Self {
            session_manager: Arc::new(DefaultSessionManager::new(storage)),
            command_executor: Arc::new(DefaultCommandExecutor::new()),
            claude_executor: Arc::new(DefaultClaudeExecutor::new()),
            user_interaction: Arc::new(TerminalUserInteraction::new()),
            git_operations: Arc::new(GitCli::new()),
            subprocess_manager: SubprocessManager::new(),
        }
    }

    /// Create test environment with mocks
    pub fn test() -> Self {
        Self {
            session_manager: Arc::new(MockSessionManager::new()),
            command_executor: Arc::new(MockCommandExecutor::new()),
            claude_executor: Arc::new(MockClaudeExecutor::new()),
            user_interaction: Arc::new(MockUserInteraction::new()),
            git_operations: Arc::new(MockGitOperations::new()),
            subprocess_manager: SubprocessManager::test(),
        }
    }
}
```

**Phase 2: Pure Orchestration Logic**
```rust
// src/cook/orchestrator/pure.rs

/// Pure orchestration functions (no I/O, no side effects)

use crate::config::WorkflowConfig;
use stillwater::Validation;

#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowType {
    MapReduce,
    Standard,
    Empty,
}

/// Classify workflow type (pure)
pub fn classify_workflow(config: &WorkflowConfig) -> WorkflowType {
    match config.mode {
        WorkflowMode::MapReduce => WorkflowType::MapReduce,
        WorkflowMode::Standard if config.steps.is_empty() => WorkflowType::Empty,
        WorkflowMode::Standard => WorkflowType::Standard,
    }
}

/// Validate workflow configuration (pure)
pub fn validate_workflow(
    config: &WorkflowConfig
) -> Validation<(), Vec<WorkflowError>> {
    Validation::all((
        validate_workflow_steps(config),
        validate_environment_vars(config),
        validate_command_syntax(config),
        validate_merge_workflow(config),
    ))
}

/// Validate workflow steps (pure helper)
fn validate_workflow_steps(
    config: &WorkflowConfig
) -> Validation<(), Vec<WorkflowError>> {
    if config.steps.is_empty() && config.mode == WorkflowMode::Standard {
        Validation::failure(vec![WorkflowError::NoSteps])
    } else {
        Validation::success(())
    }
}

/// Determine if iteration should continue (pure)
pub fn should_continue_iteration(
    iteration: u32,
    max_iterations: u32,
    files_changed: usize,
) -> IterationDecision {
    if iteration >= max_iterations {
        IterationDecision::Stop(format!("Reached max iterations: {}", max_iterations))
    } else if files_changed == 0 {
        IterationDecision::Stop("No files changed".to_string())
    } else {
        IterationDecision::Continue
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IterationDecision {
    Stop(String),
    Continue,
    AskUser,
}
```

**Phase 3: Effect-Based Orchestration**
```rust
// src/cook/orchestrator/effects.rs

use stillwater::Effect;
use super::{environment::OrchestratorEnv, pure};

type OrchEffect<T> = Effect<T, OrchestratorError, OrchestratorEnv>;

/// Workflow session state
#[derive(Debug, Clone)]
pub struct WorkflowSession {
    pub session_id: String,
    pub worktree_path: PathBuf,
    pub config: WorkflowConfig,
}

/// Setup workflow environment (Effect)
pub fn setup_workflow(config: WorkflowConfig) -> OrchEffect<WorkflowSession> {
    // Pure validation first
    Effect::from_validation(pure::validate_workflow(&config))
        .and_then(|_| {
            // I/O: Create session
            Effect::from_async(|env: &OrchestratorEnv| async move {
                let session = env.session_manager
                    .create_session(&config)
                    .await?;

                Ok(session)
            })
        })
        .and_then(|session| {
            // I/O: Create worktree
            Effect::from_async(|env: &OrchestratorEnv| async move {
                let worktree = env.git_operations
                    .create_worktree(&session.id)
                    .await?;

                Ok(WorkflowSession {
                    session_id: session.id,
                    worktree_path: worktree,
                    config: session.config,
                })
            })
        })
        .context("Setting up workflow environment")
}

/// Execute single workflow step (Effect)
pub fn execute_step(
    step: WorkflowStep,
    context: StepContext,
) -> OrchEffect<StepResult> {
    // Pure: Determine command type
    let cmd_type = pure::classify_command(&step);

    // I/O: Execute based on type
    Effect::from_async(move |env: &OrchestratorEnv| async move {
        match cmd_type {
            CommandType::Shell => {
                env.command_executor.execute_shell(&step, &context).await
            }
            CommandType::Claude => {
                env.claude_executor.execute(&step, &context).await
            }
            CommandType::Test => {
                env.command_executor.execute_test(&step, &context).await
            }
        }
    })
    .with_context(|| format!("Executing step: {}", step.name))
}

/// Execute all workflow steps (Effect composition)
pub fn execute_steps(
    session: WorkflowSession,
) -> OrchEffect<WorkflowResult> {
    let steps = session.config.steps.clone();

    // Fold over steps, chaining effects
    steps.into_iter().enumerate().fold(
        Effect::pure(Vec::new()),
        |acc_effect, (idx, step)| {
            acc_effect.and_then(move |mut results| {
                let context = StepContext {
                    session_id: session.session_id.clone(),
                    step_index: idx,
                    worktree: session.worktree_path.clone(),
                    results: results.clone(),
                };

                execute_step(step, context)
                    .map(move |result| {
                        results.push(result);
                        results
                    })
            })
        },
    )
    .map(|results| WorkflowResult {
        session_id: session.session_id.clone(),
        step_results: results,
    })
    .context("Executing workflow steps")
}

/// Complete workflow (full pipeline)
pub fn execute_workflow(config: WorkflowConfig) -> OrchEffect<WorkflowResult> {
    setup_workflow(config)
        .and_then(execute_steps)
        .and_then(save_results)
        .and_then(merge_changes)
        .context("Executing workflow")
}
```

**Phase 4: Thin Orchestrator Wrapper**
```rust
// src/cook/orchestrator/mod.rs

use effects::execute_workflow;

/// Orchestrator (thin wrapper around effects)
pub struct DefaultCookOrchestrator {
    env: Arc<OrchestratorEnv>,
}

impl DefaultCookOrchestrator {
    pub fn new(env: Arc<OrchestratorEnv>) -> Self {
        Self { env }
    }

    /// Execute workflow (runs effect)
    pub async fn run_workflow(
        &self,
        config: WorkflowConfig,
    ) -> Result<WorkflowResult, OrchestratorError> {
        execute_workflow(config)
            .run(&self.env)
            .await
    }
}
```

### Architecture Changes

**New Module Structure**:
```
src/cook/orchestrator/
├── mod.rs              (public API, thin wrapper)
├── environment.rs      (NEW - dependency injection)
├── pure.rs             (NEW - pure orchestration logic)
├── effects.rs          (NEW - Effect-based execution)
├── core.rs             (DEPRECATED - old implementation)
└── construction.rs     (updated - env-based factory)
```

**Dependency Flow**:
```
CLI Entry Point
    ↓
DefaultCookOrchestrator (thin wrapper)
    ↓
execute_workflow (Effect composition)
    ↓
Pure Functions + Environment (dependency injection)
```

### Data Structures

```rust
/// Workflow session (immutable)
#[derive(Debug, Clone)]
pub struct WorkflowSession {
    pub session_id: String,
    pub worktree_path: PathBuf,
    pub config: WorkflowConfig,
}

/// Step execution context (immutable)
#[derive(Debug, Clone)]
pub struct StepContext {
    pub session_id: String,
    pub step_index: usize,
    pub worktree: PathBuf,
    pub results: Vec<StepResult>,
}

/// Workflow execution result (immutable)
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    pub session_id: String,
    pub step_results: Vec<StepResult>,
}
```

### APIs and Interfaces

**Public API** (unchanged):
```rust
impl CookOrchestrator for DefaultCookOrchestrator {
    async fn run_workflow(&self, config: WorkflowConfig) -> Result<WorkflowResult>;
}
```

**Internal Pure API** (new, testable):
```rust
pub mod pure {
    pub fn classify_workflow(config: &WorkflowConfig) -> WorkflowType;
    pub fn validate_workflow(config: &WorkflowConfig) -> Validation<(), Vec<WorkflowError>>;
    pub fn should_continue_iteration(iteration: u32, max: u32, changes: usize) -> IterationDecision;
}
```

**Internal Effect API** (new, composable):
```rust
pub mod effects {
    pub fn setup_workflow(config: WorkflowConfig) -> OrchEffect<WorkflowSession>;
    pub fn execute_steps(session: WorkflowSession) -> OrchEffect<WorkflowResult>;
    pub fn execute_workflow(config: WorkflowConfig) -> OrchEffect<WorkflowResult>;
}
```

## Dependencies

### Prerequisites
- Spec 168: Error context preservation (ContextError)
- Spec 169: Pure state transitions (Effect pattern understanding)
- Stillwater library with Effect and Validation types

### Affected Components
- `src/cook/orchestrator/core.rs` - Entire orchestrator implementation
- `src/cli/run.rs` - CLI entry point (minor changes)
- All orchestrator tests

### External Dependencies
- `stillwater = "0.1"` (Effect, Validation types)

## Testing Strategy

### Unit Tests (Pure Functions - No I/O)

```rust
#[cfg(test)]
mod pure_tests {
    use super::*;

    #[test]
    fn test_classify_workflow() {
        let mapreduce_config = WorkflowConfig {
            mode: WorkflowMode::MapReduce,
            ..Default::default()
        };

        assert_eq!(
            pure::classify_workflow(&mapreduce_config),
            WorkflowType::MapReduce
        );

        let empty_config = WorkflowConfig {
            mode: WorkflowMode::Standard,
            steps: vec![],
        };

        assert_eq!(
            pure::classify_workflow(&empty_config),
            WorkflowType::Empty
        );
    }

    #[test]
    fn test_validate_workflow_success() {
        let valid_config = create_valid_workflow_config();

        let result = pure::validate_workflow(&valid_config);

        assert!(matches!(result, Validation::Success(_)));
    }

    #[test]
    fn test_validate_workflow_accumulates_errors() {
        let invalid_config = WorkflowConfig {
            mode: WorkflowMode::Standard,
            steps: vec![],  // Invalid: no steps
            env: Some(invalid_env_vars()),  // Invalid: malformed vars
        };

        let result = pure::validate_workflow(&invalid_config);

        match result {
            Validation::Failure(errors) => {
                assert!(errors.len() >= 2);  // Multiple errors
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_should_continue_iteration() {
        assert_eq!(
            pure::should_continue_iteration(5, 10, 3),
            IterationDecision::Continue
        );

        assert!(matches!(
            pure::should_continue_iteration(10, 10, 3),
            IterationDecision::Stop(_)
        ));

        assert!(matches!(
            pure::should_continue_iteration(2, 10, 0),
            IterationDecision::Stop(_)
        ));
    }
}
```

### Integration Tests (Effects with Mock Environment)

```rust
#[cfg(test)]
mod effect_tests {
    use super::*;

    #[tokio::test]
    async fn test_setup_workflow() {
        let env = Arc::new(OrchestratorEnv::test());
        let config = test_workflow_config();

        let result = effects::setup_workflow(config)
            .run(&env)
            .await;

        assert!(result.is_ok());
        let session = result.unwrap();
        assert!(!session.session_id.is_empty());
    }

    #[tokio::test]
    async fn test_execute_single_step() {
        let env = Arc::new(OrchestratorEnv::test());
        let step = test_shell_step();
        let context = test_step_context();

        let result = effects::execute_step(step, context)
            .run(&env)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_workflow_end_to_end() {
        let env = Arc::new(OrchestratorEnv::test());
        let config = test_workflow_config();

        let result = effects::execute_workflow(config)
            .run(&env)
            .await;

        assert!(result.is_ok());
        let workflow_result = result.unwrap();
        assert!(!workflow_result.step_results.is_empty());
    }

    #[tokio::test]
    async fn test_workflow_validation_failure() {
        let env = Arc::new(OrchestratorEnv::test());
        let invalid_config = WorkflowConfig {
            mode: WorkflowMode::Standard,
            steps: vec![],  // Invalid
        };

        let result = effects::execute_workflow(invalid_config)
            .run(&env)
            .await;

        assert!(result.is_err());
        // Verify error includes validation context
    }
}
```

### Performance Benchmarks

```rust
#[tokio::test]
async fn benchmark_orchestrator_overhead() {
    let env = Arc::new(OrchestratorEnv::test());
    let config = large_workflow_config(100); // 100 steps

    let start = Instant::now();

    let _ = effects::execute_workflow(config)
        .run(&env)
        .await;

    let duration = start.elapsed();

    // Effect composition should add <5% overhead
    assert!(duration < baseline_duration * 1.05);
}
```

## Documentation Requirements

### Architecture Updates

Add to `ARCHITECTURE.md`:
```markdown
## Orchestrator Architecture

### Effect-Based Orchestration

The orchestrator uses Stillwater's Effect pattern for composable, testable workflows:

**Pure Core** (`orchestrator/pure.rs`):
- Workflow classification and validation
- Iteration decision logic
- No I/O, completely testable

**Effect Composition** (`orchestrator/effects.rs`):
- Setup, execution, merge as composable effects
- Lazy evaluation until `.run(env)`
- Automatic error context preservation

**Environment Injection** (`orchestrator/environment.rs`):
- All dependencies injected via OrchestratorEnv
- Mock environments for testing
- No hidden dependencies or singletons

### Testing Strategy

- **Pure functions**: Test without any I/O setup (80% of logic)
- **Effects**: Test with mock environments (fast, deterministic)
- **Integration**: Test with real dependencies (minimal)
```

## Implementation Notes

### Migration Strategy

**Phase 1: Parallel Implementation** (Weeks 1-2)
- Create new modules (environment, pure, effects)
- Keep old orchestrator running
- Add comprehensive tests for new implementation

**Phase 2: Gradual Migration** (Weeks 3-4)
- Update CLI to use new orchestrator (feature flag)
- Run both implementations in parallel
- Validate identical behavior

**Phase 3: Cleanup** (Week 5)
- Remove old orchestrator (core.rs)
- Clean up deprecated code
- Update all documentation

### Edge Cases

- **Empty workflows**: Handled by validation
- **Session creation failures**: Captured in effect context
- **Concurrent workflow execution**: Environment is Clone, thread-safe

### Performance Considerations

- **Effect boxing**: Single allocation per chain
- **Environment cloning**: Arc-based, cheap
- **State immutability**: No hidden mutations, clearer reasoning

## Migration and Compatibility

### Breaking Changes
None - public API unchanged.

### Internal API Changes
- Orchestrator construction requires `OrchestratorEnv`
- Internal methods take environment parameter
- State updates return new state (immutable)

### Migration Path

```rust
// Old
let orchestrator = DefaultCookOrchestrator::new(
    session_manager,
    command_executor,
    claude_executor,
    user_interaction,
    git_operations,
    subprocess,
);

// New
let env = Arc::new(OrchestratorEnv {
    session_manager,
    command_executor,
    claude_executor,
    user_interaction,
    git_operations,
    subprocess_manager: subprocess,
});
let orchestrator = DefaultCookOrchestrator::new(env);
```

### Rollback Strategy

- Keep old orchestrator during migration (feature flag)
- Can switch back if issues arise
- Gradual migration reduces risk
