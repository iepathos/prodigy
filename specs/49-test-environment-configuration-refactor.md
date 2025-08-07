---
number: 49
title: Test Environment Configuration Refactor
category: testing
priority: high
status: draft
dependencies: []
created: 2025-01-07
---

# Specification 49: Test Environment Configuration Refactor

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The MMM codebase currently uses environment variables extensively for test configuration and behavior modification. This approach has led to several issues:

1. **Race Conditions**: Tests that modify environment variables (e.g., `MMM_TEST_MODE`, `MMM_TEST_NO_CHANGES_COMMANDS`) can interfere with each other when running in parallel, causing intermittent test failures.

2. **Global State**: Environment variables are process-wide, making it impossible to have different configurations for different test threads running simultaneously.

3. **Test Isolation**: Tests are not properly isolated, as one test's environment variable changes can affect other tests running concurrently.

4. **Maintenance Burden**: The current pattern requires careful management of setting and unsetting environment variables, with potential for leaks if cleanup doesn't happen.

Current environment variables used in tests and production code:
- `MMM_TEST_MODE` - Enables test mode behavior
- `MMM_TEST_NO_CHANGES_COMMANDS` - Specifies commands that should simulate no changes
- `MMM_NO_COMMIT_VALIDATION` - Skips commit validation
- `MMM_TRACK_FOCUS` - Enables focus tracking for tests
- `MMM_WORKTREE` - Specifies worktree name
- `MMM_ARG` - Command-line arguments
- Various other configuration variables

## Objective

Refactor the test configuration system to eliminate environment variable dependencies in favor of dependency injection, providing:
- Deterministic test execution regardless of parallelism
- Proper test isolation
- Type-safe configuration
- Easier testing and maintenance

## Requirements

### Functional Requirements

1. **Configuration Struct**: Create a `TestConfiguration` struct that encapsulates all test-related configuration previously handled by environment variables.

2. **Dependency Injection**: Modify all components that currently read environment variables to accept configuration through constructor parameters or method arguments.

3. **Builder Pattern**: Implement a builder pattern for `TestConfiguration` to allow easy construction in tests with sensible defaults.

4. **Runtime Configuration**: Provide a way to read environment variables once at application startup and convert them to the configuration struct for production use.

5. **Backward Compatibility**: Ensure the refactor doesn't break existing command-line interfaces or production behavior.

### Non-Functional Requirements

1. **Performance**: No performance degradation from the refactor
2. **Thread Safety**: Configuration must be safely shareable across threads
3. **Testability**: Configuration must be easily mockable and configurable in tests
4. **Maintainability**: Clear separation between test and production configuration

## Acceptance Criteria

- [ ] All tests pass consistently when run in parallel with `cargo test`
- [ ] No environment variable mutations in test code
- [ ] All configuration is passed through dependency injection
- [ ] `TestConfiguration` struct implemented with builder pattern
- [ ] Production code reads environment variables once at startup
- [ ] Zero race conditions in test execution
- [ ] All existing tests migrated to new configuration system
- [ ] Documentation updated to reflect new testing patterns
- [ ] No performance regression in test execution time

## Technical Details

### Implementation Approach

1. **Phase 1: Configuration Structure**
   ```rust
   #[derive(Debug, Clone, Default)]
   pub struct TestConfiguration {
       pub test_mode: bool,
       pub no_changes_commands: Vec<String>,
       pub skip_commit_validation: bool,
       pub track_focus: bool,
       pub worktree_name: Option<String>,
       pub additional_args: HashMap<String, String>,
   }

   impl TestConfiguration {
       pub fn builder() -> TestConfigurationBuilder {
           TestConfigurationBuilder::default()
       }
       
       pub fn from_env() -> Self {
           // Read environment variables once
           Self {
               test_mode: std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true",
               no_changes_commands: std::env::var("MMM_TEST_NO_CHANGES_COMMANDS")
                   .unwrap_or_default()
                   .split(',')
                   .map(|s| s.trim().to_string())
                   .collect(),
               skip_commit_validation: std::env::var("MMM_NO_COMMIT_VALIDATION")
                   .unwrap_or_default() == "true",
               track_focus: std::env::var("MMM_TRACK_FOCUS").is_ok(),
               worktree_name: std::env::var("MMM_WORKTREE").ok(),
               additional_args: HashMap::new(),
           }
       }
   }
   ```

