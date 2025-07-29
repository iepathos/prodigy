# Specification 39: End-to-End Workflow Testing with Claude CLI Mocking

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: 21-configurable-workflow, 33-batch-spec-implementation, 35-unified-improve-mapping

## Context

The MMM tool relies heavily on integration with Claude CLI for its core functionality. Currently, testing the end-to-end workflow requires actual Claude CLI calls, which:
- Makes tests non-deterministic and dependent on external services
- Increases test execution time and cost
- Prevents comprehensive testing of edge cases and error scenarios
- Makes it difficult to test parallel worktree operations safely

We need a robust testing framework that can mock Claude CLI behavior while testing real git operations and file changes.

## Objective

Implement comprehensive end-to-end testing for MMM workflows with proper mocking of Claude CLI responses while maintaining realistic git operations and file system changes.

## Requirements

### Functional Requirements

1. **Claude CLI Mocking Framework**
   - Mock subprocess calls to `claude` command
   - Support configurable responses based on input patterns
   - Simulate realistic output including spec generation and file modifications
   - Handle error scenarios and timeouts

2. **Workflow Test Coverage**
   - Test default legacy workflow (/mmm-code-review → /mmm-implement-spec → /mmm-lint)
   - Test implement.yml workflow for batch spec implementation
   - Test documentation-workflow.yml for documentation improvements
   - Test product-enhancement-workflow.yml for feature enhancements
   - Test custom workflows with variable substitution

3. **Git Operation Testing**
   - Verify proper git commits are created with expected messages
   - Test worktree creation and isolation
   - Test worktree merge operations with mocked conflict resolution
   - Ensure proper branch management and cleanup

4. **State Management Testing**
   - Verify state files are created and updated correctly
   - Test session recording and history tracking
   - Validate worktree state persistence
   - Test recovery from corrupted state files

### Non-Functional Requirements

1. **Test Performance**
   - Tests should run in under 30 seconds total
   - Parallel test execution support
   - Minimal disk I/O for temporary files

2. **Test Reliability**
   - Deterministic test outcomes
   - Proper cleanup of test artifacts
   - Isolated test environments
   - No interference between test cases

3. **Maintainability**
   - Clear test structure and naming
   - Reusable mock fixtures
   - Easy to add new workflow tests
   - Documentation of test scenarios

## Acceptance Criteria

- [ ] Claude CLI mock framework implemented with configurable responses
- [ ] Mock can simulate spec generation with proper git commits
- [ ] Mock can simulate file modifications with realistic changes
- [ ] Test suite covers default legacy workflow end-to-end
- [ ] Test suite covers implement.yml workflow with batch processing
- [ ] Test suite covers documentation-workflow.yml scenarios
- [ ] Test suite covers product-enhancement-workflow.yml scenarios
- [ ] Tests verify proper git commit creation and messages
- [ ] Tests validate worktree operations with mocked merges
- [ ] Tests confirm state management and persistence
- [ ] All tests run reliably in CI/CD environment
- [ ] Test execution time under 30 seconds
- [ ] Mock responses are realistic and maintainable

## Technical Details

### Implementation Approach

1. **Mock Infrastructure**
   ```rust
   // src/test_utils/claude_mock.rs
   pub struct ClaudeMock {
       responses: HashMap<String, MockResponse>,
       call_history: Vec<MockCall>,
   }
   
   pub struct MockResponse {
       stdout: String,
       stderr: String,
       exit_code: i32,
       delay_ms: Option<u64>,
   }
   ```

2. **Command Interception**
   - Override Command execution in test environment
   - Pattern match on command arguments
   - Return configured mock responses
   - Record call history for assertions

3. **Workflow Test Structure**
   ```rust
   #[tokio::test]
   async fn test_legacy_workflow_end_to_end() {
       let mock = ClaudeMock::new()
           .with_response("/mmm-code-review", mock_review_response())
           .with_response("/mmm-implement-spec", mock_implement_response())
           .with_response("/mmm-lint", mock_lint_response());
           
       // Run workflow with mock
       // Assert git commits created
       // Assert files modified
       // Assert state updated
   }
   ```

### Architecture Changes

1. **Test Module Structure**
   ```
   src/
   ├── test_utils/
   │   ├── mod.rs
   │   ├── claude_mock.rs      # Claude CLI mocking
   │   ├── git_helpers.rs      # Git test utilities
   │   └── fixtures.rs         # Test data fixtures
   ├── cook/
   │   └── tests/
   │       ├── mod.rs
   │       ├── legacy_workflow.rs
   │       ├── implement_workflow.rs
   │       ├── documentation_workflow.rs
   │       └── product_workflow.rs
   ```

2. **Dependency Injection**
   - Abstract command execution behind trait
   - Allow mock injection in tests
   - Maintain production behavior unchanged

### Data Structures

```rust
// Mock response templates
struct ReviewTemplate {
    spec_id: String,
    issues_found: Vec<String>,
    score: f32,
}

struct ImplementTemplate {
    files_changed: Vec<FileChange>,
    success: bool,
}

struct FileChange {
    path: String,
    changes: Vec<Change>,
}
```

### APIs and Interfaces

1. **Mock Builder API**
   ```rust
   ClaudeMock::new()
       .expect_call("/mmm-code-review")
       .with_env("MMM_FOCUS", "performance")
       .returns_spec("temp-001", vec!["Issue 1", "Issue 2"])
       .then()
       .expect_call("/mmm-implement-spec")
       .with_args(["temp-001"])
       .modifies_files(vec![("src/main.rs", "improved code")])
       .commits("Implement spec temp-001: Fix performance issues")
   ```

## Dependencies

- **Prerequisites**: Configurable workflow system (Spec 21)
- **Affected Components**: cook module, worktree module, config module
- **External Dependencies**: tempfile for test isolation, mockall or similar for mocking

## Testing Strategy

- **Unit Tests**: Mock infrastructure components
- **Integration Tests**: End-to-end workflow execution
- **Performance Tests**: Verify test execution time
- **Regression Tests**: Ensure production behavior unchanged

## Documentation Requirements

- **Test Documentation**: Document each test scenario and expected behavior
- **Mock Usage Guide**: How to add new mock responses
- **CI/CD Integration**: Setup for automated testing

## Implementation Notes

1. **Mock Realism**
   - Base mock responses on actual Claude CLI output
   - Include realistic delays for subprocess execution
   - Simulate both success and failure scenarios

2. **Test Isolation**
   - Each test runs in isolated temp directory
   - No shared state between tests
   - Proper cleanup on test completion or failure

3. **Assertion Helpers**
   - Git commit assertions
   - File change verification
   - State validation utilities

4. **Example Workflows**
   - Include all example workflows from examples/ directory
   - Test with various configuration options
   - Cover edge cases and error scenarios

## Migration and Compatibility

- No breaking changes to production code
- Tests can be run alongside existing test suite
- Gradual migration from any existing integration tests
- Mock framework available for future test development