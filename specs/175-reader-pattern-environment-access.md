---
number: 175
title: Reader Pattern Environment Access
category: foundation
priority: medium
status: draft
dependencies: [172, 173, 174]
created: 2025-11-24
---

# Specification 175: Reader Pattern Environment Access

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 172 (Stillwater Foundation), Spec 173 (Parallel Execution), Spec 174 (Pure Core)

## Context

Prodigy's current codebase passes configuration and environment dependencies manually through function parameters, leading to:

**Current Problems:**
- **Manual environment threading** - Config, managers, storage passed explicitly everywhere
- **Parameter proliferation** - Functions with 5+ parameters for environment access
- **Difficult testing** - Must construct full environment for every test
- **Tight coupling** - Functions depend on specific environment structure
- **No local overrides** - Cannot temporarily change config for specific operations

**Example of current complexity:**
```rust
async fn execute_agent(
    item: &Value,
    config: &MapConfig,
    worktree_manager: &WorktreeManager,
    executor: &CommandExecutor,
    storage: &Storage,
    dlq: &DLQ,
    metrics: &Metrics,
) -> Result<AgentResult> {
    // 7+ parameters just for environment access!
}
```

## Objective

Implement Reader pattern for clean environment/config access by:
1. **Using Effect::asks** for environment extraction
2. **Eliminating manual parameter threading** throughout codebase
3. **Enabling local config overrides** with Effect::local
4. **Simplifying function signatures** through environment context
5. **Improving testability** with minimal mock environments

## Requirements

### Functional Requirements

#### FR1: Environment Type Definitions
- **MUST** define environment types for each execution context
- **MUST** include all required dependencies in environment
- **MUST** make environments cheaply cloneable (Arc for expensive resources)
- **MUST** support environment composition for nested contexts
- **MUST** maintain backward compatibility with existing code

#### FR2: Effect::asks for Environment Access
- **MUST** use `Effect::asks` for extracting environment values
- **MUST** create helper functions for common environment access patterns
- **MUST** eliminate manual environment parameter threading
- **MUST** preserve type safety for environment access
- **MUST** provide clear error messages for missing environment values

#### FR3: Effect::local for Temporary Overrides
- **MUST** use `Effect::local` for temporary config changes
- **MUST** support timeout overrides for long-running operations
- **MUST** support debug/verbose mode overrides for specific phases
- **MUST** support retry policy overrides for risky operations
- **MUST** ensure local changes don't leak to parent context

#### FR4: Environment Composition
- **MUST** support nested environments (e.g., MapEnv contains PhaseEnv)
- **MUST** enable environment extension for specialized contexts
- **MUST** maintain clear ownership and lifetime semantics
- **MUST** avoid unnecessary cloning of expensive resources
- **MUST** use Arc for shared resources

#### FR5: Testing Support
- **MUST** enable minimal mock environments for testing
- **MUST** support partial environment mocking
- **MUST** make environment construction simple
- **MUST** provide test helpers for common environment patterns
- **MUST** eliminate need for full environment in unit tests

### Non-Functional Requirements

#### NFR1: Performance
- **MUST** have zero runtime overhead (compile-time abstraction)
- **MUST** avoid unnecessary environment cloning
- **MUST** use Arc for expensive resources (managers, storage)
- **MUST** maintain or improve current performance

#### NFR2: Ergonomics
- **MUST** reduce average function parameters by > 50%
- **MUST** make environment access intuitive and clear
- **MUST** provide good IDE autocomplete support
- **MUST** minimize boilerplate for common patterns

#### NFR3: Maintainability
- **MUST** make adding environment fields non-breaking
- **MUST** enable easy environment extension
- **MUST** keep environment types well-documented
- **MUST** follow existing Prodigy conventions

## Acceptance Criteria

- [ ] Environment types defined for all execution contexts
- [ ] Helper functions created for common environment access (get_config, get_storage, etc.)
- [ ] Manual parameter threading eliminated in map/setup/reduce phases
- [ ] Function signatures reduced (average < 3 parameters)
- [ ] Effect::local used for timeout overrides
- [ ] Effect::local used for debug mode overrides
- [ ] Integration tests verify local changes don't leak
- [ ] Mock environment helpers created for testing
- [ ] Unit tests simplified (no full environment construction)
- [ ] Performance benchmarks show no regression
- [ ] Documentation includes Reader pattern examples

## Technical Details

### Implementation Approach

#### 1. Environment Type Definitions