2. **Phase 2: Component Refactoring**
   - Modify `WorkflowExecutor` to accept `TestConfiguration` in constructor
   - Update `ClaudeExecutor` to use injected configuration
   - Refactor `MetricsCollector` to accept configuration parameter
   - Update `CookOrchestrator` to use configuration struct

3. **Phase 3: Test Migration**
   - Replace all `std::env::set_var` calls with configuration builders
   - Remove all `std::env::remove_var` calls
   - Update test fixtures to use configuration injection

### Architecture Changes

1. **Configuration Flow**:
   ```
   Application Start
        ↓
   Read Environment Variables Once
        ↓
   Create Configuration Struct
        ↓
   Pass to Components via DI
        ↓
   Components Use Injected Config
   ```

2. **Test Configuration Flow**:
   ```
   Test Setup
        ↓
   Build Test Configuration
        ↓
   Create Components with Config
        ↓
   Run Test
        ↓
   No Cleanup Needed
   ```

### Data Structures

```rust
pub struct TestConfigurationBuilder {
    test_mode: Option<bool>,
    no_changes_commands: Option<Vec<String>>,
    skip_commit_validation: Option<bool>,
    track_focus: Option<bool>,
    worktree_name: Option<String>,
    additional_args: HashMap<String, String>,
}

impl TestConfigurationBuilder {
    pub fn test_mode(mut self, enabled: bool) -> Self {
        self.test_mode = Some(enabled);
        self
    }
    
    pub fn no_changes_commands(mut self, commands: Vec<String>) -> Self {
        self.no_changes_commands = Some(commands);
        self
    }
    
    pub fn build(self) -> TestConfiguration {
        TestConfiguration {
            test_mode: self.test_mode.unwrap_or(false),
            no_changes_commands: self.no_changes_commands.unwrap_or_default(),
            skip_commit_validation: self.skip_commit_validation.unwrap_or(false),
            track_focus: self.track_focus.unwrap_or(false),
            worktree_name: self.worktree_name,
            additional_args: self.additional_args,
        }
    }
}
```

### APIs and Interfaces

Components will be updated to accept configuration:

```rust
impl WorkflowExecutor {
    pub fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        analysis_coordinator: Arc<dyn AnalysisCoordinator>,
        metrics_coordinator: Arc<dyn MetricsCoordinator>,
        user_interaction: Arc<dyn UserInteraction>,
        config: TestConfiguration,  // New parameter
    ) -> Self {
        // ...
    }
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `WorkflowExecutor`
  - `ClaudeExecutor`
  - `MetricsCollector`
  - `CookOrchestrator`
  - All test modules
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Each component tested with different configurations
- **Integration Tests**: Full workflow tests with various configurations
- **Parallel Execution Tests**: Verify no race conditions with parallel test runs
- **Performance Tests**: Benchmark to ensure no performance regression
- **Backward Compatibility Tests**: Verify production behavior unchanged

## Documentation Requirements

- **Code Documentation**: Document new configuration structs and builders
- **Testing Guide**: Update testing documentation with new patterns
- **Migration Guide**: Document how to migrate tests to new system
- **Architecture Updates**: Update ARCHITECTURE.md with configuration flow

## Implementation Notes

1. **Gradual Migration**: Can be implemented component by component
2. **Feature Flag**: Consider using a feature flag during migration
3. **Validation**: Add configuration validation to catch invalid combinations
4. **Debug Output**: Implement Debug trait for easy troubleshooting
5. **Default Values**: Ensure sensible defaults for all configuration options

## Migration and Compatibility

1. **Production Compatibility**: The refactor maintains backward compatibility by reading environment variables at startup in production mode.

2. **Test Migration Path**:
   - Identify all tests using environment variables
   - Migrate tests incrementally, component by component
   - Run both old and new patterns during transition
   - Remove old patterns once migration complete

3. **Breaking Changes**: None for end users; only internal test changes

4. **Rollback Plan**: Git revert if issues discovered, as changes are internal

## Implementation Order

1. Create `TestConfiguration` struct and builder
2. Update `WorkflowExecutor` to use configuration
3. Migrate `WorkflowExecutor` tests
4. Update `ClaudeExecutor` and its tests
5. Update `MetricsCollector` and its tests
6. Update `CookOrchestrator` and its tests
7. Remove all environment variable mutations from tests
8. Document new testing patterns
9. Clean up deprecated code

## Success Metrics

- Zero intermittent test failures due to environment variables
- Test execution time remains the same or improves
- All tests pass with `cargo test --jobs=32` (high parallelism)
- No environment variable mutations in test code
- Improved developer experience when writing tests