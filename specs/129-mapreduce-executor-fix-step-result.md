---
number: 129
title: MapReduce Executor - Fix StepResult Type Mismatch
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-10-11
---

# Specification 129: MapReduce Executor - Fix StepResult Type Mismatch

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The MapReduce executor (`src/cook/execution/mapreduce/coordination/executor.rs`) defines its own local `StepResult` struct that lacks the `json_log_location` field present in the canonical `StepResult` from `src/cook/workflow/executor.rs`. This causes Claude JSON log paths to not be displayed when setup phase commands fail, making debugging extremely difficult.

### Root Cause

The file defines a duplicate `StepResult` struct at line 37-42:

```rust
struct StepResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}
```

This shadows the proper `StepResult` from the workflow module, which includes:

```rust
pub struct StepResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub json_log_location: Option<String>,  // MISSING in MapReduce version
}
```

### Impact

1. Claude commands executed in setup phase don't display log file locations
2. Debugging failed workflows requires manually finding log files in `~/.claude/projects/`
3. Violates DRY principle with duplicate type definitions
4. Type mismatch between string/Option<String> for stdout/stderr

## Objective

Replace the MapReduce executor's local `StepResult` definition with the canonical type from the workflow module, ensuring Claude log locations are properly captured and displayed.

## Requirements

### Functional Requirements

- Remove duplicate `StepResult` struct definition (lines 35-42)
- Import `StepResult` from `crate::cook::workflow` module
- Update all `StepResult` construction sites to use non-Option types for stdout/stderr
- Preserve `json_log_location` from `ExecutionResult` when constructing `StepResult`
- Maintain backward compatibility with existing error handling

### Non-Functional Requirements

- Zero behavior changes except log path display
- All existing tests must pass unchanged
- No performance impact
- Changes limited to single file

## Acceptance Criteria

- [ ] Local `StepResult` struct removed from executor.rs
- [ ] `StepResult` properly imported from workflow module
- [ ] All Claude command executions capture `json_log_location`
- [ ] Setup phase failures display Claude log path
- [ ] Shell command executions set `json_log_location: None`
- [ ] write_file command executions preserve `json_log_location`
- [ ] All existing tests pass without modification
- [ ] Manual test: Failed setup command displays log path

## Technical Details

### Implementation Changes

**1. Update imports** (line 25):

```rust
// Before
use crate::cook::workflow::{OnFailureConfig, WorkflowStep};

// After
use crate::cook::workflow::{OnFailureConfig, WorkflowStep, StepResult};
```

**2. Remove duplicate struct** (lines 35-42):

Delete the entire local StepResult definition.

**3. Update shell command execution** (~line 415):

```rust
// Before
Ok(StepResult {
    success: exit_code == 0,
    exit_code: Some(exit_code),
    stdout: Some(output.stdout),
    stderr: Some(output.stderr),
})

// After
Ok(StepResult {
    success: exit_code == 0,
    exit_code: Some(exit_code),
    stdout: output.stdout,
    stderr: output.stderr,
    json_log_location: None,
})
```

**4. Update Claude command execution** (~line 432):

```rust
// Before
Ok(StepResult {
    success: result.success,
    exit_code: result.exit_code,
    stdout: Some(result.stdout),
    stderr: Some(result.stderr),
})

// After
Ok(StepResult {
    success: result.success,
    exit_code: result.exit_code,
    stdout: result.stdout,
    stderr: result.stderr,
    json_log_location: result.json_log_location().map(|s| s.to_string()),
})
```

**5. Update write_file execution** (~line 450):

```rust
// Before
Ok(StepResult {
    success: result.success,
    exit_code: result.exit_code,
    stdout: Some(result.stdout),
    stderr: Some(result.stderr),
})

// After
Ok(StepResult {
    success: result.success,
    exit_code: result.exit_code,
    stdout: result.stdout,
    stderr: result.stderr,
    json_log_location: result.json_log_location,
})
```

**6. Update agent execution Claude command** (~line 1028):

