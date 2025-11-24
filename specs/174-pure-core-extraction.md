---
number: 174
title: Pure Core Extraction
category: foundation
priority: high
status: draft
dependencies: [172, 173]
created: 2025-11-24
---

# Specification 174: Pure Core Extraction

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 172 (Stillwater Foundation), Spec 173 (Parallel Execution Effects)

## Context

Prodigy's codebase currently suffers from significant mixing of pure business logic with I/O operations:

**Problem Areas:**
1. **Orchestrator God Object** - `src/cook/orchestrator/core.rs` at 2,884 LOC mixing decision logic with I/O
2. **Workflow Executor Complexity** - `src/cook/workflow/executor/commands.rs` at 2,243 LOC mixing command building with execution
3. **Session Management Mutations** - Mutation-heavy state updates making testing difficult
4. **Scattered Business Logic** - Domain logic spread across I/O-heavy modules

**Consequences:**
- **Difficult to test** - Mocking required for simple logic testing
- **Hard to understand** - Business logic obscured by I/O details
- **Brittle refactoring** - Changes ripple through tightly coupled code
- **Poor reusability** - Logic tied to specific I/O contexts

This specification covers Phase 3 of the Stillwater migration: extracting pure business logic into a testable core, leaving I/O in a thin imperative shell.

## Objective

Transform Prodigy's architecture to "pure core, imperative shell" by:
1. **Extracting pure planning logic** from orchestrator (2,884 LOC → ~500 LOC)
2. **Splitting workflow executor** into pure transformations + I/O effects (2,243 LOC → ~300 LOC + effects)
3. **Creating immutable session updates** with pure transformation functions
4. **Establishing module hierarchy** that enforces separation of concerns
5. **Achieving 100% test coverage** on pure functions without mocking

## Requirements

### Functional Requirements

#### FR1: Orchestrator Decomposition
- **MUST** extract execution planning into pure functions
- **MUST** reduce orchestrator core to < 500 LOC (I/O coordination only)
- **MUST** create pure modules for mode detection, resource allocation
- **MUST** separate planning from execution
- **MUST** maintain all existing orchestration functionality

#### FR2: Workflow Executor Pure/IO Split
- **MUST** extract command building as pure string transformation
- **MUST** extract variable expansion as pure function
- **MUST** extract output parsing as pure function
- **MUST** create effect modules for Claude, shell, handler execution
- **MUST** compose effects with pure transformations

#### FR3: Session Management Immutability
- **MUST** create pure `apply_session_update` functions
- **MUST** eliminate mutation in session state transitions
- **MUST** validate state transitions as pure logic
- **MUST** wrap with effect for I/O (load, save)
- **MUST** maintain session history and audit trail

#### FR4: Pure Module Hierarchy
- **MUST** create `src/core/orchestration/` for pure planning logic
- **MUST** create `src/cook/workflow/pure/` for pure transformations
- **MUST** create effect modules parallel to pure modules
- **MUST** enforce no I/O in pure modules (linting/architecture tests)
- **MUST** make pure modules reusable across contexts

#### FR5: Effect Orchestration Patterns
- **MUST** use `Effect::and_then` for sequential composition
- **MUST** use `Effect::map` for pure transformations
- **MUST** use `Effect::from_async_fn` for I/O boundaries
- **MUST** provide clear examples of composition patterns
- **MUST** maintain error context through effect chains

### Non-Functional Requirements

#### NFR1: Testability
- **MUST** achieve 100% test coverage on pure functions
- **MUST** eliminate mocking from pure logic tests
- **MUST** make tests fast (< 1ms per pure function test)
- **MUST** enable property-based testing on pure functions

#### NFR2: Code Quality
- **MUST** reduce orchestrator from 2,884 LOC to < 500 LOC
- **MUST** reduce workflow executor from 2,243 LOC to < 300 LOC (+ effects)
- **MUST** increase pure function LOC to > 1,000 LOC
- **MUST** pass clippy with no warnings

