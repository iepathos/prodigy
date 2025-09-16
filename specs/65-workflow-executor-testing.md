---
number: 65
title: Workflow Executor Testing
category: testing
priority: critical
status: draft
dependencies: []
created: 2025-09-16
---

# Specification 65: Workflow Executor Testing

**Category**: testing
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The Workflow Executor is responsible for orchestrating command execution, variable management, and control flow. Currently at 24.7% coverage (359/1453 lines), it has 1,094 uncovered lines. Comprehensive testing would add ~4.2% to overall coverage and ensure reliability of core workflow execution logic.

## Objective

Increase Workflow Executor test coverage from 24.7% to 70%+ by implementing tests for all command types, variable interpolation, error handling strategies, and complex control flow scenarios.

## Requirements

### Functional Requirements
- Test all command types (claude, shell, test, goal_seek, foreach)
- Test variable interpolation and capture mechanisms
- Test error handling and on_failure strategies
- Test conditional execution (if/when clauses)
- Test loop constructs and iteration limits
- Test timeout handling at command and workflow levels
- Test git commit verification logic
- Test session state management

### Non-Functional Requirements
- Tests must use mock executors for determinism
- Tests must complete within 20 seconds total
- Tests must cover both success and failure paths
- Tests must verify state transitions

## Acceptance Criteria

- [ ] Workflow Executor coverage reaches 70% or higher
- [ ] All command types have comprehensive tests
- [ ] Variable interpolation edge cases are covered
- [ ] Error handling strategies are verified
- [ ] Control flow logic is fully tested
- [ ] Session management integration works correctly
- [ ] Git operations are properly mocked
- [ ] All tests pass in CI environment

## Technical Details

### Implementation Approach

#### Core Components to Test

1. **Command Execution**
   ```rust
   // Test each command type
   - Claude command with arguments
   - Shell command with environment variables
   - Test command with retry logic
   - GoalSeek with iterative refinement
   - Foreach with collection processing
   ```

2. **Variable Management**
   ```rust
   // Variable interpolation scenarios
   - Simple variable substitution: ${var}
   - Nested variables: ${var.field.subfield}
   - Array indexing: ${items[0]}
   - Default values: ${var:-default}
   - Command output capture
   - Variable scoping in loops
   ```

3. **Error Handling**
   ```rust
   // Error recovery strategies
   - on_failure with retry
   - on_failure with alternate commands
   - max_attempts configuration
   - fail_workflow behavior
   - Error propagation
   ```

4. **Control Flow**
   ```rust
   // Conditional and loop execution
   - if conditions with expressions
   - when clauses for dynamic conditions
   - foreach loops with collections
   - Nested control structures
   - Break and continue semantics
   ```

### Test Structure

```rust
// tests/workflow/executor_tests.rs
mod command_execution_tests;
mod variable_interpolation_tests;
mod error_handling_tests;
mod control_flow_tests;
mod session_management_tests;

// Mock implementations
pub struct MockClaudeExecutor {
    responses: HashMap<String, String>,
    should_fail: bool,
}

pub struct MockShellExecutor {
    exit_codes: HashMap<String, i32>,
    outputs: HashMap<String, String>,
}

pub struct MockSessionManager {
    state: Arc<Mutex<SessionState>>,
}
```

### Key Test Scenarios

```rust
#[tokio::test]
async fn test_variable_interpolation_complex() {
    let workflow = r#"
        variables:
          base_path: "/tmp"
          items: ["file1.txt", "file2.txt"]

        steps:
          - shell: "echo ${base_path}/${items[0]}"
            capture_output: file_path
          - claude: "/analyze ${file_path}"
    "#;
    // Verify interpolation and capture
}

#[tokio::test]
async fn test_on_failure_retry_strategy() {
    let workflow = r#"
        steps:
          - shell: "flaky_command"
            on_failure:
              max_attempts: 3
              retry_delay: 1
              fallback:
                - shell: "recovery_command"
    "#;
    // Verify retry behavior
}

#[tokio::test]
async fn test_foreach_with_conditional() {
    let workflow = r#"
        steps:
          - foreach: "${files}"
            as: file
            steps:
              - if: "${file.size} > 1000"
                then:
                  - shell: "process_large ${file.path}"
                else:
                  - shell: "process_small ${file.path}"
    "#;
    // Verify loop and conditional logic
}

#[tokio::test]
async fn test_commit_verification() {
    // Test git commit requirements
    // Mock git operations
    // Verify commit detection
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: WorkflowExecutor, CommandRegistry, SessionManager
- **External Dependencies**: Mock frameworks for git operations

## Testing Strategy

- **Unit Tests**: Individual method testing
- **Integration Tests**: Complete workflow execution
- **Edge Cases**: Boundary conditions and error paths
- **State Tests**: Verify state transitions
- **Regression Tests**: Previous bug scenarios

## Documentation Requirements

- **Test Scenarios**: Document each test's purpose
- **Mock Behavior**: Explain mock configurations
- **Coverage Metrics**: Track coverage improvements

## Implementation Notes

### Complex Scenarios to Cover

1. **Nested Variable Resolution**
   ```yaml
   variables:
     config:
       paths:
         output: "/tmp/out"
   steps:
     - shell: "mkdir -p ${config.paths.output}"
   ```

2. **Cascading Failures**
   ```yaml
   steps:
     - shell: "command1"
       on_failure:
         - shell: "recovery1"
           on_failure:
             - shell: "recovery2"
   ```

3. **Dynamic Command Generation**
   ```yaml
   steps:
     - shell: "ls *.txt"
       capture_output: files
     - foreach: "${files}"
       steps:
         - claude: "/process ${item}"
   ```

### Mock Configuration

```rust
impl MockClaudeExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_response(mut self, cmd: &str, response: &str) -> Self {
        self.responses.insert(cmd.to_string(), response.to_string());
        self
    }

    pub fn failing(mut self) -> Self {
        self.should_fail = true;
        self
    }
}
```

## Migration and Compatibility

Tests are additive only; no changes to existing functionality required.