```rust
// Before
Ok(StepResult {
    success: result.success,
    exit_code: result.exit_code,
    stdout: Some(result.stdout),
    stderr: Some(result.stderr),
})

// After
Ok(StepResult {
    success: result.success,
    exit_code: result.exit_code,
    stdout: result.stdout,
    stderr: result.stderr,
    json_log_location: result.json_log_location().map(|s| s.to_string()),
})
```

**7. Update agent execution shell command** (~line 1063):

```rust
// Before
Ok(StepResult {
    success: exit_code == 0,
    exit_code: Some(exit_code),
    stdout: Some(output.stdout),
    stderr: Some(output.stderr),
})

// After
Ok(StepResult {
    success: exit_code == 0,
    exit_code: Some(exit_code),
    stdout: output.stdout,
    stderr: output.stderr,
    json_log_location: None,
})
```

**8. Update agent write_file execution** (~line 1114):

```rust
// Before
Ok(StepResult {
    success: result.success,
    exit_code: result.exit_code,
    stdout: Some(result.stdout),
    stderr: Some(result.stderr),
})

// After
Ok(StepResult {
    success: result.success,
    exit_code: result.exit_code,
    stdout: result.stdout,
    stderr: result.stderr,
    json_log_location: result.json_log_location,
})
```

### Files Modified

- `src/cook/execution/mapreduce/coordination/executor.rs` - Only file that needs changes

### Testing Strategy

**Unit Tests** (existing tests should pass):
- All MapReduce coordinator tests
- Setup phase execution tests
- Agent execution tests

**Integration Tests**:
- Run failing workflow with setup phase
- Verify Claude log path is displayed in error output
- Confirm log file exists at displayed path

**Manual Verification**:

```bash
# Create a test workflow that fails in setup
cat > test-setup-fail.yml <<'EOF'
name: test-setup-fail
mode: mapreduce

setup:
  - claude: "/nonexistent-command"

map:
  input: "items.json"
  agent_template:
    - shell: "echo 'should not reach here'"
EOF

# Run the workflow
prodigy run test-setup-fail.yml

# Expected output should include:
# ❌ Setup phase failed: ...
# 📝 Claude log: /Users/.../.claude/projects/.../session-xyz.json
```

## Dependencies

**Prerequisites**: None

**Affected Components**:
- MapReduce coordinator
- Setup phase executor
- Agent execution logic

**External Dependencies**: None

## Implementation Notes

### Why This Fix is Safe

1. **Minimal scope**: Only touches type definition and construction sites
2. **No logic changes**: Same values, just different field names
3. **Type safety**: Compiler catches all conversion sites
4. **Backward compatible**: `json_log_location` is optional, so existing code tolerates None

### Potential Issues

1. **Code that matches on stdout/stderr**: Need to update pattern matching if any code does `if let Some(stdout) = result.stdout`
   - **Mitigation**: Search codebase for such patterns
   - **Expected**: None found in current codebase

2. **Serialization/deserialization**: If `StepResult` is serialized, format changes
   - **Mitigation**: `StepResult` is not serialized in current code
   - **Expected**: No impact

## Migration and Compatibility

### Breaking Changes

None - This is a bug fix that restores expected behavior.

### Rollback Plan

If issues arise, the change can be trivially reverted as it's contained to a single file with clear git history.

### Deployment

Can be deployed immediately with no coordination required. The fix is self-contained and doesn't affect any APIs or stored data.

## Success Metrics

- Claude log paths appear in all setup phase error messages
- Zero test failures after implementation
- Zero performance regression
- Reduced debugging time for MapReduce failures (subjective improvement)

## Documentation Requirements

### Code Documentation

- No documentation changes needed - the bug fix makes behavior match documented expectations

### User Documentation

- Update troubleshooting guide to reflect that Claude logs are now always shown
- Update MapReduce documentation to mention log file locations in error messages

### Architecture Updates

- No architecture changes - this fixes a deviation from the architecture
