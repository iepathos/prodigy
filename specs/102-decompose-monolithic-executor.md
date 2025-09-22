---
number: 102
title: Decompose Monolithic Workflow Executor
category: foundation
priority: critical
status: draft
dependencies: [101]
created: 2025-09-22
---

# Specification 102: Decompose Monolithic Workflow Executor

## Context

The `cook/workflow/executor.rs` file is a massive monolith at 5,398 lines, handling execution, validation, interpolation, error recovery, and more. This violates VISION.md principles of simplicity ("small, focused functions") and functional programming ("single responsibility"). The file is difficult to maintain, test, and understand.

Current responsibilities mixed in executor.rs:
- Workflow execution coordination
- Variable interpolation
- Step validation
- Error recovery logic
- Progress tracking
- State management
- Command processing

## Objective

Break down the monolithic executor into focused, single-responsibility modules that follow functional programming principles, making the codebase more maintainable and testable while preserving all current functionality.

## Requirements

### Functional Requirements
- Extract variable interpolation into separate module
- Extract step validation into separate module
- Extract error recovery into separate module
- Extract progress tracking into separate module
- Core executor should focus only on workflow coordination
- All existing functionality must be preserved
- No breaking changes to public APIs

### Non-Functional Requirements
- Each module should be under 200 lines
- Functions should average under 20 lines (VISION.md target)
- Clear module boundaries with minimal coupling
- Comprehensive test coverage for extracted modules
- Performance must remain equivalent or improve

## Acceptance Criteria

- [ ] `executor.rs` reduced to under 1,000 lines
- [ ] Variable interpolation extracted to `interpolation.rs`
- [ ] Step validation extracted to `validation.rs`
- [ ] Error recovery extracted to `recovery.rs`
- [ ] Progress tracking extracted to `progress.rs`
- [ ] Each module has single, clear responsibility
- [ ] All existing tests pass without modification
- [ ] New module-specific test coverage added
- [ ] No cyclic dependencies between modules

## Technical Details

### Proposed Module Structure

```
cook/workflow/
├── executor.rs           # Core coordination only (<1000 lines)
├── interpolation.rs      # Variable interpolation
├── validation.rs         # Step validation
├── recovery.rs          # Error recovery strategies
├── progress.rs          # Progress tracking
└── types.rs             # Shared types and traits
```

### Implementation Approach

1. **Phase 1: Extract Pure Functions**
   - Identify and extract pure interpolation functions
   - Extract validation logic with no side effects
   - Create shared types module for common structures

2. **Phase 2: Extract State Management**
   - Move progress tracking to dedicated module
   - Extract error recovery strategies
   - Maintain clear interfaces between modules

3. **Phase 3: Refactor Core Executor**
   - Slim down executor to pure coordination
   - Use composition pattern to combine modules
   - Ensure single entry point for workflow execution

### Functional Programming Patterns

```rust
// Before: Mixed responsibilities in executor
impl WorkflowExecutor {
    fn execute(&mut self, workflow: Workflow) -> Result<Output> {
        // 200+ lines mixing validation, interpolation, execution, error handling
    }
}

// After: Composed functional modules
pub fn execute_workflow(
    workflow: Workflow,
    context: ExecutionContext,
) -> Result<Output> {
    let validated = validation::validate_workflow(&workflow)?;
    let interpolated = interpolation::interpolate_variables(validated, &context)?;
    let result = core_execution::execute(interpolated, context);
    recovery::handle_result(result, &workflow.recovery_config)
}
```

## Dependencies

- **Spec 101**: Must complete unwrap/panic elimination first to ensure reliable error handling during refactoring

## Testing Strategy

- Extract tests alongside code modules
- Add integration tests for module interactions
- Performance benchmarks to ensure no regression
- Property-based tests for interpolation and validation

## Documentation Requirements

- Document new module architecture
- Update development guidelines for module boundaries
- Create examples of adding new execution features
- Document the functional composition patterns used