```rust
// src/cook/execution/environment.rs

/// MapReduce execution environment
#[derive(Clone)]
pub struct MapEnv {
    // Configuration
    pub config: MapConfig,

    // Shared resources (Arc for cheap cloning)
    pub worktree_manager: Arc<WorktreeManager>,
    pub executor: Arc<CommandExecutor>,
    pub storage: Arc<Storage>,
    pub dlq: Arc<DLQ>,
    pub metrics: Arc<Metrics>,

    // Phase-specific
    pub agent_template: Vec<Command>,
    pub session_id: SessionId,
}

/// Phase (setup/reduce) execution environment
#[derive(Clone)]
pub struct PhaseEnv {
    pub config: PhaseConfig,
    pub executor: Arc<CommandExecutor>,
    pub storage: Arc<Storage>,
    pub variables: Arc<RwLock<HashMap<String, Value>>>,
    pub session_id: SessionId,
}

/// Workflow execution environment
#[derive(Clone)]
pub struct WorkflowEnv {
    pub config: WorkflowConfig,
    pub claude_runner: Arc<ClaudeRunner>,
    pub shell_runner: Arc<ShellRunner>,
    pub handler_runner: Arc<HandlerRunner>,
    pub output_patterns: Vec<OutputPattern>,
    pub session_id: SessionId,
}
```

#### 2. Environment Access Helpers

```rust
// src/cook/execution/mapreduce/environment_helpers.rs

use stillwater::Effect;

/// Get max parallel agents from environment
pub fn get_max_parallel() -> Effect<usize, (), MapEnv> {
    Effect::asks(|env: &MapEnv| env.config.max_parallel)
}

/// Get worktree manager from environment
pub fn get_worktree_manager() -> Effect<Arc<WorktreeManager>, (), MapEnv> {
    Effect::asks(|env: &MapEnv| env.worktree_manager.clone())
}

/// Get storage from environment
pub fn get_storage() -> Effect<Arc<Storage>, (), MapEnv> {
    Effect::asks(|env: &MapEnv| env.storage.clone())
}

/// Get entire config from environment
pub fn get_config() -> Effect<MapConfig, (), MapEnv> {
    Effect::asks(|env: &MapEnv| env.config.clone())
}

/// Get session ID from environment
pub fn get_session_id() -> Effect<SessionId, (), MapEnv> {
    Effect::asks(|env: &MapEnv| env.session_id.clone())
}
```

#### 3. Using Effect::asks

**Before (manual threading):**
```rust
async fn execute_agent(
    item: &Value,
    config: &MapConfig,
    worktree_manager: &WorktreeManager,
    executor: &CommandExecutor,
) -> Result<AgentResult> {
    let worktree = worktree_manager.create_worktree(&format!("agent-{}", item.id)).await?;
    let result = executor.execute_commands(&worktree, item).await?;
    worktree_manager.merge_to_parent(&worktree).await?;
    Ok(result)
}
```

**After (Reader pattern):**
```rust
fn execute_agent(item: Value) -> Effect<AgentResult, AgentError, MapEnv> {
    get_worktree_manager()
        .and_then(|wt_mgr| {
            let worktree_name = format!("agent-{}", item.id);
            create_worktree_effect(&worktree_name)
                .map(move |wt| (wt_mgr, wt))
        })
        .and_then(|(wt_mgr, worktree)| {
            execute_commands_effect(&item, &worktree)
                .map(move |result| (wt_mgr, worktree, result))
        })
        .and_then(|(wt_mgr, worktree, result)| {
            merge_to_parent_effect(&worktree)
                .map(move |_| result)
        })
}

// Clean composition without manual parameter passing!
```

#### 4. Effect::local for Temporary Overrides

**Timeout Override:**
```rust
// src/cook/execution/mapreduce/phases/setup.rs

/// Execute setup with extended timeout
fn execute_setup_with_long_timeout(
    commands: Vec<Command>,
) -> Effect<PhaseResult, PhaseError, PhaseEnv> {
    Effect::local(
        |env: &PhaseEnv| PhaseEnv {
            config: PhaseConfig {
                timeout: Duration::from_secs(600), // 10 minutes
                ..env.config
            },
            ..env.clone()
        },
        execute_setup_commands(commands),
    )
}

/// Execute command with retries (override retry policy)
fn execute_with_retries(
    command: Command,
) -> Effect<CommandResult, CommandError, PhaseEnv> {
    Effect::local(
        |env: &PhaseEnv| PhaseEnv {
            config: PhaseConfig {
                max_retries: 5,
                retry_delay: Duration::from_secs(2),
                ..env.config
            },
            ..env.clone()
        },
        execute_command_effect(&command),
    )
}
```

