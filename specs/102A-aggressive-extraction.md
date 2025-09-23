---
number: 102A
title: Aggressive Executor Extraction - Phase 1
category: foundation
priority: critical
status: draft
dependencies: [101]
created: 2025-09-23
---

# Specification 102A: Aggressive Executor Extraction - Phase 1

## Context

The `workflow/executor.rs` file is 5,426 lines and violates core principles. Since we're in early prototyping, we can aggressively refactor without maintaining backward compatibility. This allows for a much cleaner and faster decomposition.

## Objective

Aggressively extract all pure functions and closely related functionality into focused modules, breaking the monolith into manageable pieces. Update tests as needed.

## Requirements

### Functional Requirements
- Extract ALL validation logic to `workflow/validation.rs`
- Extract ALL interpolation to `workflow/interpolation.rs`
- Extract ALL step building to `workflow/step_builder.rs`
- Extract ALL progress tracking to `workflow/progress_tracker.rs`
- Extract ALL timing logic to `workflow/timing.rs`
- Extract ALL git operations to `workflow/git_operations.rs`
- Move error recovery to `workflow/recovery.rs`
- Update imports and fix compilation errors
- Rewrite tests to work with new structure

### Non-Functional Requirements
- Each module under 500 lines
- No concern for backward compatibility
- Direct extraction without wrappers
- Clean, focused modules

## Acceptance Criteria

- [ ] `executor.rs` reduced to under 2,000 lines
- [ ] Created 7+ focused modules
- [ ] All compilation errors fixed
- [ ] Tests updated and passing
- [ ] No duplicate code between modules

## Technical Details

### Extraction Strategy

#### Step 1: Rip Out Validation (~400 lines)
```rust
// workflow/validation.rs
pub fn validate_workflow(workflow: &ExtendedWorkflowConfig) -> Result<()> { }
pub fn validate_step(step: &WorkflowStep, context: &Context) -> Result<()> { }
pub fn check_commit_requirement(before: &str, after: &str, required: bool) -> Result<()> { }
pub fn evaluate_when_clause(clause: &str, context: &Context) -> Result<bool> { }
// Move ALL validation from executor.rs
```

#### Step 2: Rip Out Interpolation (~500 lines)
```rust
// workflow/interpolation.rs
pub struct Interpolator {
    strict_mode: bool,
    context: InterpolationContext,
}

impl Interpolator {
    pub fn interpolate(&self, template: &str) -> String { }
    pub fn interpolate_with_tracking(&self, template: &str) -> (String, Vec<Resolution>) { }
    pub fn build_context(env: &HashMap<String, String>, captured: &HashMap<String, String>) -> InterpolationContext { }
}
// Move ALL interpolation logic including regex patterns
```

#### Step 3: Rip Out Step Building (~300 lines)
```rust
// workflow/step_builder.rs
pub fn build_step(normalized: &NormalizedStep) -> WorkflowStep { }
pub fn get_step_display_name(step: &WorkflowStep) -> String { }
pub fn determine_command_type(command: &str) -> CommandType { }
// Move ALL step construction logic
```

#### Step 4: Rip Out Progress Tracking (~400 lines)
```rust
// workflow/progress_tracker.rs
pub struct ProgressTracker {
    total_steps: usize,
    completed: Vec<CompletedStep>,
}

impl ProgressTracker {
    pub fn track_step(&mut self, step: CompletedStep) { }
    pub fn should_skip(&self, index: usize) -> bool { }
    pub fn report_progress(&self, current: usize, total: usize) { }
}
// Move ALL progress logic
```

#### Step 5: Rip Out Timing (~200 lines)
```rust
// workflow/timing.rs
pub struct TimingTracker {
    workflow_start: Instant,
    step_timings: HashMap<String, Duration>,
}

impl TimingTracker {
    pub fn start_step(&mut self, name: String) { }
    pub fn end_step(&mut self, name: &str) -> Duration { }
    pub fn get_stats(&self) -> TimingStats { }
}
// Move ALL timing logic
```

