---
number: 58
title: Command Timeout Configuration
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-09
---

# Specification 58: Command Timeout Configuration

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy workflows can execute long-running Claude and shell commands that sometimes hang indefinitely, causing workflow execution to stall. This is particularly problematic in automated environments and CI/CD pipelines where hanging processes can block entire build queues and consume resources unnecessarily. 

While the underlying execution infrastructure already supports timeouts through the `ExecutionContext` struct, there is currently no way to specify timeout values in workflow YAML files. Users need the ability to set command-specific timeouts to ensure workflows complete predictably and failed commands can be retried or handled appropriately.

## Objective

Enable users to specify timeout values in seconds for individual workflow commands (shell and claude) through a simple `timeout:` field in workflow YAML files. Commands that exceed the specified timeout will be terminated and treated as failures, allowing on_failure handlers and workflow error recovery to proceed normally.

## Requirements

### Functional Requirements

#### Workflow Configuration
- Support `timeout:` field for shell commands in workflow YAML
- Support `timeout:` field for claude commands in workflow YAML
- Accept timeout values as positive integers representing seconds
- Apply timeout to the entire command execution including startup time
- Default to no timeout (unlimited execution time) when not specified

#### Command Execution
- Terminate commands that exceed the specified timeout duration
- Return appropriate error status when timeout occurs
- Include timeout information in execution logs and error messages
- Preserve partial output from timed-out commands for debugging
- Trigger on_failure handlers when commands timeout

#### Error Handling
- Treat timeout as a command failure with distinct error code
- Include timeout duration and elapsed time in error messages
- Support retry mechanisms for timed-out commands
- Allow workflow to continue or fail based on existing error handling configuration

### Non-Functional Requirements

#### Performance
- Minimal overhead for timeout monitoring (<1% CPU usage)
- Efficient process termination without resource leaks
- No impact on commands that complete within timeout

#### Compatibility
- Backward compatible with existing workflows (no timeout = unlimited)
- Compatible with all command types (shell, claude, test)
- Works across all supported platforms (Linux, macOS, Windows)

## Acceptance Criteria

- [ ] Shell commands accept `timeout:` field in workflow YAML
- [ ] Claude commands accept `timeout:` field in workflow YAML
- [ ] Commands are terminated after specified timeout duration
- [ ] Timeout errors trigger on_failure handlers appropriately
- [ ] Error messages clearly indicate timeout occurred with duration
- [ ] Partial output is preserved from timed-out commands
- [ ] Existing workflows without timeout continue to work unchanged
- [ ] Integration tests verify timeout behavior for both command types
- [ ] Documentation updated with timeout configuration examples

## Technical Details

### Implementation Approach

1. **Configuration Parsing**
   - Add `timeout: Option<u64>` field to `WorkflowStepCommand` struct
   - Update YAML deserializer to parse timeout values
   - Validate timeout is positive integer during workflow validation

2. **Command Execution**
   - Pass timeout from workflow config to `ExecutionContext`
   - Use existing `timeout_seconds` field in execution layer
   - Ensure subprocess manager respects timeout configuration

3. **Error Handling**
   - Define timeout-specific error variant in error types
   - Include timeout details in error messages and logs
   - Ensure proper cleanup of terminated processes

### Data Structures

```rust
// In src/config/command.rs
pub struct WorkflowStepCommand {
    // ... existing fields ...
    
    /// Timeout in seconds for command execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}
```

### YAML Schema

```yaml
# Example workflow with timeouts
commands:
  - shell: "npm test"
    timeout: 300  # 5 minutes
    on_failure:
      claude: "/fix-tests"
      
  - claude: "/implement feature"
    timeout: 600  # 10 minutes
    
  - shell: "cargo build --release"
    timeout: 900  # 15 minutes
```

### Error Messages

```
Error: Command timed out after 300 seconds
Command: npm test
Elapsed: 300.5s
Partial output preserved in logs
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/config/command.rs` - Command configuration structures
  - `src/cook/execution/bridge.rs` - Command execution bridge
  - `src/cook/workflow/executor.rs` - Workflow executor
  - `src/subprocess/` - Subprocess management layer
- **External Dependencies**: None (uses std library timeout features)

## Testing Strategy

### Unit Tests
- Parse timeout values from YAML configurations
- Validate timeout field constraints
- Verify timeout propagation to execution context
- Test error creation for timeout scenarios

### Integration Tests
- Shell command terminates after timeout
- Claude command terminates after timeout
- on_failure handler triggered for timeout
- Partial output captured from timed-out command
- Workflow continues/fails based on error configuration
- No timeout means unlimited execution time

### Performance Tests
- Verify minimal overhead for timeout monitoring
- Test resource cleanup after timeout termination
- Validate no memory leaks from terminated processes

### User Acceptance
- Test common timeout scenarios in real workflows
- Verify timeout works in MapReduce parallel execution
- Validate timeout behavior in CI/CD environments

## Documentation Requirements

### Code Documentation
- Document timeout field in WorkflowStepCommand struct
- Add inline comments explaining timeout behavior
- Include timeout examples in command handler documentation

### User Documentation
- Add timeout section to workflow configuration guide
- Provide examples of common timeout values
- Document timeout error messages and recovery
- Include best practices for setting appropriate timeouts

### Architecture Updates
- Update workflow execution flow diagrams
- Document timeout handling in error recovery section
- Add timeout to command lifecycle documentation

## Implementation Notes

### Timeout Selection Guidelines
- Interactive commands: 30-60 seconds
- Build commands: 5-15 minutes  
- Test suites: 10-30 minutes
- Claude commands: 5-10 minutes
- Long-running operations: 30-60 minutes

### Platform Considerations
- Use tokio timeout for async operations
- Ensure SIGTERM followed by SIGKILL on Unix
- Use process termination API on Windows
- Handle partial output buffering correctly

### Error Recovery
- Timeouts should be retryable by default
- Consider exponential backoff for timeout retries
- Log timeout patterns for workflow optimization
- Support timeout adjustment in retry attempts

## Migration and Compatibility

### Backward Compatibility
- No changes required for existing workflows
- Default behavior (no timeout) remains unchanged
- Timeout is opt-in per command

### Migration Path
1. Deploy timeout support in prodigy
2. Update documentation with examples
3. Gradually add timeouts to critical workflows
4. Monitor timeout patterns and adjust values
5. Consider adding workflow-level default timeout

### Future Enhancements
- Support for timeout units (30s, 5m, 1h)
- Workflow-level default timeout configuration
- Dynamic timeout based on system load
- Timeout profiles for different environments
- Integration with monitoring/alerting systems