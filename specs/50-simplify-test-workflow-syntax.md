---
number: 50
title: Simplify Test Workflow Syntax
category: optimization
priority: medium
status: draft
dependencies: []
created: 2025-01-06
---

# Specification 50: Simplify Test Workflow Syntax

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: none

## Context

The current workflow configuration supports a special `test:` command type that wraps shell commands for test execution. This adds an unnecessary layer of abstraction since test commands are simply shell commands with optional error handling. The `test:` wrapper creates confusion and inconsistency in the workflow syntax.

Currently, test commands are defined as:
```yaml
- test:
    command: "just test"
    on_failure:
      claude: "/mmm-debug-test-failure --spec $ARG --output ${test.output}"
      max_attempts: 3
      fail_workflow: false
```

This can be simplified to use the existing `shell:` command type with the same `on_failure` capabilities.

## Objective

Remove the `test:` command type from the workflow system and update all examples to use `shell:` commands directly. This simplifies the workflow syntax, reduces code complexity, and makes the system more consistent.

## Requirements

### Functional Requirements
- Remove support for `test:` command type from the workflow executor
- Update all example workflow files to use `shell:` instead of `test:`
- Ensure `on_failure` blocks work identically with `shell:` commands
- Maintain backward compatibility by gracefully handling old `test:` syntax with deprecation warning

### Non-Functional Requirements
- No regression in test execution functionality
- Clear error messages for deprecated syntax
- Consistent command display format (shell: prefix)
- Minimal disruption to existing workflows

## Acceptance Criteria

- [ ] The `test:` command type is removed from `WorkflowStepCommand` struct
- [ ] The `TestCommand` struct and `TestDebugConfig` are deprecated or removed
- [ ] The workflow executor properly handles shell commands with `on_failure` blocks
- [ ] All example files in `examples/` directory use `shell:` instead of `test:`
- [ ] Output variables like `${shell.output}` work correctly in `on_failure` blocks
- [ ] The command display shows "shell: just test" instead of "test: just test"
- [ ] Existing workflows with `test:` syntax show a deprecation warning but continue to work
- [ ] All unit tests and integration tests pass
- [ ] Documentation is updated to reflect the new syntax

## Technical Details

### Implementation Approach

1. **Update WorkflowStepCommand struct**:
   - Remove the `test: Option<TestCommand>` field
   - Update deserialization to handle legacy test commands

2. **Modify CommandType enum**:
   - Remove `Test(TestCommand)` variant
   - Update command type determination logic

3. **Update Workflow Executor**:
   - Remove `execute_test_command` method
   - Ensure shell commands support all test features (retry, output capture)

4. **Transform Example Files**:
   - Convert all `test:` blocks to `shell:` commands
   - Preserve all `on_failure` configurations

5. **Add Deprecation Handling**:
   - Detect old `test:` syntax during parsing
   - Convert to `shell:` internally
   - Display deprecation warning

### Architecture Changes

The workflow command parsing and execution flow will be simplified:
- Before: `test:` → `TestCommand` → `CommandType::Test` → `execute_test_command`
- After: `shell:` → `CommandType::Shell` → `execute_shell_command`

### Data Structures

Remove or deprecate:
```rust
pub struct TestCommand {
    pub command: String,
    pub on_failure: Option<TestDebugConfig>,
}

pub struct TestDebugConfig {
    pub claude: Option<String>,
    pub max_attempts: u32,
    pub fail_workflow: bool,
    pub commit_required: bool,
}
```

### APIs and Interfaces

The workflow YAML syntax changes from:
```yaml
- test:
    command: "just test"
    on_failure: ...
```

To:
```yaml
- shell: "just test"
  on_failure: ...
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/config/command.rs` - Command structures
  - `src/cook/workflow/executor.rs` - Workflow execution logic
  - `src/cook/orchestrator.rs` - Command display logic
  - All example workflow files in `examples/`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test shell command execution with retry logic
  - Test output variable substitution in on_failure blocks
  - Test deprecation warning for old syntax

- **Integration Tests**:
  - Run all example workflows to ensure they work correctly
  - Test failure scenarios with retry attempts
  - Verify output capture and variable substitution

- **Regression Tests**:
  - Ensure existing test execution behavior is preserved
  - Verify command display format is correct

- **User Acceptance**:
  - All example workflows execute successfully
  - Clear deprecation messages for old syntax

## Documentation Requirements

- **Code Documentation**:
  - Update inline comments explaining the simplified syntax
  - Document the deprecation path for test commands

- **User Documentation**:
  - Update workflow configuration documentation
  - Provide migration guide from test: to shell:
  - Update all example file comments

- **Architecture Updates**:
  - Update workflow execution flow diagrams
  - Document the simplified command type hierarchy

## Implementation Notes

### Migration Path
1. First, add support for `on_failure` to shell commands
2. Add deprecation warning when parsing `test:` commands
3. Internally convert `test:` to `shell:` during parsing
4. Update all examples to use new syntax
5. In future version, remove test command support entirely

### Variable Substitution
Ensure that `${shell.output}` works in `on_failure` blocks just like `${test.output}` did. This requires the shell command executor to capture output properly.

### Retry Logic
The retry logic currently in `execute_test_command` should be generalized and made available to all command types through the `on_failure` mechanism.

## Migration and Compatibility

### Breaking Changes
- The `test:` command syntax will be deprecated
- Variable names change from `${test.output}` to `${shell.output}`

### Migration Steps
1. Show deprecation warning for 2-3 versions
2. Automatically convert test: to shell: internally
3. Provide automated migration tool if needed
4. Eventually remove test: support entirely

### Compatibility Considerations
- Old workflows will continue to work with deprecation warnings
- The conversion should be transparent to users
- No functional changes to test execution behavior

## Example Transformation

Before:
```yaml
- test:
    command: "just test"
    on_failure:
      claude: "/mmm-debug-test-failure --spec $ARG --output ${test.output}"
      max_attempts: 3
      fail_workflow: false
```

After:
```yaml
- shell: "just test"
  on_failure:
    claude: "/mmm-debug-test-failure --spec $ARG --output ${shell.output}"
    max_attempts: 3
    fail_workflow: false
```

The functionality remains identical, but the syntax is simpler and more consistent with other command types.