#### NFR3: Maintainability
- **MUST** make business logic changes require only pure module edits
- **MUST** make I/O changes require only effect module edits
- **MUST** enable parallel development on pure and effects
- **MUST** improve code readability and self-documentation

## Acceptance Criteria

- [ ] Orchestrator core reduced to < 500 LOC
- [ ] Pure execution planning module created and tested
- [ ] Mode detection, resource allocation extracted as pure functions
- [ ] Workflow executor split into pure + effects modules
- [ ] Command builder, variable expansion, output parser are pure
- [ ] Session updates use immutable transformations
- [ ] Pure module hierarchy established in `src/core/`
- [ ] Effect modules created parallel to pure modules
- [ ] 100% test coverage on pure functions
- [ ] Zero mocking required for pure function tests
- [ ] Property tests verify determinism and laws
- [ ] Integration tests verify effect composition
- [ ] All existing functionality preserved
- [ ] Performance benchmarks show no regression

## Technical Details

### Implementation Approach

#### 1. Orchestrator Decomposition

**Pure Execution Planning:**
```rust
// src/core/orchestration/execution_planning.rs

/// Pure: Execution plan with no I/O
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    pub mode: ExecutionMode,
    pub resource_needs: ResourceRequirements,
    pub phases: Vec<Phase>,
    pub parallel_budget: usize,
}

/// Pure: Plan execution from config
pub fn plan_execution(config: &CookConfig) -> ExecutionPlan {
    let mode = detect_execution_mode(config);
    let resource_needs = calculate_resources(config, &mode);
    let phases = determine_phases(config, &mode);
    let parallel_budget = compute_parallel_budget(&resource_needs);

    ExecutionPlan {
        mode,
        resource_needs,
        phases,
        parallel_budget,
    }
}

/// Pure: Detect execution mode
pub fn detect_execution_mode(config: &CookConfig) -> ExecutionMode {
    if config.command.dry_run {
        ExecutionMode::DryRun
    } else if config.mapreduce.is_some() {
        ExecutionMode::MapReduce
    } else if config.command.arguments.is_some() {
        ExecutionMode::Iterative
    } else {
        ExecutionMode::Standard
    }
}

/// Pure: Calculate resource requirements
pub fn calculate_resources(
    config: &CookConfig,
    mode: &ExecutionMode,
) -> ResourceRequirements {
    match mode {
        ExecutionMode::MapReduce => {
            let mr = config.mapreduce.as_ref().unwrap();
            ResourceRequirements {
                worktrees: mr.max_parallel + 1, // +1 for parent
                memory_estimate: estimate_memory(mr),
                disk_space: estimate_disk(mr),
                max_concurrent_commands: mr.max_parallel,
            }
        }
        ExecutionMode::Iterative => {
            let iterations = config.command.arguments.as_ref().unwrap().len();
            ResourceRequirements {
                worktrees: 1,
                memory_estimate: iterations * 50_000_000, // 50MB per iteration
                disk_space: 0,
                max_concurrent_commands: 1,
            }
        }
        _ => ResourceRequirements::minimal(),
    }
}

/// Pure: Determine execution phases
pub fn determine_phases(
    config: &CookConfig,
    mode: &ExecutionMode,
) -> Vec<Phase> {
    match mode {
        ExecutionMode::MapReduce => {
            let mr = config.mapreduce.as_ref().unwrap();
            vec![
                Phase::Setup(mr.setup.clone()),
                Phase::Map(mr.map.clone()),
                Phase::Reduce(mr.reduce.clone()),
            ]
        }
        ExecutionMode::Standard | ExecutionMode::Iterative => {
            vec![Phase::Commands(config.commands.clone())]
        }
        ExecutionMode::DryRun => {
            vec![Phase::DryRunAnalysis]
        }
    }
}
```