**Debug Mode Override:**
```rust
// src/cook/execution/mapreduce/phases/map.rs

/// Execute map phase with debug logging
fn execute_map_with_debug(
    items: Vec<Value>,
) -> Effect<Vec<AgentResult>, PhaseError, MapEnv> {
    Effect::local(
        |env: &MapEnv| MapEnv {
            config: MapConfig {
                debug: true,
                verbose: true,
                log_level: LogLevel::Debug,
                ..env.config
            },
            ..env.clone()
        },
        execute_map_phase(items),
    )
}

/// Execute single agent with extra logging (nested local)
fn execute_agent_verbose(
    item: Value,
) -> Effect<AgentResult, AgentError, MapEnv> {
    get_config()
        .and_then(|config| {
            if config.debug {
                info!("Executing agent for item: {:?}", item);
            }

            // Temporarily increase verbosity for this agent only
            Effect::local(
                |env: &MapEnv| MapEnv {
                    config: MapConfig {
                        log_level: LogLevel::Trace,
                        ..env.config
                    },
                    ..env.clone()
                },
                execute_agent(item),
            )
        })
}
```

#### 5. Environment Composition Example

**Nested contexts:**
```rust
// src/cook/orchestrator/effects.rs

use stillwater::Effect;

/// Create execution environment from orchestrator dependencies
fn create_execution_env(
    deps: &Dependencies,
    config: &CookConfig,
) -> ExecutionEnv {
    ExecutionEnv {
        // Workflow environment
        workflow_env: WorkflowEnv {
            config: config.workflow.clone(),
            claude_runner: deps.claude_runner.clone(),
            shell_runner: deps.shell_runner.clone(),
            handler_runner: deps.handler_runner.clone(),
            output_patterns: config.output_patterns.clone(),
            session_id: deps.session_id.clone(),
        },

        // MapReduce environment (contains phase env)
        mapreduce_env: config.mapreduce.as_ref().map(|mr_config| MapEnv {
            config: mr_config.clone(),
            worktree_manager: deps.worktree_manager.clone(),
            executor: deps.executor.clone(),
            storage: deps.storage.clone(),
            dlq: deps.dlq.clone(),
            metrics: deps.metrics.clone(),
            agent_template: mr_config.agent_template.clone(),
            session_id: deps.session_id.clone(),
        }),
    }
}

/// Switch environment context for different execution modes
fn execute_with_mode(
    plan: &ExecutionPlan,
    exec_env: ExecutionEnv,
) -> Effect<ExecutionResult, CookError, ExecutionEnv> {
    match plan.mode {
        ExecutionMode::MapReduce => {
            // Extract MapReduce environment
            Effect::asks(|env: &ExecutionEnv| env.mapreduce_env.clone())
                .and_then(|mr_env| {
                    // Execute in MapReduce environment context
                    execute_mapreduce_phase()
                        .run_async(&mr_env.unwrap())
                        .map(Effect::pure)
                })
        }
        ExecutionMode::Standard => {
            // Extract Workflow environment
            Effect::asks(|env: &ExecutionEnv| env.workflow_env.clone())
                .and_then(|wf_env| {
                    execute_standard_workflow()
                        .run_async(&wf_env)
                        .map(Effect::pure)
                })
        }
        _ => Effect::pure(ExecutionResult::default()),
    }
}
```

### Architecture Changes

**New Modules:**
```
src/cook/execution/
├── environment.rs                # Environment type definitions
├── environment_helpers.rs        # Reader pattern helpers
└── mock_environment.rs           # Test environment builders
```

**Modified Modules:**
```
src/cook/execution/mapreduce/phases/
├── map.rs                        # Use Reader pattern
├── setup.rs                      # Use Reader pattern
└── reduce.rs                     # Use Reader pattern

src/cook/orchestrator/
└── effects.rs                    # Environment construction
```

### APIs and Interfaces

