---
number: 106
title: Fix Dry-Run Mode Validation Command Execution
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-19
---

# Specification 106: Fix Dry-Run Mode Validation Command Execution

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Currently, when running Prodigy workflows in dry-run mode (`--dry-run`), the system correctly simulates most command execution without actually running them. However, there's a critical bug where validation commands (particularly Claude validation commands) are still being executed even in dry-run mode. This defeats the purpose of dry-run, which should be a completely safe preview of what would happen without any actual execution.

The issue manifests when running commands like:
```bash
prodigy cook workflows/implement.yml -wy --args 105 --dry-run
```

The output shows:
- `[DRY RUN] Would execute: claude: /prodigy-implement-spec $ARG` (correct)
- `[DRY RUN] Would run validation (Claude): /prodigy-validate-spec $ARG` (correct message)
- But then: `🔄 Running validation (Claude): /prodigy-validate-spec 105` (actually executing!)

This indicates that while the dry-run mode is partially working, the validation execution path is not respecting the dry-run flag.

## Objective

Ensure that dry-run mode is completely non-destructive by preventing ALL command execution, including validation commands. When `--dry-run` is enabled, no external commands (shell, Claude, or validation) should be executed - only simulation and preview should occur.

## Requirements

### Functional Requirements

1. **Complete Command Simulation**:
   - All command types must respect dry-run mode
   - Claude commands must not be executed in dry-run
   - Shell commands must not be executed in dry-run
   - Validation commands must not be executed in dry-run
   - Goal-seek operations must be simulated, not executed

2. **Validation Simulation**:
   - In dry-run mode, validation should be simulated with assumed success
   - Display what validation would be performed
   - Show threshold requirements
   - Indicate assumed validation result (pass/fail based on configuration)

3. **Output Consistency**:
   - Dry-run output must clearly indicate simulation mode
   - All simulated actions must be prefixed with `[DRY RUN]`
   - No mixing of actual execution with dry-run messages

4. **Workflow Flow Preservation**:
   - Dry-run must follow the same logical flow as actual execution
   - Conditional branches should be evaluated based on simulated results
   - Iteration counts and patterns should be preserved

### Non-Functional Requirements

1. **Safety**:
   - Dry-run mode must be 100% safe with zero side effects
   - No file system modifications
   - No network calls
   - No process spawning

2. **Performance**:
   - Dry-run should be faster than actual execution
   - Minimal resource usage
   - Quick feedback for workflow validation

3. **Debugging**:
   - Clear indication of what would happen
   - Preserve all logging and context information
   - Help users understand workflow behavior

## Acceptance Criteria

- [ ] Running `prodigy cook --dry-run` executes NO external commands
- [ ] Claude validation commands are simulated, not executed
- [ ] Shell validation commands are simulated, not executed
- [ ] All dry-run output is clearly marked with `[DRY RUN]` prefix
- [ ] Validation results are simulated based on workflow configuration
- [ ] Goal-seek operations show iteration simulation without execution
- [ ] MapReduce workflows show work distribution simulation
- [ ] No file system changes occur during dry-run
- [ ] No network requests are made during dry-run
- [ ] Dry-run completes faster than actual execution
- [ ] Integration tests verify dry-run safety

## Technical Details

### Implementation Approach

1. **Validation Execution Path**:
   - Locate validation command execution in workflow executor
   - Add dry-run check before any validation execution
   - Return simulated validation results in dry-run mode

2. **Command Handler Updates**:
   - Update Claude command handler to respect dry-run
   - Update shell command handler to respect dry-run
   - Ensure all command types check dry-run flag

3. **Validation Result Simulation**:
   - Create mock validation results for dry-run
   - Use workflow configuration to determine simulated outcome
   - Preserve validation context for debugging

### Architecture Changes

The fix requires updates to the command execution pipeline:

1. **Workflow Executor** (`src/cook/workflow/executor.rs`):
   - Add dry-run check in validation execution
   - Return simulated results instead of executing

2. **Command Handlers** (`src/commands/handlers/`):
   - Ensure all handlers respect dry-run flag
   - Add simulation logic for each command type

3. **Validation Module**:
   - Create validation simulation functionality
   - Generate appropriate mock results

### Data Structures

```rust
// Validation simulation result
struct SimulatedValidation {
    command: String,
    threshold: f64,
    simulated_score: f64,
    would_pass: bool,
    dry_run: bool,
}
```

### APIs and Interfaces

No external API changes. Internal changes:
- Add `dry_run` parameter propagation through execution pipeline
- Update validation interfaces to support simulation
- Ensure all command handlers have dry-run awareness

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - Workflow executor (`src/cook/workflow/executor.rs`)
  - Command handlers (`src/commands/handlers/*.rs`)
  - Validation module
  - Integration tests
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test dry-run flag propagation
  - Test validation simulation logic
  - Test each command handler's dry-run behavior
  - Verify no external execution occurs

- **Integration Tests**:
  - Run complex workflows with dry-run
  - Verify no file system changes
  - Check no process spawning
  - Validate output format consistency
  - Test MapReduce dry-run simulation
  - Test goal-seek dry-run behavior

- **Safety Tests**:
  - Attempt destructive operations in dry-run
  - Verify no actual execution
  - Check for any side effects
  - Monitor system calls during dry-run

- **Performance Tests**:
  - Compare dry-run vs actual execution time
  - Verify dry-run is significantly faster
  - Check resource usage in dry-run mode

## Documentation Requirements

- **Code Documentation**:
  - Document dry-run behavior in each handler
  - Add comments explaining simulation logic
  - Update command handler interfaces

- **User Documentation**:
  - Clarify dry-run behavior in README
  - Add examples of dry-run usage
  - Document what is and isn't simulated

- **Testing Documentation**:
  - Document how to verify dry-run safety
  - Add dry-run test scenarios
  - Include validation simulation examples

## Implementation Notes

1. **Critical Safety Points**:
   - Never trust command type alone - always check dry-run flag
   - Validation is a type of command execution and must be simulated
   - Even "read-only" commands should be simulated in dry-run

2. **Simulation Fidelity**:
   - Simulated results should be realistic
   - Use workflow configuration to guide simulation
   - Preserve enough detail for debugging

3. **Error Handling**:
   - In dry-run, simulate both success and failure paths
   - Show what errors would be handled
   - Indicate recovery strategies that would be attempted

4. **Logging Considerations**:
   - Preserve all logging in dry-run mode
   - Clearly mark simulated vs actual in logs
   - Help users understand execution flow

## Migration and Compatibility

### Breaking Changes
None - this is a bug fix that makes dry-run work as intended.

### Compatibility
- Existing workflows continue to work
- Dry-run behavior becomes more predictable
- No changes to workflow syntax or semantics

### User Communication
- Note in release notes as important bug fix
- Emphasize improved safety of dry-run mode
- Document that validation is now properly simulated