**Slim Orchestrator (I/O only):**
```rust
// src/cook/orchestrator/core.rs (reduced to ~500 LOC)

use stillwater::Effect;
use crate::core::orchestration::*;

impl CookOrchestrator {
    /// Run workflow (I/O coordination only)
    pub async fn run(&self, config: CookConfig) -> Result<ExecutionResult> {
        // Pure planning (no I/O)
        let plan = execution_planning::plan_execution(&config);

        // Effect composition (I/O)
        let effect = self.setup_environment(&plan)
            .and_then(|env| self.execute_plan(&plan, env))
            .and_then(|result| self.finalize_session(result))
            .context("Running workflow");

        // Execute at boundary
        effect.run_async(&self.dependencies).await
    }

    /// Setup environment effect
    fn setup_environment(
        &self,
        plan: &ExecutionPlan,
    ) -> Effect<ExecutionEnvironment, CookError, Dependencies> {
        Effect::from_async_fn(|deps| async move {
            let session = deps.session_manager.create_session().await?;
            let worktree = if plan.resource_needs.worktrees > 0 {
                Some(deps.worktree_manager.create_parent().await?)
            } else {
                None
            };

            Ok(ExecutionEnvironment {
                session,
                worktree,
                variables: HashMap::new(),
            })
        })
    }

    /// Execute plan effect
    fn execute_plan(
        &self,
        plan: &ExecutionPlan,
        env: ExecutionEnvironment,
    ) -> Effect<ExecutionResult, CookError, Dependencies> {
        match plan.mode {
            ExecutionMode::MapReduce => self.execute_mapreduce(plan, env),
            ExecutionMode::Standard => self.execute_standard(plan, env),
            ExecutionMode::Iterative => self.execute_iterative(plan, env),
            ExecutionMode::DryRun => self.execute_dry_run(plan, env),
        }
    }

    /// Finalize session effect
    fn finalize_session(
        &self,
        result: ExecutionResult,
    ) -> Effect<ExecutionResult, CookError, Dependencies> {
        Effect::from_async_fn(move |deps| async move {
            // Save final session state
            deps.session_manager
                .finalize_session(&result.session_id)
                .await?;

            // Cleanup worktrees if needed
            if let Some(worktree) = result.worktree {
                deps.worktree_manager.cleanup(&worktree).await?;
            }

            Ok(result)
        })
    }
}
```

#### 2. Workflow Executor Pure/IO Split

**Pure Command Building:**
```rust
// src/cook/workflow/pure/command_builder.rs

/// Pure: Build command string from template
pub fn build_command(
    template: &str,
    variables: &HashMap<String, String>,
) -> String {
    expand_variables(template, variables)
}

/// Pure: Expand variables in template
pub fn expand_variables(
    template: &str,
    variables: &HashMap<String, String>,
) -> String {
    let mut result = template.to_string();

    // Simple variable expansion: ${VAR}
    for (key, value) in variables {
        let placeholder = format!("${{{}}}", key);
        result = result.replace(&placeholder, value);
    }

    // Short variable expansion: $VAR
    for (key, value) in variables {
        let placeholder = format!("${}", key);
        // Use word boundaries to avoid partial matches
        if !key.is_empty() {
            result = result.replace(&placeholder, value);
        }
    }

    result
}

/// Pure: Extract variable references from template
pub fn extract_variable_references(template: &str) -> HashSet<String> {
    let mut refs = HashSet::new();

    // Match ${VAR} pattern
    let braced_regex = Regex::new(r"\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap();
    for cap in braced_regex.captures_iter(template) {
        refs.insert(cap[1].to_string());
    }

    // Match $VAR pattern
    let simple_regex = Regex::new(r"\$([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    for cap in simple_regex.captures_iter(template) {
        refs.insert(cap[1].to_string());
    }

    refs
}
```

