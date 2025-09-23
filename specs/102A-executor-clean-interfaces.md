---
number: 102A
title: Executor Clean Interfaces - Phase 1
category: foundation
priority: critical
status: draft
dependencies: [101]
created: 2025-09-23
---

# Specification 102A: Executor Clean Interfaces - Phase 1

## Context

This is Phase 1 of decomposing the monolithic `workflow/executor.rs` file (5,426 lines). This phase focuses on establishing clean interfaces and traits without breaking existing functionality. We'll use the Facade pattern to maintain backward compatibility while preparing for future extraction.

Current problems:
- No clear separation between execution, state management, and progress tracking
- Direct coupling between components makes testing difficult
- Functions directly access WorkflowExecutor's internal state

## Objective

Define clear trait boundaries and interfaces that will enable gradual decomposition of the executor while maintaining 100% backward compatibility.

## Requirements

### Functional Requirements
- Define trait interfaces for core executor responsibilities
- Extend existing `workflow/traits.rs` with new abstractions
- Create type aliases and wrapper types for better abstraction
- Maintain all existing public APIs unchanged
- All existing tests must pass without modification

### Non-Functional Requirements
- Zero performance regression
- No breaking changes to any public or crate-public APIs
- Traits should be composable and testable
- Clear documentation for each trait's responsibility

## Acceptance Criteria

- [ ] Extended `workflow/traits.rs` with new trait definitions
- [ ] Created `workflow/types.rs` for shared type definitions
- [ ] WorkflowExecutor implements all new traits
- [ ] All existing tests pass without modification
- [ ] No performance regression (benchmark comparison)
- [ ] Documentation for each trait explains its purpose

## Technical Details

### New Trait Definitions

```rust
// workflow/traits.rs (extend existing file)

/// Manages execution state and completed steps
pub trait ExecutionStateManager: Send + Sync {
    /// Track a completed step
    fn track_completion(&mut self, step: CompletedStep);

    /// Check if a step should be skipped (already completed)
    fn should_skip_step(&self, step_index: usize) -> bool;

    /// Get list of completed steps
    fn get_completed_steps(&self) -> &[CompletedStep];

    /// Clear completed steps
    fn clear_completed_steps(&mut self);
}

/// Reports progress during workflow execution
pub trait ExecutionProgressReporter: Send + Sync {
    /// Report that a step is starting
    fn report_step_start(&self, step_index: usize, total: usize, description: &str);

    /// Report that a step completed
    fn report_step_complete(&self, result: &StepResult, duration: Duration);

    /// Report iteration progress
    fn report_iteration_progress(&self, current: u32, max: u32);

    /// Report workflow phase
    fn report_phase(&self, phase: ExecutionPhase);
}

/// Handles variable interpolation and resolution
pub trait VariableResolver: Send + Sync {
    /// Interpolate variables in a template string
    fn interpolate(&self, template: &str, context: &VariableContext) -> String;

    /// Interpolate with tracking of resolved variables
    fn interpolate_with_tracking(&self, template: &str, context: &VariableContext)
        -> (String, Vec<VariableResolution>);

    /// Strict interpolation that fails on undefined variables
    fn interpolate_strict(&self, template: &str, context: &VariableContext)
        -> Result<String, String>;
}

/// Validates workflow steps and configurations
pub trait StepValidator: Send + Sync {
    /// Validate a workflow step before execution
    fn validate_step(&self, step: &WorkflowStep, context: &ValidationContext) -> Result<()>;

    /// Validate entire workflow configuration
    fn validate_workflow(&self, workflow: &ExtendedWorkflowConfig) -> Result<()>;

    /// Check step requirements (e.g., required commands available)
    fn check_requirements(&self, step: &WorkflowStep) -> Result<()>;
}

/// Manages error recovery and retry logic
pub trait ErrorRecoveryManager: Send + Sync {
    /// Determine recovery action for an error
    fn determine_recovery_action(&self, error: &Error, step: &WorkflowStep) -> RecoveryAction;

    /// Execute recovery action
    async fn execute_recovery(&mut self, action: RecoveryAction, context: &mut ExecutionContext)
        -> Result<()>;

    /// Check if retry is available
    fn can_retry(&self, step_index: usize) -> bool;
}
```

### Type Definitions

```rust
// workflow/types.rs (new file)

use std::time::Duration;
use chrono::{DateTime, Utc};

/// Result of executing a workflow step
#[derive(Debug, Clone)]
pub struct StepResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration: Duration,
}

/// Information about a completed step
#[derive(Debug, Clone)]
pub struct CompletedStep {
    pub step_index: usize,
    pub command: String,
    pub result: StepResult,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

/// Variable resolution information
#[derive(Debug, Clone)]
pub struct VariableResolution {
    pub name: String,
    pub value: String,
    pub source: VariableSource,
}

/// Source of a variable value
#[derive(Debug, Clone)]
pub enum VariableSource {
    Environment,
    Context,
    Iteration,
    CapturedOutput,
    Default,
}

/// Execution phase for progress tracking
#[derive(Debug, Clone)]
pub enum ExecutionPhase {
    Initialization,
    Validation,
    Execution,
    Recovery,
    Completion,
}

/// Context for variable resolution
#[derive(Debug, Clone)]
pub struct VariableContext {
    pub environment: HashMap<String, String>,
    pub iteration: HashMap<String, String>,
    pub captured: HashMap<String, String>,
    pub git: HashMap<String, String>,
}

/// Context for step validation
#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub available_commands: HashSet<String>,
    pub working_directory: PathBuf,
    pub dry_run: bool,
}

/// Action to take for error recovery
#[derive(Debug, Clone)]
pub enum RecoveryAction {
    Retry { delay: Duration },
    Skip,
    RunHandler { handler: HandlerStep },
    Abort,
}
```

### Implementation Approach

1. **Add traits to existing WorkflowExecutor**:
```rust
impl ExecutionStateManager for WorkflowExecutor {
    fn track_completion(&mut self, step: CompletedStep) {
        self.completed_steps.push(step);
    }

    fn should_skip_step(&self, step_index: usize) -> bool {
        self.completed_steps.iter().any(|s| s.step_index == step_index)
    }

    // ... other methods
}
```

2. **Keep existing methods as facades**:
```rust
impl WorkflowExecutor {
    // Existing public method remains unchanged
    pub async fn execute(&mut self, workflow: &ExtendedWorkflowConfig, env: &ExecutionEnvironment) -> Result<()> {
        // Internally can start using trait methods
        self.validate_workflow(workflow)?;  // Uses StepValidator trait
        // ... rest of implementation
    }
}
```

## Implementation Steps

1. Create `workflow/types.rs` with shared type definitions
2. Extend `workflow/traits.rs` with new trait definitions
3. Implement traits for WorkflowExecutor (in executor.rs)
4. Update internal methods to use trait bounds where possible
5. Add trait-based unit tests alongside existing tests

## Testing Strategy

- No changes to existing tests required
- Add new trait-specific tests in `workflow/traits_test.rs`
- Create mock implementations for testing
- Benchmark before and after to ensure no regression

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Performance overhead from trait dispatch | Use static dispatch where possible, benchmark critical paths |
| Confusion from having both old and new patterns | Clear documentation, gradual migration |
| Breaking changes to internal APIs | Keep all changes additive, use default implementations |

## Success Metrics

- All existing tests pass
- Zero performance regression
- Traits are used in at least 3 places in the codebase
- Can create mock implementations for testing

## Documentation Requirements

- Document each trait's responsibility and contract
- Provide examples of mock implementations
- Update development guide with trait patterns