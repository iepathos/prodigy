---
number: 106
title: Add Comprehensive Test Coverage for Critical Modules
category: testing
priority: high
status: draft
dependencies: [101, 103]
created: 2025-09-21
---

# Specification 106: Add Comprehensive Test Coverage for Critical Modules

## Context

The codebase has significant gaps in test coverage, particularly in critical modules handling session management, storage migration, CLI commands, and configuration. Over 20 modules lack any test coverage, creating risk during refactoring and limiting confidence in system reliability. This violates the principle of comprehensive testing and makes it difficult to safely implement other improvements.

## Objective

Achieve comprehensive test coverage for all critical modules, with focus on business logic, error paths, and edge cases. Target 80% code coverage overall, with 100% coverage for critical paths.

## Requirements

### Functional Requirements

1. Add unit tests for all untested critical modules
2. Implement integration tests for workflow execution paths
3. Add property-based tests for complex transformations
4. Create test fixtures and utilities for common patterns
5. Priority modules requiring tests:
   - `/src/init/command.rs` - Command initialization
   - `/src/unified_session/manager.rs` - Session management
   - `/src/unified_session/state.rs` - Session state handling
   - `/src/storage/migrate.rs` - Storage migration logic
   - `/src/cli/yaml_migrator.rs` - YAML migration
   - `/src/cli/analytics_command.rs` - Analytics commands
   - `/src/storage/config.rs` - Storage configuration

### Non-Functional Requirements

- Tests must be deterministic and reliable
- Test execution time under 30 seconds for unit tests
- Tests should be maintainable and self-documenting
- Follow AAA (Arrange-Act-Assert) pattern
- Minimize test dependencies and mocking

## Acceptance Criteria

- [ ] All identified critical modules have test files
- [ ] 80% overall code coverage achieved
- [ ] 100% coverage for error handling paths
- [ ] All tests pass consistently (no flaky tests)
- [ ] Test documentation explains test scenarios
- [ ] Performance benchmarks for critical operations

## Technical Details

### Testing Strategy

1. **Unit Testing Approach**
   ```rust
   // Test file structure for each module
   #[cfg(test)]
   mod tests {
       use super::*;
       use anyhow::Result;

       // Test fixtures
       fn setup() -> TestContext {
           TestContext::new()
       }

       // Happy path tests
       #[test]
       fn test_normal_operation() -> Result<()> {
           let ctx = setup();
           let result = operation(&ctx)?;
           assert_eq!(result, expected);
           Ok(())
       }

       // Error path tests
       #[test]
       fn test_error_conditions() {
           let ctx = setup();
           let result = operation_that_fails(&ctx);
           assert!(result.is_err());
           assert!(result.unwrap_err().to_string().contains("expected"));
       }

       // Edge case tests
       #[test]
       fn test_edge_cases() -> Result<()> {
           test_empty_input()?;
           test_single_element()?;
           test_maximum_size()?;
           Ok(())
       }
   }
   ```

2. **Integration Testing Pattern**
   ```rust
   // tests/integration/workflow_execution.rs
   use tempfile::TempDir;
   use prodigy::workflow::Executor;

   #[tokio::test]
   async fn test_full_workflow_execution() -> Result<()> {
       let temp = TempDir::new()?;
       let workflow = create_test_workflow();

       let executor = Executor::new(temp.path());
       let result = executor.execute(workflow).await?;

       assert!(result.success);
       assert_eq!(result.steps_completed, 5);
       verify_outputs(&temp)?;
       Ok(())
   }
   ```

3. **Property-Based Testing**
   ```rust
   use proptest::prelude::*;

   proptest! {
       #[test]
       fn test_normalize_idempotent(input in any::<WorkflowConfig>()) {
           let normalized_once = normalize(input.clone());
           let normalized_twice = normalize(normalized_once.clone());
           prop_assert_eq!(normalized_once, normalized_twice);
       }

       #[test]
       fn test_never_panics(input in any::<String>()) {
           let result = parse_safely(&input);
           // Should return Result, never panic
           prop_assert!(result.is_ok() || result.is_err());
       }
   }
   ```

### Test Coverage by Module

1. **Session Management Tests**
   - Session creation and initialization
   - State transitions and persistence
   - Concurrent session handling
   - Recovery after crashes
   - Session cleanup and expiration

2. **Storage Migration Tests**
   - Migration from v1 to v2 format
   - Backward compatibility
   - Data integrity during migration
   - Rollback scenarios
   - Large dataset migrations

3. **CLI Command Tests**
   - Command parsing and validation
   - Argument handling edge cases
   - Interactive prompt testing
   - Output formatting verification
   - Error message clarity

4. **Configuration Tests**
   - Configuration loading from multiple sources
   - Environment variable overrides
   - Default value handling
   - Validation of invalid configs
   - Configuration merging logic

### Test Utilities and Fixtures

```rust
// src/test_utils/mod.rs
pub struct TestContext {
    pub temp_dir: TempDir,
    pub config: Config,
    pub executor: MockExecutor,
}

impl TestContext {
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    pub fn with_config(config: Config) -> Self {
        let temp_dir = TempDir::new().unwrap();
        let executor = MockExecutor::new();
        Self { temp_dir, config, executor }
    }

    pub fn create_test_workflow(&self) -> Workflow {
        // Standard test workflow
    }

    pub fn verify_state(&self, expected: &State) -> Result<()> {
        // State verification helper
    }
}
```

## Dependencies

- Depends on Spec 101 (error handling) for testable errors
- Depends on Spec 103 (I/O separation) for easier testing
- May require adding proptest as dev dependency

## Testing Strategy

1. **Phase 1: Critical Path Coverage**
   - Focus on modules with zero tests
   - Add basic happy path and error tests
   - Ensure all public APIs are tested

2. **Phase 2: Comprehensive Coverage**
   - Add edge case testing
   - Implement property-based tests
   - Add performance benchmarks

3. **Phase 3: Test Infrastructure**
   - Create shared test utilities
   - Implement test data generators
   - Add continuous coverage monitoring

## Documentation Requirements

- Document testing best practices
- Create testing guide for contributors
- Document test utilities and fixtures
- Add examples of each test pattern
- Include coverage reporting setup