**Pure Output Parsing:**
```rust
// src/cook/workflow/pure/output_parser.rs

/// Pure: Extract variables from command output
pub fn parse_output_variables(
    output: &str,
    patterns: &[OutputPattern],
) -> HashMap<String, String> {
    patterns
        .iter()
        .filter_map(|pattern| extract_match(output, pattern))
        .collect()
}

/// Pure: Extract single variable match
fn extract_match(output: &str, pattern: &OutputPattern) -> Option<(String, String)> {
    match pattern {
        OutputPattern::Regex { name, regex } => {
            regex.captures(output).and_then(|cap| {
                cap.get(1).map(|m| (name.clone(), m.as_str().to_string()))
            })
        }
        OutputPattern::Json { name, json_path } => {
            extract_json_path(output, json_path)
                .map(|value| (name.clone(), value))
        }
        OutputPattern::Line { name, line_number } => {
            output.lines().nth(*line_number)
                .map(|line| (name.clone(), line.to_string()))
        }
    }
}

/// Pure: Extract value from JSON path
fn extract_json_path(json_str: &str, path: &str) -> Option<String> {
    let value: Value = serde_json::from_str(json_str).ok()?;
    let pointer = JsonPointer::parse(path).ok()?;
    pointer.resolve(&value).ok()
        .map(|v| v.to_string())
}
```

**Effect Composition:**
```rust
// src/cook/workflow/effects/claude.rs

use stillwater::Effect;
use crate::cook::workflow::pure::*;

/// Effect: Execute Claude command
pub fn execute_claude_command(
    template: &str,
    variables: &HashMap<String, String>,
) -> Effect<CommandOutput, CommandError, WorkflowEnv> {
    let template = template.to_string();
    let variables = variables.clone();

    Effect::from_async_fn(move |env| async move {
        // Pure: Build command
        let command = command_builder::build_command(&template, &variables);

        // I/O: Execute
        let output = env.claude_runner.run(&command).await?;

        // Pure: Parse output
        let new_vars = output_parser::parse_output_variables(
            &output.stdout,
            &env.output_patterns,
        );

        Ok(CommandOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.exit_code,
            variables: new_vars,
        })
    })
}

// src/cook/workflow/effects/shell.rs

/// Effect: Execute shell command
pub fn execute_shell_command(
    template: &str,
    variables: &HashMap<String, String>,
) -> Effect<CommandOutput, CommandError, WorkflowEnv> {
    let template = template.to_string();
    let variables = variables.clone();

    Effect::from_async_fn(move |env| async move {
        // Pure: Build command
        let command = command_builder::build_command(&template, &variables);

        // I/O: Execute
        let output = env.shell_runner.run(&command).await?;

        // Pure: Parse output
        let new_vars = output_parser::parse_output_variables(
            &output.stdout,
            &env.output_patterns,
        );

        Ok(CommandOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.exit_code,
            variables: new_vars,
        })
    })
}
```

#### 3. Session Management Immutability

**Pure Session Updates:**
```rust
// src/core/session/updates.rs

/// Pure: Apply session update
pub fn apply_session_update(
    session: UnifiedSession,
    update: SessionUpdate,
) -> Result<UnifiedSession, SessionError> {
    let updated = UnifiedSession {
        updated_at: Utc::now(),
        ..session
    };

    match update {
        SessionUpdate::Status(status) => apply_status_update(updated, status),
        SessionUpdate::Progress(progress) => apply_progress_update(updated, progress),
        SessionUpdate::Variables(vars) => apply_variable_update(updated, vars),
        SessionUpdate::AddStep(step) => apply_add_step(updated, step),
    }
}

/// Pure: Apply status update with validation
fn apply_status_update(
    session: UnifiedSession,
    status: SessionStatus,
) -> Result<UnifiedSession, SessionError> {
    // Validate state transition
    validate_status_transition(&session.status, &status)?;

    Ok(UnifiedSession {
        status,
        ..session
    })
}

/// Pure: Validate status transition
fn validate_status_transition(
    from: &SessionStatus,
    to: &SessionStatus,
) -> Result<(), SessionError> {
    use SessionStatus::*;

    let valid = matches!(
        (from, to),
        (Created, Running)
            | (Running, Paused)
            | (Running, Completed)
            | (Running, Failed)
            | (Paused, Running)
            | (Paused, Cancelled)
    );

    if valid {
        Ok(())
    } else {
        Err(SessionError::InvalidTransition {
            from: from.clone(),
            to: to.clone(),
        })
    }
}

/// Pure: Apply progress update
fn apply_progress_update(
    session: UnifiedSession,
    progress: ProgressUpdate,
) -> Result<UnifiedSession, SessionError> {
    let mut new_progress = session.progress.clone();

    new_progress.completed_steps += progress.completed_steps;
    new_progress.failed_steps += progress.failed_steps;
    new_progress.current_step = progress.current_step;

    Ok(UnifiedSession {
        progress: new_progress,
        ..session
    })
}

/// Pure: Apply variable update (merge)
fn apply_variable_update(
    session: UnifiedSession,
    new_vars: HashMap<String, Value>,
) -> Result<UnifiedSession, SessionError> {
    let mut variables = session.variables.clone();
    variables.extend(new_vars);

    Ok(UnifiedSession {
        variables,
        ..session
    })
}
```