**Environment Access API:**
```rust
// Get single values
pub fn get_max_parallel() -> Effect<usize, (), MapEnv>;
pub fn get_worktree_manager() -> Effect<Arc<WorktreeManager>, (), MapEnv>;
pub fn get_storage() -> Effect<Arc<Storage>, (), MapEnv>;
pub fn get_config() -> Effect<MapConfig, (), MapEnv>;

// Compose multiple values
pub fn get_execution_context() -> Effect<(MapConfig, Arc<Storage>, SessionId), (), MapEnv> {
    get_config()
        .and_then(|config| {
            get_storage().map(move |storage| (config, storage))
        })
        .and_then(|(config, storage)| {
            get_session_id().map(move |session_id| (config, storage, session_id))
        })
}
```

**Local Override API:**
```rust
// Timeout overrides
pub fn with_timeout<A, E>(
    duration: Duration,
    effect: Effect<A, E, PhaseEnv>,
) -> Effect<A, E, PhaseEnv>;

// Debug mode overrides
pub fn with_debug<A, E>(
    effect: Effect<A, E, MapEnv>,
) -> Effect<A, E, MapEnv>;

// Retry policy overrides
pub fn with_retries<A, E>(
    max_retries: usize,
    effect: Effect<A, E, PhaseEnv>,
) -> Effect<A, E, PhaseEnv>;
```

**Mock Environment API:**
```rust
// Test helpers
pub fn mock_map_env() -> MapEnv;
pub fn mock_phase_env() -> PhaseEnv;
pub fn mock_workflow_env() -> WorkflowEnv;

// Builder pattern for test environments
pub struct MapEnvBuilder {
    config: MapConfig,
    worktree_manager: Option<Arc<MockWorktreeManager>>,
    // ...
}

impl MapEnvBuilder {
    pub fn with_config(mut self, config: MapConfig) -> Self { ... }
    pub fn with_mock_worktree_manager(mut self, mgr: MockWorktreeManager) -> Self { ... }
    pub fn build(self) -> MapEnv { ... }
}
```

## Dependencies

### Prerequisites
- **Spec 172** completed (Stillwater foundation)
- **Spec 173** completed (Parallel execution effects)
- **Spec 174** completed (Pure core extraction)
- Stillwater Reader pattern available (Effect::asks, Effect::local)

### Affected Components
- All MapReduce phase implementations
- All workflow execution code
- Session management
- Orchestrator coordination
- All integration tests

### External Dependencies
- `stillwater = "0.2.0"` (Reader pattern)

## Testing Strategy

### Unit Tests

**Environment Access:**
```rust
#[test]
fn test_get_max_parallel() {
    let env = MapEnv {
        config: MapConfig { max_parallel: 10, .. },
        ..mock_map_env()
    };

    let effect = get_max_parallel();
    let result = effect.run(&env).unwrap();

    assert_eq!(result, 10);
}
```

**Local Overrides:**
```rust
#[tokio::test]
async fn test_local_timeout_override() {
    let env = PhaseEnv {
        config: PhaseConfig {
            timeout: Duration::from_secs(30),
            ..Default::default()
        },
        ..mock_phase_env()
    };

    let effect = Effect::local(
        |env: &PhaseEnv| PhaseEnv {
            config: PhaseConfig {
                timeout: Duration::from_secs(600),
                ..env.config
            },
            ..env.clone()
        },
        get_config().map(|c| c.timeout),
    );

    let timeout = effect.run_async(&env).await.unwrap();
    assert_eq!(timeout, Duration::from_secs(600));

    // Original environment unchanged
    assert_eq!(env.config.timeout, Duration::from_secs(30));
}
```

**Local Changes Don't Leak:**
```rust
#[tokio::test]
async fn test_local_changes_dont_leak() {
    let env = MapEnv {
        config: MapConfig { debug: false, .. },
        ..mock_map_env()
    };

    // Execute with debug enabled locally
    let inner_effect = Effect::local(
        |env: &MapEnv| MapEnv {
            config: MapConfig { debug: true, ..env.config },
            ..env.clone()
        },
        get_config().map(|c| c.debug),
    );

    let inner_debug = inner_effect.run_async(&env).await.unwrap();
    assert!(inner_debug); // Debug enabled inside local

    // Check environment unchanged
    let outer_debug = get_config()
        .map(|c| c.debug)
        .run_async(&env)
        .await
        .unwrap();
    assert!(!outer_debug); // Debug still disabled outside
}
```

### Integration Tests

**Reduced Parameter Signatures:**
```rust
#[tokio::test]
async fn test_execute_agent_simplified_signature() {
    let env = MapEnvBuilder::new()
        .with_mock_worktree_manager()
        .with_mock_executor()
        .build();

    let item = json!({"id": 1, "data": "test"});

    // Simple signature - environment implicit!
    let effect = execute_agent(item);
    let result = effect.run_async(&env).await;

    assert!(result.is_ok());
}
```

