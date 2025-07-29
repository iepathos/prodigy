# Specification 40: Improve Test Coverage

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: 39 (end-to-end workflow testing)

## Context

The mmm project currently has low test coverage (43.26%) despite having numerous test files. Analysis reveals that most tests run in `MMM_TEST_MODE=true` which bypasses actual implementation logic. Key modules like `src/cook/mod.rs` (19.7% coverage) and `src/main.rs` (30.5% coverage) lack adequate testing. The code directly calls external commands (git, Claude CLI) without abstraction layers, making it difficult to test error paths and complex scenarios.

## Objective

Improve test coverage to at least 70% by adding abstraction layers for external dependencies, creating comprehensive unit tests, and testing error scenarios without relying on `MMM_TEST_MODE`.

## Requirements

### Functional Requirements

1. **Abstraction Layer for External Commands**
   - Create trait-based abstractions for git operations
   - Create trait-based abstractions for Claude CLI calls
   - Implement production and mock implementations
   - Enable dependency injection in core modules

2. **Unit Tests for Core Functions**
   - Test individual functions in isolation
   - Cover both success and error paths
   - Test edge cases and boundary conditions
   - Remove dependency on `MMM_TEST_MODE`

3. **Error Scenario Testing**
   - Test git command failures
   - Test Claude CLI not found scenarios
   - Test network failures and retry logic
   - Test invalid configurations
   - Test merge conflicts

4. **Interactive Function Testing**
   - Mock stdin/stdout for testing prompts
   - Test TTY detection logic
   - Test user input handling
   - Test merge prompt workflows

5. **State Management Testing**
   - Test worktree state transitions
   - Test session management
   - Test state persistence and recovery
   - Test concurrent access scenarios

### Non-Functional Requirements

- Tests must run quickly (< 30 seconds for unit tests)
- Tests must be deterministic and repeatable
- Tests must not require external dependencies
- Coverage tools must accurately track test coverage

## Acceptance Criteria

- [ ] Overall test coverage reaches at least 70%
- [ ] `src/cook/mod.rs` coverage exceeds 60%
- [ ] `src/main.rs` coverage exceeds 50%
- [ ] All error paths have at least one test
- [ ] Interactive functions have mock-based tests
- [ ] Tests run without `MMM_TEST_MODE`
- [ ] CI/CD pipeline includes coverage reporting
- [ ] Coverage report is generated on each commit

## Technical Details

### Implementation Approach

1. **Phase 1: Create Abstraction Layers**
   ```rust
   // Example trait for git operations
   pub trait GitOperations: Send + Sync {
       async fn commit(&self, message: &str) -> Result<()>;
       async fn create_worktree(&self, name: &str) -> Result<PathBuf>;
       async fn merge_branch(&self, branch: &str) -> Result<()>;
   }
   
   // Example trait for Claude CLI
   pub trait ClaudeClient: Send + Sync {
       async fn execute_command(&self, cmd: &str, args: &[&str]) -> Result<String>;
       async fn check_availability(&self) -> Result<bool>;
   }
   ```

2. **Phase 2: Refactor Core Modules**
   - Inject dependencies through constructors
   - Use trait objects or generics for flexibility
   - Maintain backward compatibility

3. **Phase 3: Create Comprehensive Tests**
   - Unit tests for each public function
   - Integration tests with mocked dependencies
   - Property-based tests for complex logic

### Architecture Changes

- Add `src/testing/` module for test utilities
- Add trait definitions in appropriate modules
- Refactor `cook::run()` to accept injected dependencies
- Create mock implementations for all traits

### Data Structures

```rust
// Test fixtures
pub struct TestContext {
    pub git_ops: Box<dyn GitOperations>,
    pub claude_client: Box<dyn ClaudeClient>,
    pub temp_dir: TempDir,
}

// Mock implementations
pub struct MockGitOperations {
    pub commit_responses: Vec<Result<()>>,
    pub worktree_responses: Vec<Result<PathBuf>>,
}

pub struct MockClaudeClient {
    pub command_responses: HashMap<String, Result<String>>,
    pub availability: bool,
}
```

### APIs and Interfaces

No external API changes. Internal refactoring only.

## Dependencies

- **Prerequisites**: Spec 39 (end-to-end workflow testing infrastructure)
- **Affected Components**: 
  - `src/cook/mod.rs` - Major refactoring
  - `src/cook/git_ops.rs` - Extract to trait
  - `src/cook/retry.rs` - Add mock support
  - `src/main.rs` - Dependency injection
- **External Dependencies**: 
  - `mockall` or similar mocking framework
  - `proptest` for property-based testing

## Testing Strategy

- **Unit Tests**: Test each function with mocked dependencies
- **Integration Tests**: Test workflows with controlled mock responses
- **Performance Tests**: Ensure abstractions don't impact performance
- **Coverage Analysis**: Use `cargo-tarpaulin` with proper configuration

## Documentation Requirements

- **Code Documentation**: Document all new traits and their contracts
- **Test Documentation**: Explain test scenarios and mock behavior
- **Architecture Updates**: Update ARCHITECTURE.md with abstraction layer details

## Implementation Notes

1. Start with the most critical untested functions
2. Prioritize error paths that are currently impossible to test
3. Use builder pattern for complex test setups
4. Consider using `rstest` for parameterized tests
5. Ensure mocks are realistic and match actual behavior

## Migration and Compatibility

- No breaking changes to external APIs
- Internal refactoring must maintain all existing functionality
- Gradual migration path for each module
- Feature flags if needed for staged rollout