**Effect Wrapper:**
```rust
// src/unified_session/effects.rs

use stillwater::Effect;
use crate::core::session::updates::*;

/// Effect: Update session with I/O
pub fn update_session_effect(
    id: SessionId,
    update: SessionUpdate,
) -> Effect<UnifiedSession, SessionError, SessionEnv> {
    Effect::from_async_fn(move |env| async move {
        // I/O: Load session
        let session = env.storage.load_session(&id).await?;

        // Pure: Apply update
        let updated = apply_session_update(session, update)?;

        // I/O: Save session
        env.storage.save_session(&updated).await?;

        Ok(updated)
    })
}

/// Effect: Batch update session (multiple updates)
pub fn batch_update_session_effect(
    id: SessionId,
    updates: Vec<SessionUpdate>,
) -> Effect<UnifiedSession, SessionError, SessionEnv> {
    Effect::from_async_fn(move |env| async move {
        // I/O: Load once
        let mut session = env.storage.load_session(&id).await?;

        // Pure: Apply all updates
        for update in updates {
            session = apply_session_update(session, update)?;
        }

        // I/O: Save once
        env.storage.save_session(&session).await?;

        Ok(session)
    })
}
```

### Architecture Changes

**New Module Structure:**
```
src/
├── core/                          # Pure core logic (no I/O)
│   ├── orchestration/
│   │   ├── execution_planning.rs # Pure execution planning
│   │   ├── resource_allocation.rs # Pure resource calculation
│   │   └── mode_detection.rs     # Pure mode classification
│   ├── session/
│   │   ├── updates.rs            # Pure session transformations
│   │   └── validation.rs         # Pure state transition validation
│   └── workflow/
│       └── validation.rs         # Pure workflow validation
├── cook/
│   ├── orchestrator/
│   │   ├── core.rs               # Slim I/O coordinator (~500 LOC)
│   │   └── effects/              # Orchestrator effects
│   └── workflow/
│       ├── pure/                 # Pure transformations
│       │   ├── command_builder.rs
│       │   ├── variable_expansion.rs
│       │   └── output_parser.rs
│       └── effects/              # I/O effects
│           ├── claude.rs
│           ├── shell.rs
│           └── handler.rs
└── unified_session/
    ├── manager.rs                # Session coordination
    └── effects.rs                # Session I/O effects
```

### Data Structures

