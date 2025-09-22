---
number: 101
title: Merge Workflow Test Coverage
category: testing
priority: high
status: draft
dependencies: []
created: 2025-09-21
---

# Specification 101: Merge Workflow Test Coverage

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The merge workflow feature allows Prodigy to execute custom commands when merging worktree changes back to the main branch. This includes support for both simplified (direct command array) and full (with commands wrapper) syntax, variable interpolation, and proper logging with verbosity control. Currently, this functionality lacks comprehensive test coverage, creating risk of regressions and making it difficult to verify correct behavior across different scenarios.

The following components need test coverage:
- `MergeWorkflow` struct deserialization in `src/config/mapreduce.rs`
- `execute_custom_merge_workflow` method in `src/worktree/manager.rs`
- Merge workflow logging behavior with various verbosity settings
- Environment variable handling for streaming output control

## Objective

Implement comprehensive unit and integration tests for the merge workflow functionality to ensure reliability, prevent regressions, and validate correct behavior across all supported use cases and configurations.

## Requirements

### Functional Requirements

1. **Deserialization Testing**
   - Test parsing of simplified syntax (direct command array)
   - Test parsing of full syntax (with commands wrapper and timeout)
   - Test default timeout application
   - Test invalid syntax handling
   - Test empty merge workflow handling

2. **Execution Logic Testing**
   - Test shell command execution with variable interpolation
   - Test Claude command execution with variable interpolation
   - Test command failure handling and error propagation
   - Test output collection from commands
   - Test working directory context

3. **Variable Interpolation Testing**
   - Test `${merge.worktree}` substitution
   - Test `${merge.source_branch}` substitution
   - Test `${merge.target_branch}` substitution
   - Test `${merge.session_id}` substitution
   - Test handling of missing variables

4. **Logging Behavior Testing**
   - Test logging output with verbosity = 0 (default)
   - Test logging output with verbosity >= 1 (verbose)
   - Test `PRODIGY_CLAUDE_CONSOLE_OUTPUT` environment variable override
   - Test shell command output display
   - Test Claude command streaming behavior

5. **Integration Testing**
   - Test end-to-end workflow execution with merge configuration
   - Test merge workflow in MapReduce context
   - Test merge workflow in regular workflow context
   - Test merge workflow with actual git operations

### Non-Functional Requirements

1. **Test Organization**
   - Tests should be organized into logical modules
   - Use descriptive test names following Rust conventions
   - Include both unit tests and integration tests
   - Mock external dependencies where appropriate

2. **Test Coverage**
   - Achieve at least 80% code coverage for merge workflow code
   - Cover all error paths and edge cases
   - Include property-based tests where applicable

3. **Test Performance**
   - Tests should run quickly (< 100ms for unit tests)
   - Integration tests should complete within 5 seconds
   - Tests should not require network access

## Acceptance Criteria

- [ ] Unit tests for `MergeWorkflow` deserialization cover both syntax formats
- [ ] Unit tests validate default timeout application and error handling
- [ ] Unit tests for `execute_custom_merge_workflow` cover all command types
- [ ] Variable interpolation is tested for all supported variables
- [ ] Logging behavior is tested for all verbosity levels
- [ ] Environment variable overrides are properly tested
- [ ] Integration tests verify end-to-end merge workflow execution
- [ ] All tests pass consistently in CI environment
- [ ] Test coverage report shows >= 80% coverage for merge workflow code
- [ ] Documentation includes examples of running merge workflow tests

## Technical Details

### Implementation Approach

1. **Unit Test Structure**
   ```rust
   // src/config/mapreduce.rs
   #[cfg(test)]
   mod merge_workflow_tests {
       use super::*;

       #[test]
       fn test_deserialize_simplified_syntax() { ... }

       #[test]
       fn test_deserialize_full_syntax() { ... }

       #[test]
       fn test_default_timeout() { ... }
   }

   // src/worktree/manager.rs
   #[cfg(test)]
   mod merge_execution_tests {
       use super::*;

       #[test]
       fn test_execute_shell_command() { ... }

       #[test]
       fn test_execute_claude_command() { ... }

       #[test]
       fn test_variable_interpolation() { ... }
   }
   ```

2. **Mock Infrastructure**
   ```rust
   struct MockSubprocessRunner {
       expected_commands: Vec<ExpectedCommand>,
       responses: Vec<ProcessOutput>,
   }

   struct MockClaudeExecutor {
       expected_commands: Vec<String>,
       responses: Vec<ClaudeResponse>,
   }
   ```

3. **Integration Test Structure**
   ```rust
   // tests/merge_workflow_integration.rs
   #[test]
   fn test_merge_workflow_end_to_end() { ... }

   #[test]
   fn test_merge_workflow_with_failures() { ... }

   #[test]
   fn test_merge_workflow_logging_levels() { ... }
   ```

### Architecture Changes

No architectural changes required. Tests will be added alongside existing code following Rust testing conventions.

### Data Structures

1. **Test Fixtures**
   ```rust
   const SIMPLIFIED_MERGE_YAML: &str = r#"
   merge:
     - shell: "git fetch origin"
     - claude: "/merge-master"
   "#;

   const FULL_MERGE_YAML: &str = r#"
   merge:
     commands:
       - shell: "git fetch origin"
       - claude: "/merge-master"
     timeout: 900
   "#;
   ```

2. **Test Helpers**
   ```rust
   fn create_test_merge_workflow() -> MergeWorkflow { ... }
   fn create_test_worktree_manager() -> WorktreeManager { ... }
   fn assert_command_executed(cmd: &str) { ... }
   ```

### APIs and Interfaces

No new APIs. Tests will use existing public interfaces and test-only helper methods where needed.

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/config/mapreduce.rs`
  - `src/worktree/manager.rs`
  - Test infrastructure in `tests/` directory
- **External Dependencies**:
  - `mockall` or similar mocking framework may be beneficial
  - `tempfile` for integration tests requiring file system operations

## Testing Strategy

- **Unit Tests**:
  - Test each component in isolation
  - Mock external dependencies
  - Focus on edge cases and error conditions
  - Use property-based testing for variable interpolation

- **Integration Tests**:
  - Test complete merge workflow execution
  - Use temporary git repositories
  - Verify actual command execution
  - Test with real workflow configurations

- **Performance Tests**:
  - Ensure merge workflow execution doesn't regress
  - Measure command execution overhead

- **User Acceptance**:
  - Manual verification that tests cover real-world scenarios
  - Ensure test output is clear and diagnostic

## Documentation Requirements

- **Code Documentation**:
  - Document test purpose and scenarios
  - Include examples in test function documentation
  - Document any test-specific helper functions

- **User Documentation**:
  - Update developer guide with how to run merge workflow tests
  - Include test coverage commands
  - Document how to add new merge workflow tests

- **Architecture Updates**:
  - Update ARCHITECTURE.md to reference test organization
  - Document testing strategy for workflow features

## Implementation Notes

1. **Test Isolation**: Each test should be completely independent and not rely on shared state
2. **Deterministic Output**: Tests should produce consistent results regardless of environment
3. **Clear Assertions**: Test failures should clearly indicate what went wrong
4. **Mock vs Real**: Use mocks for unit tests, real implementations for integration tests
5. **Coverage Gaps**: Pay special attention to error paths and edge cases
6. **Logging Tests**: May need to capture and assert on log output

## Migration and Compatibility

No migration required. Tests are additive and don't affect existing functionality. All tests should be compatible with the current CI/CD pipeline and development workflow.