**Nested Local Contexts:**
```rust
#[tokio::test]
async fn test_nested_local_contexts() {
    let env = MapEnv {
        config: MapConfig {
            debug: false,
            timeout: Duration::from_secs(30),
            ..Default::default()
        },
        ..mock_map_env()
    };

    let effect = Effect::local(
        |env: &MapEnv| MapEnv {
            config: MapConfig { debug: true, ..env.config },
            ..env.clone()
        },
        // Nested local - change timeout within debug context
        Effect::local(
            |env: &MapEnv| MapEnv {
                config: MapConfig {
                    timeout: Duration::from_secs(300),
                    ..env.config
                },
                ..env.clone()
            },
            get_config(),
        ),
    );

    let config = effect.run_async(&env).await.unwrap();

    // Both changes applied
    assert!(config.debug);
    assert_eq!(config.timeout, Duration::from_secs(300));
}
```

### Performance Tests

**Zero-cost abstraction verification:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_environment_access(c: &mut Criterion) {
    let env = mock_map_env();

    c.bench_function("direct_access", |b| {
        b.iter(|| {
            let _val = black_box(&env.config.max_parallel);
        })
    });

    c.bench_function("reader_pattern", |b| {
        b.iter(|| {
            let effect = get_max_parallel();
            let _val = black_box(effect.run(&env).unwrap());
        })
    });

    // Should have identical performance
}
```

## Documentation Requirements

### Code Documentation

**Reader pattern guide:**
```rust
/// Executes an agent using the Reader pattern for environment access.
///
/// Instead of passing configuration and dependencies explicitly, this function
/// uses `Effect::asks` to extract them from the environment when executed.
///
/// # Environment Requirements
///
/// - `MapEnv::worktree_manager`: For creating isolated worktrees
/// - `MapEnv::executor`: For running commands
/// - `MapEnv::config.max_parallel`: For concurrency limiting
///
/// # Example
///
/// ```rust
/// let env = MapEnv { /* ... */ };
/// let item = json!({"id": 1, "data": "test"});
///
/// let effect = execute_agent(item);
/// let result = effect.run_async(&env).await?;
/// ```
///
/// # Local Overrides
///
/// You can temporarily override environment values:
///
/// ```rust
/// let effect = Effect::local(
///     |env: &MapEnv| MapEnv {
///         config: MapConfig { debug: true, ..env.config },
///         ..env.clone()
///     },
///     execute_agent(item),
/// );
/// ```
fn execute_agent(item: Value) -> Effect<AgentResult, AgentError, MapEnv>
```

### User Documentation

**Update CLAUDE.md:**
- Add "Reader Pattern" section
- Document Effect::asks and Effect::local
- Provide common usage patterns
- Show testing with mock environments

### Architecture Updates

**Update ARCHITECTURE.md:**
- Add "Environment Management with Reader Pattern" section
- Document environment type hierarchy
- Show environment composition patterns
- Explain local override use cases

## Implementation Notes

### Critical Success Factors
1. **Clean API** - Intuitive helper functions
2. **Performance** - Zero runtime overhead
3. **Simplified signatures** - Fewer parameters
4. **Easy testing** - Minimal mock environments

### Gotchas and Pitfalls
- **Arc for expensive resources** - Avoid cloning entire managers
- **Local changes scope** - Ensure they don't leak
- **Environment construction** - Keep simple and cheap
- **Type inference** - May need explicit types with `Effect::asks`

### Best Practices
- Use helper functions (get_config, get_storage) instead of raw asks
- Create test environment builders for common patterns
- Document environment requirements in function docs
- Use Effect::local sparingly (only when truly needed)
- Keep environment types well-organized

## Migration and Compatibility

### Breaking Changes
- **None** - Gradual migration possible
- Can mix manual threading with Reader pattern during transition

### Backward Compatibility
- All existing code continues to work
- Reader pattern opt-in by default
- Can refactor incrementally

### Migration Steps
1. Define environment types
2. Create helper functions for environment access
3. Refactor one module at a time to use Reader pattern
4. Update tests to use mock environments
5. Remove manual parameter threading
6. Measure performance impact
7. Update documentation

### Rollback Strategy
If issues arise:
1. Revert to manual parameter threading
2. Remove environment types
3. Restore original function signatures

**Rollback impact:** Lose ergonomic improvements, return to parameter proliferation.