**Execution Plan:**
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    pub mode: ExecutionMode,
    pub resource_needs: ResourceRequirements,
    pub phases: Vec<Phase>,
    pub parallel_budget: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRequirements {
    pub worktrees: usize,
    pub memory_estimate: usize,
    pub disk_space: usize,
    pub max_concurrent_commands: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionMode {
    Standard,
    MapReduce,
    Iterative,
    DryRun,
}
```

**Session Update:**
```rust
#[derive(Debug, Clone)]
pub enum SessionUpdate {
    Status(SessionStatus),
    Progress(ProgressUpdate),
    Variables(HashMap<String, Value>),
    AddStep(StepRecord),
}
```

### APIs and Interfaces

**Pure APIs (no I/O):**
```rust
// Execution planning
pub fn plan_execution(config: &CookConfig) -> ExecutionPlan;
pub fn detect_execution_mode(config: &CookConfig) -> ExecutionMode;
pub fn calculate_resources(config: &CookConfig, mode: &ExecutionMode) -> ResourceRequirements;

// Command building
pub fn build_command(template: &str, variables: &HashMap<String, String>) -> String;
pub fn expand_variables(template: &str, variables: &HashMap<String, String>) -> String;
pub fn extract_variable_references(template: &str) -> HashSet<String>;

// Output parsing
pub fn parse_output_variables(output: &str, patterns: &[OutputPattern]) -> HashMap<String, String>;

// Session updates
pub fn apply_session_update(session: UnifiedSession, update: SessionUpdate) -> Result<UnifiedSession, SessionError>;
pub fn validate_status_transition(from: &SessionStatus, to: &SessionStatus) -> Result<(), SessionError>;
```

**Effect APIs (I/O):**
```rust
// Workflow effects
pub fn execute_claude_command(template: &str, variables: &HashMap<String, String>)
    -> Effect<CommandOutput, CommandError, WorkflowEnv>;
pub fn execute_shell_command(template: &str, variables: &HashMap<String, String>)
    -> Effect<CommandOutput, CommandError, WorkflowEnv>;

// Session effects
pub fn update_session_effect(id: SessionId, update: SessionUpdate)
    -> Effect<UnifiedSession, SessionError, SessionEnv>;
pub fn batch_update_session_effect(id: SessionId, updates: Vec<SessionUpdate>)
    -> Effect<UnifiedSession, SessionError, SessionEnv>;
```

## Dependencies

### Prerequisites
- **Spec 172** completed (Stillwater foundation)
- **Spec 173** completed (Parallel execution effects)
- Stillwater Effect and composition patterns available

### Affected Components
- `src/cook/orchestrator/core.rs` - Major refactor (2,884 → ~500 LOC)
- `src/cook/workflow/executor/commands.rs` - Split into pure + effects (2,243 → ~300 + effects)
- `src/unified_session/manager.rs` - Extract pure updates
- All orchestration and workflow tests

### External Dependencies
- `stillwater = "0.2.0"` (Effect composition)
- `regex = "*"` (for variable expansion)

## Testing Strategy

### Unit Tests (Pure Functions - No Mocking!)

**Execution Planning:**
```rust
#[test]
fn test_plan_execution_mapreduce() {
    let config = CookConfig {
        mapreduce: Some(MapReduceConfig {
            max_parallel: 10,
            // ...
        }),
        // ...
    };

    let plan = plan_execution(&config);

    assert_eq!(plan.mode, ExecutionMode::MapReduce);
    assert_eq!(plan.parallel_budget, 10);
    assert_eq!(plan.resource_needs.worktrees, 11); // 10 + parent

    // Pure function - no I/O, no mocks needed!
}

#[test]
fn test_detect_execution_mode() {
    let dry_run_config = CookConfig { command: Command { dry_run: true, .. }, .. };
    assert_eq!(detect_execution_mode(&dry_run_config), ExecutionMode::DryRun);

    let mapreduce_config = CookConfig { mapreduce: Some(..), .. };
    assert_eq!(detect_execution_mode(&mapreduce_config), ExecutionMode::MapReduce);
}
```

**Command Building:**
```rust
#[test]
fn test_build_command() {
    let template = "echo ${name} ${value}";
    let vars = [
        ("name".into(), "test".into()),
        ("value".into(), "123".into()),
    ].iter().cloned().collect();

    let result = build_command(template, &vars);

    assert_eq!(result, "echo test 123");
    // Pure function - deterministic, easy to test!
}

#[test]
fn test_expand_variables_handles_missing() {
    let template = "echo ${exists} ${missing}";
    let vars = [("exists".into(), "value".into())].iter().cloned().collect();

    let result = expand_variables(template, &vars);

    assert_eq!(result, "echo value ${missing}");
    // Missing variables preserved
}
```

**Session Updates:**
```rust
#[test]
fn test_apply_status_update_validates_transition() {
    let session = UnifiedSession {
        status: SessionStatus::Running,
        // ...
    };

    // Valid transition
    let result = apply_session_update(
        session.clone(),
        SessionUpdate::Status(SessionStatus::Completed),
    );
    assert!(result.is_ok());

    // Invalid transition
    let result = apply_session_update(
        session,
        SessionUpdate::Status(SessionStatus::Created),
    );
    assert!(result.is_err());
}
```

### Property Tests

**Determinism:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_planning_is_deterministic(
        max_parallel in 1usize..100,
        dry_run: bool,
    ) {
        let config = create_config(max_parallel, dry_run);

        let plan1 = plan_execution(&config);
        let plan2 = plan_execution(&config);

        // Pure function - same input, same output
        prop_assert_eq!(plan1, plan2);
    }

    #[test]
    fn prop_variable_expansion_idempotent(
        template in ".*",
        vars in prop::collection::hash_map(".*", ".*", 0..10),
    ) {
        let result1 = expand_variables(&template, &vars);
        let result2 = expand_variables(&result1, &vars);

        // Should be idempotent after first expansion
        prop_assert_eq!(result1, result2);
    }
}
```

### Effect Tests with Mock Environment

```rust
#[tokio::test]
async fn test_workflow_execution_effect() {
    let mock_env = MockWorkflowEnv {
        claude_runner: MockClaudeRunner::with_output("/test", "success"),
        output_patterns: vec![],
    };

    let effect = execute_claude_command("/test", &HashMap::new());
    let result = effect.run_async(&mock_env).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().stdout, "success");
}
```

### Integration Tests

**End-to-End Orchestration:**
```rust
#[tokio::test]
async fn test_orchestrator_with_pure_planning() {
    let config = load_test_config("standard_workflow.yml");

    // Pure planning
    let plan = plan_execution(&config);
    assert_eq!(plan.mode, ExecutionMode::Standard);

    // Effect execution
    let orchestrator = create_test_orchestrator();
    let result = orchestrator.run(config).await;

    assert!(result.is_ok());
}
```

## Documentation Requirements

### Code Documentation

**Comprehensive examples:**
- Document pure/imperative separation pattern
- Show effect composition examples
- Provide testing examples for pure functions
- Explain immutable session update pattern

### User Documentation

**Update CLAUDE.md:**
- Add section on pure core / imperative shell
- Document pure module organization
- Explain testability benefits
- Provide migration guide for contributors

### Architecture Updates

**Major ARCHITECTURE.md update:**
- New section: "Pure Core, Imperative Shell Architecture"
- Component hierarchy diagram
- Data flow through pure → effects
- Module organization principles

## Implementation Notes

### Critical Success Factors
1. **Complete separation** - No I/O in pure modules
2. **100% test coverage** - All pure functions tested
3. **No mocking needed** - Pure tests are simple
4. **Dramatic LOC reduction** - Orchestrator < 500 LOC

### Migration Path
1. Create pure module hierarchy
2. Extract planning functions
3. Extract command building/parsing
4. Extract session updates
5. Create effect wrappers
6. Refactor orchestrator to use pure planning
7. Refactor workflow executor to use pure transformations
8. Update all tests
9. Document patterns

## Migration and Compatibility

### Breaking Changes
- **None** - Internal refactoring only
- Public APIs preserved
- Workflow files unchanged

### Backward Compatibility
- All workflows work without modification
- Session storage format unchanged
- Checkpoint format preserved

### Rollback Strategy
If critical issues arise:
1. Revert module reorganization
2. Restore original orchestrator
3. Restore original workflow executor
4. Redeploy previous version

**Rollback impact:** Lose testability improvements, return to mixed concerns.
