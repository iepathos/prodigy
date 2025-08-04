---
number: 49
title: Test and Debug Loop for Workflows
category: testing
priority: high
status: draft
dependencies: []
created: 2025-08-04
---

# Specification 49: Test and Debug Loop for Workflows

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

When MMM runs coverage improvement workflows, the implemented changes sometimes introduce bugs that cause tests to fail. Currently, when tests fail after implementation, the workflow either continues with broken tests or requires manual intervention. This leads to merged code that doesn't pass tests, degrading code quality.

We need an automated way to detect test failures and attempt to fix them before proceeding with the workflow. This should be configurable in the workflow YAML files with a simple, clean syntax.

## Objective

Add support for automatic test debugging loops in MMM workflows, allowing Claude to fix failing tests with a configurable number of retry attempts before continuing or failing the workflow.

## Requirements

### Functional Requirements
- Support `test:` command in workflow YAML with configurable test execution
- Detect test failures and trigger Claude debug commands automatically
- Pass test output, original spec, and attempt count to debug command
- Retry up to a configurable maximum number of attempts
- Stop retrying when tests pass (exit code 0)
- Continue workflow after successful fix or max attempts reached

### Non-Functional Requirements
- Clean, intuitive YAML syntax that follows existing patterns
- Minimal performance overhead when tests pass on first attempt
- Clear logging of debug attempts and outcomes
- Preserve all test output for debugging

## Acceptance Criteria

- [ ] Workflow YAML supports `test:` command with `on_failure:` block
- [ ] Test failures trigger Claude debug command automatically
- [ ] Debug command receives spec path, test output, and attempt number
- [ ] Tests are re-run after each debug attempt
- [ ] Loop stops when tests pass or max attempts reached
- [ ] Workflow continues or fails based on configuration
- [ ] Clear logging shows each debug attempt and outcome
- [ ] Integration test demonstrates full debug loop functionality

## Technical Details

### Implementation Approach

1. **YAML Schema Extension**:
   ```yaml
   - test:
       command: "cargo test"  # or any test command
       on_failure:
         claude: "/mmm-debug-test-failure --spec ${coverage.spec} --output ${test.output}"
         max_attempts: 3
         stop_on_success: true  # default: true
         fail_workflow: false   # default: false, if true workflow aborts after max attempts
   ```

2. **Workflow Engine Changes**:
   - Add new `TestCommand` type to workflow command enum
   - Implement test execution with output capture
   - Add retry loop logic with Claude command invocation
   - Pass structured context to Claude commands

3. **Variable Interpolation**:
   - `${test.output}`: Full stdout/stderr from test command
   - `${test.exit_code}`: Exit code from test command
   - `${test.attempt}`: Current attempt number (1-based)
   - `${coverage.spec}` or other workflow variables from previous steps

### Architecture Changes

1. **New Components**:
   - `TestCommand` struct in workflow module
   - `TestDebugConfig` for retry configuration
   - Test output capture and parsing utilities

2. **Modified Components**:
   - Workflow parser to recognize `test:` commands
   - Workflow executor to handle retry loops
   - Variable interpolation to support test-specific variables

### Data Structures

```rust
pub struct TestCommand {
    pub command: String,
    pub on_failure: Option<TestDebugConfig>,
}

pub struct TestDebugConfig {
    pub claude: String,  // Claude command with variables
    pub max_attempts: u32,
    pub stop_on_success: bool,
    pub fail_workflow: bool,
}

pub struct TestResult {
    pub exit_code: i32,
    pub output: String,  // Combined stdout/stderr
    pub duration: Duration,
}
```

### APIs and Interfaces

The Claude debug command will receive:
- `--spec`: Path to the original specification file
- `--output`: Full test output (may be provided via temp file if too large)
- `--attempt`: Current attempt number
- `--exit-code`: Test command exit code

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - Workflow parser and executor
  - Claude command system
  - Variable interpolation engine
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Test command parsing from YAML
  - Retry logic with mock commands
  - Variable interpolation for test variables
  
- **Integration Tests**: 
  - Full workflow with intentionally failing tests
  - Verify debug command receives correct parameters
  - Test max attempts and success conditions
  
- **Performance Tests**: 
  - Ensure minimal overhead for passing tests
  - Measure debug loop performance
  
- **User Acceptance**: 
  - Run coverage workflow with introduced bugs
  - Verify automated fixing works as expected

## Documentation Requirements

- **Code Documentation**: 
  - Document new TestCommand structures
  - Add examples to workflow module
  
- **User Documentation**: 
  - Update workflow examples to show test commands
  - Document available test variables
  - Add troubleshooting guide for debug loops
  
- **Architecture Updates**: 
  - Add test-debug loop to workflow architecture diagram

## Implementation Notes

1. **Error Handling**:
   - Gracefully handle missing Claude commands
   - Clear error messages for invalid configurations
   - Preserve test output even on catastrophic failures

2. **Performance Considerations**:
   - Stream test output to avoid memory issues
   - Use temporary files for very large outputs
   - Implement timeouts for hung tests

3. **Debugging Support**:
   - Option to save all debug attempts for analysis
   - Verbose logging mode for troubleshooting
   - Clear indication of why loop terminated

## Migration and Compatibility

- Fully backward compatible - existing workflows continue to work
- New `test:` command is optional
- Can gradually migrate existing `run:` test commands to `test:` format
- No breaking changes to workflow schema

## Example Workflows

### Basic Test with Debug
```yaml
commands:
  - claude: "/mmm-implement-spec ${spec}"
  
  - test:
      command: "cargo test"
      on_failure:
        claude: "/mmm-debug-test-failure --spec ${spec} --output ${test.output}"
        max_attempts: 3
```

### Complex Test Scenarios
```yaml
commands:
  - test:
      command: "cargo test --features integration"
      on_failure:
        claude: "/mmm-debug-test-failure --spec ${spec} --output ${test.output} --mode integration"
        max_attempts: 5
        fail_workflow: true  # Abort if can't fix after 5 attempts
        
  - test:
      command: "cargo test --doc"
      on_failure:
        claude: "/mmm-fix-doc-tests --output ${test.output}"
        max_attempts: 2
        fail_workflow: false  # Continue even if doc tests fail
```

### Coverage Workflow Update
```yaml
# Test coverage improvement workflow
commands:
    - claude: "/mmm-coverage"
      id: coverage
      outputs:
        spec:
          file_pattern: "*-coverage-improvements.md"
      analysis:
        max_cache_age: 300
    
    - claude: "/mmm-implement-spec ${coverage.spec}"
    
    - test:
        command: "cargo test"
        on_failure:
          claude: "/mmm-debug-test-failure --spec ${coverage.spec} --output ${test.output}"
          max_attempts: 3
          stop_on_success: true
    
    - claude: "/mmm-lint"
      commit_required: false
```