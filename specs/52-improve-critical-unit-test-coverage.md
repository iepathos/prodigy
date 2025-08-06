---
number: 52
title: Improve Unit Test Coverage for Critical Components
category: testing
priority: high
status: draft
dependencies: [51]
created: 2025-08-06
---

# Specification 52: Improve Unit Test Coverage for Critical Components

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: [51] - Refactor Tests to Unit-Focused Architecture

## Context

After removing integration tests and establishing a mock infrastructure (spec 51), the project has 58.44% unit test coverage with 524 passing tests. However, critical components have very low coverage:

**Critical Files with <30% Coverage:**
- `cook/workflow/executor.rs`: 0% (2/293 lines) - Core workflow execution
- `cook/coordinators/*.rs`: 2-5% - Essential coordination components
- `cook/orchestrator.rs`: 29% (178/608 lines) - Main orchestration logic
- `cook/git_ops.rs`: 17% (4/23 lines) - Git operations
- `context/analyzer.rs`: 20% (46/222 lines) - Context analysis
- `abstractions/claude.rs`: 26% (67/249 lines) - Claude client abstraction
- `subprocess/git.rs`: 25% (35/140 lines) - Git subprocess handling
- `worktree/manager.rs`: 28% (121/420 lines) - Worktree management

These components are critical to MMM's core functionality and need comprehensive unit test coverage.

## Objective

Increase unit test coverage to at least 80% overall and ensure all critical components have >70% coverage through comprehensive unit tests using the mock infrastructure established in spec 51.

## Requirements

### Functional Requirements

1. **Cook Workflow Components (Priority 1)**
   - Add unit tests for `WorkflowExecutor` using mock Claude and Git clients
   - Test all workflow step types (claude, shell, test commands)
   - Test conditional execution and iteration logic
   - Test error handling and retry mechanisms
   - Target: >80% coverage for workflow/executor.rs

2. **Cook Coordinators (Priority 1)**
   - Test `ExecutionCoordinator` with mock subprocess runners
   - Test `WorkflowCoordinator` with mock workflow configs
   - Test `EnvironmentCoordinator` with mock file system
   - Test `SessionCoordinator` with mock state management
   - Target: >70% coverage for all coordinator modules

3. **Cook Orchestrator (Priority 1)**
   - Test full orchestration flow with all mocked dependencies
   - Test iteration control and stopping conditions
   - Test analysis integration and metrics collection
   - Test worktree mode vs normal mode
   - Target: >70% coverage for orchestrator.rs

4. **Git Operations (Priority 2)**
   - Test all git command wrappers with mock subprocess
   - Test error handling for git failures
   - Test worktree operations
   - Target: >80% coverage for git_ops.rs and subprocess/git.rs

5. **Context Analysis (Priority 2)**
   - Test dependency graph building with mock file system
   - Test architecture pattern detection
   - Test technical debt identification
   - Test coverage mapping
   - Target: >70% coverage for context/analyzer.rs

6. **Claude Abstraction (Priority 2)**
   - Test retry logic with mock failures
   - Test command building and environment setup
   - Test availability checking
   - Target: >70% coverage for abstractions/claude.rs

### Non-Functional Requirements

1. **Test Quality**
   - Each test should be focused on a single behavior
   - Tests should be independent and not rely on shared state
   - Use descriptive test names that explain what is being tested
   - Include both positive and negative test cases

2. **Mock Usage**
   - Leverage the mock infrastructure from spec 51
   - Create reusable test fixtures for common scenarios
   - Ensure mocks accurately represent real behavior

3. **Performance**
   - Unit tests should complete in under 30 seconds total
   - Individual test modules should complete in under 2 seconds

## Acceptance Criteria

- [ ] Overall unit test coverage increased to >80%
- [ ] All critical components have >70% coverage:
  - [ ] WorkflowExecutor >80%
  - [ ] All coordinators >70%
  - [ ] CookOrchestrator >70%
  - [ ] Git operations >80%
  - [ ] Context analyzer >70%
  - [ ] Claude abstraction >70%
- [ ] All new tests pass consistently
- [ ] Test execution time remains under 30 seconds
- [ ] No test flakiness or race conditions
- [ ] Clear test documentation and examples

## Technical Details

### Testing Strategy

1. **Use Builder Pattern for Complex Test Data**
```rust
TestWorkflowBuilder::new()
    .with_claude_command("/mmm-code-review")
    .with_shell_command("cargo test")
    .with_condition("test -f Cargo.toml")
    .build()
```

2. **Mock All External Dependencies**
```rust
let mock_claude = MockClaudeClientBuilder::new()
    .with_success("/mmm-code-review", "Found issues")
    .fail_after(3)  // Simulate failure after 3 calls
    .build();
```

3. **Test Error Paths Thoroughly**
```rust
#[test]
fn test_workflow_handles_claude_unavailable() {
    let mock_claude = MockClaudeClientBuilder::new()
        .unavailable()
        .build();
    // Test that workflow handles unavailability gracefully
}
```

4. **Use Parameterized Tests for Similar Scenarios**
```rust
#[test_case("git", "status" ; "git status")]
#[test_case("git", "diff" ; "git diff")]
#[test_case("git", "log" ; "git log")]
fn test_git_command_execution(cmd: &str, arg: &str) {
    // Test different git commands with same logic
}
```

### File Organization

Each module should have its tests in a `#[cfg(test)]` module at the bottom of the file:

```rust
// src/cook/workflow/executor.rs

// ... implementation ...

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mocks::*;
    use crate::testing::fixtures::*;
    
    #[test]
    fn test_executor_initialization() {
        // Test code
    }
}
```

### Priority Order for Implementation

1. **Phase 1**: Cook workflow and coordinators (highest impact on core functionality)
2. **Phase 2**: Cook orchestrator and git operations  
3. **Phase 3**: Context analysis and Claude abstraction
4. **Phase 4**: Remaining low-coverage modules

## Dependencies

- Spec 51 mock infrastructure must be available
- Existing unit tests must continue to pass
- No changes to production code interfaces (only adding tests)

## Testing Strategy

- Run `cargo test --lib` after each module to ensure no regressions
- Use `cargo tarpaulin --lib` to verify coverage improvements
- Review test quality through code review before merging

## Documentation Requirements

- Each test should have a comment explaining what behavior it tests
- Complex test setups should be documented
- Add examples of using the mock infrastructure to CONTRIBUTING.md

## Implementation Notes

1. **Avoid Over-Mocking**
   - Don't mock internal implementation details
   - Only mock external dependencies and I/O
   - Test behavior, not implementation

2. **Test Naming Convention**
   - Use descriptive names: `test_executor_retries_on_transient_failure`
   - Group related tests with common prefixes
   - Include the condition being tested in the name

3. **Coverage vs Quality**
   - Focus on testing critical paths first
   - Don't add trivial tests just for coverage
   - Ensure tests actually verify behavior, not just execute code

## Success Metrics

- Coverage increase from 58.44% to >80%
- All critical components >70% coverage
- No increase in test flakiness
- Test execution time <30 seconds
- Developer confidence in making changes increased