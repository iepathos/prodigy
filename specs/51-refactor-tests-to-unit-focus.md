---
number: 51
title: Refactor Tests to Unit-Focused Architecture
category: testing
priority: high
status: draft
dependencies: []
created: 2025-01-06
---

# Specification 51: Refactor Tests to Unit-Focused Architecture

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: none

## Context

The current test suite has significant issues with its integration test architecture:
- 31 integration test files exist but have 0% coverage (they compile but don't execute properly)
- Integration tests are heavily mocked/stubbed, essentially functioning as poorly-structured unit tests
- Overall test coverage is only 58.3% with many critical functions having 0% coverage
- Key modules like `cook/orchestrator.rs` (40.6%), `abstractions/claude.rs` (36.8%), and `main.rs` (0%) have poor coverage
- Integration tests use `#[cfg(tarpaulin)]` conditionals to fake external dependencies
- Test maintenance is difficult due to the split between integration and unit test architectures

The integration tests are not providing real integration value since they mock most external dependencies. This creates a false sense of coverage while missing actual business logic testing.

## Objective

Transform the test architecture from a failing integration-heavy approach to a robust unit-test-focused strategy that provides better coverage, faster feedback, and more maintainable tests. This will improve code quality, developer confidence, and reduce the likelihood of bugs reaching production.

## Requirements

### Functional Requirements

1. **Remove Non-Functional Integration Tests**
   - Delete integration tests that only mock external dependencies
   - Remove tests with 0% coverage that aren't actually executing
   - Eliminate duplicate testing between integration and unit tests

2. **Convert Valuable Integration Tests to Unit Tests**
   - Extract business logic testing from integration tests
   - Move tests into appropriate module-level `#[cfg(test)]` modules
   - Preserve test scenarios while improving isolation

3. **Create Comprehensive Mock Infrastructure**
   - Develop complete mock implementations for `ClaudeClient` trait
   - Create mock `GitOperations` for git-related testing
   - Build mock `SubprocessManager` for command execution testing
   - Implement mock file system operations where needed

4. **Expand Unit Test Coverage**
   - Target critical paths with 0% coverage
   - Focus on high-impact modules (orchestrator, workflow executor, claude client)
   - Ensure all public APIs have comprehensive unit tests
   - Add tests for error handling and edge cases

### Non-Functional Requirements

1. **Test Performance**
   - Unit tests must run in under 5 seconds for the full suite
   - Individual unit test modules should complete in under 1 second
   - Tests must be deterministic and not depend on timing

2. **Test Maintainability**
   - Tests should be co-located with the code they test
   - Mock implementations should be reusable across test modules
   - Test helpers should be well-documented and discoverable

3. **Coverage Goals**
   - Achieve 80% overall code coverage
   - Critical modules must have >90% coverage
   - All error paths must be tested

## Acceptance Criteria

- [ ] All integration tests with 0% coverage are removed
- [ ] Integration tests that only mock dependencies are deleted
- [ ] Valuable test scenarios from integration tests are preserved as unit tests
- [ ] Mock implementations created for all external dependencies:
  - [ ] `MockClaudeClient` with configurable responses
  - [ ] `MockGitOperations` for git command simulation
  - [ ] `MockSubprocessManager` for subprocess testing
  - [ ] Mock file system utilities where needed
- [ ] Unit test coverage increased to at least 80% overall
- [ ] Critical modules have >90% coverage:
  - [ ] `CookOrchestrator`
  - [ ] `WorkflowExecutor`
  - [ ] `ClaudeClient` implementations
  - [ ] `MetricsCollector`
- [ ] All public APIs have comprehensive unit tests
- [ ] Test suite runs in under 5 seconds
- [ ] Documentation updated with new testing strategy

## Technical Details

### Implementation Approach

1. **Phase 1: Analysis and Planning**
   - Analyze each integration test for salvageable scenarios
   - Map integration test scenarios to appropriate unit test locations
   - Identify all external dependencies needing mocks

2. **Phase 2: Mock Infrastructure Development**
   - Create `src/testing/mocks/` module structure
   - Implement trait-based mocks with builder patterns for configuration
   - Develop test fixture generators for common scenarios

3. **Phase 3: Integration Test Removal**
   - Delete non-functional integration tests
   - Extract valuable test logic before deletion
   - Document any integration scenarios that need preservation

4. **Phase 4: Unit Test Enhancement**
   - Add unit tests for all uncovered critical functions
   - Migrate extracted integration test logic to unit tests
   - Ensure comprehensive error path coverage

### Architecture Changes

```rust
// New mock infrastructure structure
src/testing/
├── mocks/
│   ├── mod.rs
│   ├── claude.rs      // MockClaudeClient implementation
│   ├── git.rs         // MockGitOperations implementation
│   ├── subprocess.rs  // MockSubprocessManager implementation
│   └── fs.rs          // Mock file system utilities
├── fixtures/
│   ├── mod.rs
│   └── builders.rs    // Test data builders
└── helpers/
    ├── mod.rs
    └── assertions.rs  // Custom test assertions
```

### Data Structures

```rust
// Example mock builder pattern
pub struct MockClaudeClientBuilder {
    responses: HashMap<String, Result<String>>,
    availability: bool,
    error_on_call: Option<usize>,
}

impl MockClaudeClientBuilder {
    pub fn new() -> Self { ... }
    pub fn with_response(mut self, command: &str, response: Result<String>) -> Self { ... }
    pub fn unavailable(mut self) -> Self { ... }
    pub fn fail_after(mut self, calls: usize) -> Self { ... }
    pub fn build(self) -> MockClaudeClient { ... }
}
```

### APIs and Interfaces

- Maintain existing trait boundaries for mocking
- Use builder pattern for mock configuration
- Provide test helper functions for common scenarios
- Create assertion helpers for complex validations

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - All test files in `/tests` directory
  - All `#[cfg(test)]` modules in `/src`
  - Test coverage reporting configuration
- **External Dependencies**: No new external dependencies

## Testing Strategy

- **Unit Tests**: Primary testing approach using mocks for external dependencies
- **Integration Tests**: Keep only essential end-to-end tests that test real integration points
- **Performance Tests**: Ensure test suite performance meets requirements
- **Coverage Validation**: Use `cargo tarpaulin` to verify coverage goals

## Documentation Requirements

- **Code Documentation**: 
  - Document all mock implementations
  - Add examples to mock builders
  - Document test helpers and utilities
- **User Documentation**: 
  - Update CONTRIBUTING.md with new testing guidelines
  - Create TESTING.md with comprehensive testing strategy
  - Add examples of writing effective unit tests
- **Architecture Updates**: 
  - Update architecture documentation to reflect mock infrastructure
  - Document testing boundaries and responsibilities

## Implementation Notes

1. **Mock Behavior Consistency**
   - Mocks should behave consistently with real implementations
   - Error conditions should be realistically simulated
   - Mock state should be isolated between tests

2. **Test Organization**
   - Keep unit tests in `#[cfg(test)]` modules within source files
   - Use separate files for large test modules
   - Group related tests using nested modules

3. **Coverage Exceptions**
   - Some code paths may be legitimately difficult to test
   - Document coverage exceptions with clear justification
   - Focus on testing business logic over boilerplate

4. **Gradual Migration**
   - Can be implemented incrementally
   - Start with highest-value modules
   - Maintain CI/CD stability during transition

## Migration and Compatibility

- **Breaking Changes**: None for production code
- **Test Migration**: 
  - Existing unit tests remain unchanged
  - Integration tests will be removed or converted
  - CI/CD configuration may need updates
- **Coverage Reporting**: 
  - Update coverage thresholds after migration
  - Adjust coverage exclusion patterns if needed