#### Step 6: Rip Out Git Operations (~300 lines)
```rust
// workflow/git_operations.rs
pub async fn get_current_head(dir: &Path) -> Result<String> { }
pub async fn verify_commit(before: &str, after: &str) -> Result<bool> { }
pub async fn track_changes(tracker: &mut GitChangeTracker, changes: Vec<Change>) { }
// Move ALL git-related operations
```

#### Step 7: Rip Out Error Recovery (~600 lines)
```rust
// workflow/recovery.rs
pub async fn handle_step_failure(
    step: &WorkflowStep,
    error: Error,
    context: &mut Context,
) -> Result<StepResult> { }

pub async fn execute_on_failure_handler(
    handler: &OnFailureConfig,
    context: &mut Context,
) -> Result<()> { }
// Move ALL error recovery logic
```

### Update Executor Structure

```rust
// workflow/executor.rs (AFTER - ~1,500 lines)
use super::{validation, interpolation, step_builder, progress_tracker, timing, git_operations, recovery};

pub struct WorkflowExecutor {
    interpolator: interpolation::Interpolator,
    progress: progress_tracker::ProgressTracker,
    timing: timing::TimingTracker,
    session_manager: Arc<SessionManager>,
    command_registry: Arc<CommandRegistry>,
}

impl WorkflowExecutor {
    pub async fn execute(&mut self, workflow: &ExtendedWorkflowConfig, env: &ExecutionEnvironment) -> Result<()> {
        // Validate
        validation::validate_workflow(workflow)?;

        // Initialize tracking
        self.progress.reset();
        self.timing.start_workflow();

        // Execute steps
        for (index, step) in workflow.steps.iter().enumerate() {
            if self.progress.should_skip(index) {
                continue;
            }

            let result = self.execute_step(step, env).await?;
            self.progress.track_step(CompletedStep::from(index, &result));
        }

        Ok(())
    }

    async fn execute_step(&mut self, step: &WorkflowStep, env: &ExecutionEnvironment) -> Result<StepResult> {
        // Validate step
        validation::validate_step(step, &self.build_context())?;

        // Execute based on type
        let result = match &step.command_type {
            CommandType::Claude(cmd) => self.execute_claude(cmd, env).await,
            CommandType::Shell(cmd) => self.execute_shell(cmd, env).await,
            // ... other types
        }?;

        // Handle failure if needed
        if !result.success && !step.allow_failure {
            return recovery::handle_step_failure(step, result.into(), &mut self.build_context()).await;
        }

        Ok(result)
    }
}
```

### Test Updates

```rust
// workflow/executor_tests.rs
// BEFORE:
#[test]
fn test_validation() {
    let executor = WorkflowExecutor::new(...);
    executor.validate_workflow_config(&workflow).unwrap();
}

// AFTER:
#[test]
fn test_validation() {
    validation::validate_workflow(&workflow).unwrap();
}
```

## Implementation Steps

1. **Create all new module files** with basic structure
2. **Move validation logic** - Cut/paste all validation functions
3. **Move interpolation logic** - Cut/paste interpolation code
4. **Move other components** - Continue extracting
5. **Fix compilation errors** - Update imports, fix references
6. **Update tests** - Rewrite to use new modules directly
7. **Delete dead code** - Remove any unused code

## Testing Strategy

- Update existing tests to call extracted modules directly
- No need for compatibility tests
- Add module-specific tests where missing
- Run full test suite after each extraction

## Benefits of This Approach

- **Faster** - No compatibility layers or wrappers
- **Cleaner** - Direct module structure without legacy code
- **Simpler** - Each module has single responsibility
- **Testable** - Can test each module in isolation
- **Maintainable** - Clear boundaries between concerns

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking tests | Fix tests as we go, they need updating anyway |
| Missing functionality | Use compiler errors to find all usage sites |
| Merge conflicts | Complete quickly in focused effort |

## Success Metrics

- `executor.rs` under 2,000 lines
- All modules under 500 lines
- Tests passing (after updates)
- Clean module boundaries
- No circular dependencies