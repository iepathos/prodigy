# Functional Core, Imperative Shell Pattern

## Overview

The Functional Core, Imperative Shell pattern is a software architecture approach that separates business logic (the functional core) from side effects and I/O operations (the imperative shell). This pattern is fundamental to Prodigy's architecture and provides significant benefits for testing, maintainability, and code quality.

## Core Principles

### Functional Core
- **Pure Functions**: Functions that always return the same output for the same input
- **No Side Effects**: No modification of external state, no I/O operations
- **Immutable Data**: Data is transformed, not mutated
- **Composable**: Small functions that combine to create complex behavior
- **Testable**: Easy to test with simple input/output assertions

### Imperative Shell
- **Handles I/O**: All file system, network, and database operations
- **Manages Side Effects**: Logging, metrics, external system calls
- **Thin Orchestration Layer**: Minimal logic, delegates to core
- **Integration Point**: Where the pure world meets the real world

## Implementation in Prodigy

### Directory Structure

```
src/
├── core/                 # Functional Core - Pure business logic
│   ├── config/          # Configuration transformation and validation
│   ├── session/         # Session state calculations
│   ├── workflow/        # Workflow parsing and validation
│   ├── mapreduce/       # MapReduce distribution logic
│   └── validation/      # Data validation rules
│
├── storage/             # Imperative Shell - I/O operations
├── worktree/           # Imperative Shell - Git operations
├── cook/execution/     # Imperative Shell - Command execution
└── cli/                # Imperative Shell - User interface
```

## Practical Examples

### Example 1: Session State Management

#### Functional Core (src/core/session/)
```rust
// Pure function: Calculate next session state
pub fn calculate_next_state(
    current: SessionState,
    event: SessionEvent,
) -> Result<SessionState, StateError> {
    match (current.status, event) {
        (Status::Running, SessionEvent::Complete) => {
            Ok(SessionState {
                status: Status::Completed,
                completed_at: Some(event.timestamp),
                ..current
            })
        }
        (Status::Running, SessionEvent::Fail(error)) => {
            Ok(SessionState {
                status: Status::Failed,
                error: Some(error),
                completed_at: Some(event.timestamp),
                ..current
            })
        }
        _ => Err(StateError::InvalidTransition)
    }
}

// Pure function: Validate state transition
pub fn is_valid_transition(from: Status, to: Status) -> bool {
    matches!(
        (from, to),
        (Status::Initializing, Status::Running) |
        (Status::Running, Status::Completed) |
        (Status::Running, Status::Failed) |
        (Status::Running, Status::Paused) |
        (Status::Paused, Status::Running)
    )
}
```

#### Imperative Shell (src/unified_session/)
```rust
// I/O wrapper: Load, transform, save
pub async fn update_session_state(
    session_id: &str,
    event: SessionEvent,
) -> Result<()> {
    // I/O: Load current state
    let current = storage::load_session(session_id).await?;

    // Pure: Calculate next state
    let next = core::session::calculate_next_state(current, event)?;

    // I/O: Save new state
    storage::save_session(session_id, next).await?;

    // Side effect: Log the transition
    log::info!("Session {} transitioned to {:?}", session_id, next.status);

    Ok(())
}
```

### Example 2: Workflow Validation

#### Functional Core (src/core/workflow/)
```rust
// Pure function: Validate workflow structure
pub fn validate_workflow(workflow: &Workflow) -> ValidationResult {
    let mut errors = Vec::new();

    if workflow.name.is_empty() {
        errors.push(ValidationError::MissingName);
    }

    if workflow.steps.is_empty() {
        errors.push(ValidationError::NoSteps);
    }

    for (idx, step) in workflow.steps.iter().enumerate() {
        if let Some(error) = validate_step(step, idx) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        ValidationResult::Valid
    } else {
        ValidationResult::Invalid(errors)
    }
}

// Pure function: Transform workflow for execution
pub fn prepare_workflow(
    workflow: Workflow,
    context: ExecutionContext,
) -> PreparedWorkflow {
    PreparedWorkflow {
        name: workflow.name,
        steps: workflow.steps
            .into_iter()
            .map(|step| interpolate_variables(step, &context))
            .collect(),
        timeout: workflow.timeout.unwrap_or(DEFAULT_TIMEOUT),
        retry_policy: workflow.retry_policy.unwrap_or_default(),
    }
}
```

#### Imperative Shell (src/cook/workflow/)
```rust
// I/O wrapper: Load, validate, execute
pub async fn run_workflow(path: &Path, context: ExecutionContext) -> Result<()> {
    // I/O: Read workflow file
    let content = fs::read_to_string(path).await?;
    let workflow: Workflow = serde_yaml::from_str(&content)?;

    // Pure: Validate workflow
    match core::workflow::validate_workflow(&workflow) {
        ValidationResult::Invalid(errors) => {
            // Side effect: Log errors
            for error in errors {
                log::error!("Workflow validation error: {:?}", error);
            }
            return Err(anyhow!("Invalid workflow"));
        }
        ValidationResult::Valid => {}
    }

    // Pure: Prepare workflow for execution
    let prepared = core::workflow::prepare_workflow(workflow, context);

    // I/O: Execute the workflow
    execute_prepared_workflow(prepared).await
}
```

### Example 3: MapReduce Work Distribution

#### Functional Core (src/core/mapreduce/)
```rust
// Pure function: Calculate work distribution
pub fn distribute_work_items<T>(
    items: Vec<T>,
    max_parallel: usize,
) -> Vec<WorkBatch<T>> {
    let batch_size = (items.len() + max_parallel - 1) / max_parallel;

    items
        .chunks(batch_size)
        .enumerate()
        .map(|(idx, chunk)| WorkBatch {
            id: format!("batch-{}", idx),
            items: chunk.to_vec(),
            priority: calculate_priority(idx, chunk.len()),
        })
        .collect()
}

// Pure function: Calculate batch priority
fn calculate_priority(batch_index: usize, item_count: usize) -> u32 {
    // Prioritize smaller batches and earlier indices
    (1000 - item_count as u32) * 10 + (100 - batch_index as u32)
}

// Pure function: Aggregate results
pub fn aggregate_results(
    batch_results: Vec<BatchResult>,
) -> AggregatedResult {
    let total = batch_results.len();
    let successful = batch_results.iter().filter(|r| r.is_success()).count();
    let failed_items = batch_results
        .into_iter()
        .flat_map(|r| r.failed_items)
        .collect();

    AggregatedResult {
        total_batches: total,
        successful_batches: successful,
        failed_items,
        success_rate: (successful as f64) / (total as f64),
    }
}
```

## Testing Strategies

### Testing the Functional Core

Pure functions are trivial to test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transition() {
        let current = SessionState {
            status: Status::Running,
            ..Default::default()
        };

        let event = SessionEvent::Complete;
        let next = calculate_next_state(current, event).unwrap();

        assert_eq!(next.status, Status::Completed);
    }

    #[test]
    fn test_invalid_transition() {
        let current = SessionState {
            status: Status::Completed,
            ..Default::default()
        };

        let event = SessionEvent::Start;
        let result = calculate_next_state(current, event);

        assert!(matches!(result, Err(StateError::InvalidTransition)));
    }

    #[test]
    fn test_work_distribution() {
        let items = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let batches = distribute_work_items(items, 3);

        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].items.len(), 4);
        assert_eq!(batches[1].items.len(), 3);
        assert_eq!(batches[2].items.len(), 3);
    }
}
```

### Testing the Imperative Shell

Focus on testing the orchestration, not the business logic:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_workflow_loading() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.yaml");

        // Write a valid workflow
        fs::write(&path, "name: test\nsteps:\n  - shell: echo test").unwrap();

        // Test that it loads and validates correctly
        let context = ExecutionContext::default();
        let result = run_workflow(&path, context).await;

        // We're testing the I/O and orchestration work,
        // not the validation logic (that's tested in core)
        assert!(result.is_ok());
    }
}
```

## Benefits

### 1. Testability
- Pure functions require no mocks or stubs
- Tests are fast and deterministic
- Easy to achieve high test coverage
- Property-based testing becomes trivial

### 2. Maintainability
- Clear separation of concerns
- Easy to understand data flow
- Refactoring is safer with pure functions
- Side effects are contained and visible

### 3. Debugging
- Pure functions can be tested in isolation
- Reproducible behavior makes bugs easier to find
- State transitions are explicit and traceable
- I/O issues are separated from logic issues

### 4. Composition
- Small functions combine into complex behavior
- Function composition enables powerful abstractions
- Reusable logic across different contexts
- Pipeline-based data transformations

### 5. Concurrency
- Pure functions are inherently thread-safe
- No shared mutable state to synchronize
- Parallel execution becomes trivial
- Race conditions are eliminated in the core

## Anti-Patterns to Avoid

### 1. Hidden I/O in Core Functions
```rust
// BAD: I/O hidden in core function
fn calculate_result(data: &Data) -> Result<Output> {
    let config = fs::read_to_string("config.json")?;  // Hidden I/O!
    // ... calculation logic
}

// GOOD: Config passed as parameter
fn calculate_result(data: &Data, config: &Config) -> Output {
    // ... pure calculation logic
}
```

### 2. Mutable State in Core
```rust
// BAD: Mutating input
fn process_items(items: &mut Vec<Item>) {
    items.sort();
    items.dedup();
}

// GOOD: Return new data
fn process_items(items: Vec<Item>) -> Vec<Item> {
    let mut processed = items;
    processed.sort();
    processed.dedup();
    processed
}
```

### 3. Side Effects in Core
```rust
// BAD: Logging in core function
fn validate_data(data: &Data) -> bool {
    if data.value < 0 {
        log::error!("Invalid value");  // Side effect!
        return false;
    }
    true
}

// GOOD: Return structured result
fn validate_data(data: &Data) -> ValidationResult {
    if data.value < 0 {
        ValidationResult::Invalid("Negative value")
    } else {
        ValidationResult::Valid
    }
}
```

## Migration Guide

### Step 1: Identify Mixed Functions
Look for functions that:
- Perform I/O operations (file, network, database)
- Have side effects (logging, metrics, global state)
- Are difficult to test without mocks
- Have multiple responsibilities

### Step 2: Extract Pure Logic
1. Identify the core business logic
2. Extract it into a pure function
3. Pass all required data as parameters
4. Return results instead of mutating state

### Step 3: Create I/O Wrapper
1. Handle all I/O operations in the wrapper
2. Call the pure function with loaded data
3. Handle the results (save, log, etc.)
4. Keep the wrapper thin and focused

### Step 4: Update Tests
1. Write simple unit tests for pure functions
2. Write integration tests for I/O wrappers
3. Remove unnecessary mocks and stubs
4. Increase test coverage of business logic

## Conclusion

The Functional Core, Imperative Shell pattern provides a powerful architectural foundation for building maintainable, testable, and reliable software. By separating pure business logic from side effects and I/O operations, we achieve:

- Better testability through pure functions
- Clearer code organization and separation of concerns
- Easier debugging and maintenance
- Improved reliability and predictability
- Natural support for concurrency and parallelism

This pattern is not just a theoretical concept but a practical approach that has proven its value in Prodigy's architecture, enabling us to build complex workflow orchestration with confidence and clarity.