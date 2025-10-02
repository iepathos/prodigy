---
number: 115
title: Improve Logging Clarity and Reduce Redundancy
category: optimization
priority: medium
status: draft
dependencies: []
created: 2025-10-02
---

# Specification 115: Improve Logging Clarity and Reduce Redundancy

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently has redundant and overly verbose logging at the INFO level that clutters the workflow execution output. This makes it harder for users to follow workflow progress and understand what's happening.

### Current Issues

1. **Duplicate execution messages**:
   ```
   🔄 Executing step 4/6: claude: /prodigy-debtmap-implement --plan .prodigy/IMPLEMENTATION_PLAN.md
   🔄 Executing: claude: /prodigy-debtmap-implement --plan .prodigy/IMPLEMENTATION_PLAN.md
   ```
   The same information is displayed twice.

2. **Verbose context dumping at INFO level**:
   ```
   2025-10-02T15:41:13.445841Z  INFO === Step Execution Context ===
   2025-10-02T15:41:13.445846Z  INFO Step: claude: /prodigy-debtmap-implement --plan .prodigy/IMPLEMENTATION_PLAN.md
   2025-10-02T15:41:13.445848Z  INFO Working Directory: /Users/glen/.prodigy/worktrees/prodigy/session-8d9f4d9d-6cf0-4343-a548-56ffda60d219
   2025-10-02T15:41:13.445851Z  INFO Project Directory: /Users/glen/memento-mori/prodigy
   2025-10-02T15:41:13.445853Z  INFO Worktree: session-8d9f4d9d-6cf0-4343-a548-56ffda60d219
   2025-10-02T15:41:13.445854Z  INFO Session ID: session-8d9f4d9d-6cf0-4343-a548-56ffda60d219
   2025-10-02T15:41:13.445856Z  INFO Variables:
   2025-10-02T15:41:13.445857Z  INFO   PROJECT_ROOT = /Users/glen/.prodigy/worktrees/prodigy/session-8d9f4d9d-6cf0-4343-a548-56ffda60d219
   2025-10-02T15:41:13.445860Z  INFO Captured Outputs:
   2025-10-02T15:41:13.445861Z  INFO   claude.output = {"type":"system","subtype":"init","cwd":"/Users/glen/.prodigy/worktrees/prodigy/session-8d9f4d9d-6cf... (truncated)
   2025-10-02T15:41:13.445864Z  INFO   CAPTURED_OUTPUT = {"type":"system","subtype":"init","cwd":"/Users/glen/.prodigy/worktrees/prodigy/session-8d9f4d9d-6cf... (truncated)
   2025-10-02T15:41:13.454803Z  INFO Environment Variables:
   2025-10-02T15:41:13.454814Z  INFO   PRODIGY_CLAUDE_STREAMING = true
   2025-10-02T15:41:13.454816Z  INFO   PRODIGY_AUTOMATION = true
   2025-10-02T15:41:13.454818Z  INFO Actual execution directory: /Users/glen/.prodigy/worktrees/prodigy/session-8d9f4d9d-6cf0-4343-a548-56ffda60d219
   2025-10-02T15:41:13.454820Z  INFO ==============================
   ```
   This detailed context is useful for debugging but shouldn't be shown at INFO level by default.

### Impact

- Cluttered output makes it hard to follow workflow progress
- Important status updates get buried in verbose logging
- Users can't easily see "what's happening now"
- Debugging information pollutes normal operation logs

## Objective

Improve Prodigy's logging to provide clean, actionable output at INFO level while preserving detailed context information at DEBUG/TRACE levels accessible via verbosity flags.

## Requirements

### Functional Requirements

1. **Eliminate duplicate execution messages**
   - Display step execution message once, not twice
   - Keep the most informative version (with step number and full command)
   - Remove redundant "Executing: ..." line

2. **Move verbose context to DEBUG level**
   - Step execution context (working directory, session ID, etc.) → DEBUG
   - Environment variables → DEBUG
   - Variable interpolation details → DEBUG
   - Captured output previews → DEBUG

3. **Clean INFO level output**
   - Show only essential progress information:
     - Step number and total (e.g., "step 4/6")
     - Command type and key parameters
     - Success/failure status
     - Important milestones (iteration complete, validation passed, etc.)

4. **Preserve debug capabilities**
   - All current logging available at DEBUG level (-v flag)
   - Trace-level logging for deep debugging (-vv flag)
   - No information should be lost, just moved to appropriate levels

### Non-Functional Requirements

1. **Backward Compatibility**
   - Existing verbosity flags continue to work (-v, -vv, -vvv)
   - Users can still access all information they previously could

2. **Performance**
   - Logging changes should not impact execution performance
   - String formatting should be lazy (only when needed)

3. **Maintainability**
   - Clear logging level guidelines for developers
   - Consistent use of log levels across codebase

## Acceptance Criteria

- [ ] Step execution is logged once, not twice
- [ ] Step execution context (=== Step Execution Context ===) appears only at DEBUG level
- [ ] Environment variables logged only at DEBUG level
- [ ] Variable interpolation details logged only at DEBUG level
- [ ] Captured output previews logged only at DEBUG level
- [ ] INFO level shows clean progress: step number, command, status
- [ ] All previous information still accessible with -v flag
- [ ] Trace-level context available with -vv flag
- [ ] No regressions in existing test suite
- [ ] Updated documentation reflects new logging behavior

## Technical Details

### Implementation Approach

1. **Identify logging call sites**
   - Search for duplicate "Executing" messages in orchestrator/executor code
   - Locate verbose context logging (Step Execution Context blocks)
   - Find environment variable and variable logging

2. **Adjust log levels**
   - Change context dumping from `info!()` to `debug!()`
   - Move environment variable logging to `debug!()`
   - Move variable interpolation details to `debug!()`
   - Keep step progress at `info!()` level

3. **Consolidate execution messages**
   - Keep single "Executing step N/M: <command>" message
   - Remove duplicate "Executing: <command>" message
   - Ensure step number, total, and full command are visible

### Files to Modify

Primary locations based on log message format:

1. **src/cook/orchestrator.rs** or **src/cook/workflow/executor.rs**
   - Contains "Executing step N/M" messages
   - Contains "=== Step Execution Context ===" logging

2. **Variable interpolation logging**
   - Likely in interpolation engine or variable context code
   - Change from `info!()` to `debug!()`

3. **Environment variable logging**
   - Search for "Environment Variables:" logging
   - Move to `debug!()` level

### Log Level Guidelines

**INFO Level** (default, no flags):
- Workflow/iteration start/complete
- Step execution: "🔄 Executing step N/M: <command>"
- Step completion: "✅ Step N/M completed"
- Validation results: "✅ Validation passed" or "⚠️ Validation incomplete"
- Important milestones: "📊 Total workflow time: ..."

**DEBUG Level** (-v flag):
- Step execution context (directories, session IDs)
- Environment variables
- Variable values and interpolation
- Captured output previews
- Checkpoint saves

**TRACE Level** (-vv flag):
- Detailed execution flow
- Internal state transitions
- Low-level debugging information

### Example Refactoring

**Before**:
```rust
info!("🔄 Executing step {}/{}: {}", step_num, total, command);
// ... some code ...
info!("🔄 Executing: {}", command);
info!("=== Step Execution Context ===");
info!("Step: {}", command);
info!("Working Directory: {}", working_dir);
info!("Session ID: {}", session_id);
// ... more context ...
```

**After**:
```rust
info!("🔄 Executing step {}/{}: {}", step_num, total, command);
debug!("=== Step Execution Context ===");
debug!("Working Directory: {}", working_dir);
debug!("Session ID: {}", session_id);
// ... more context at debug level ...
```

## Dependencies

**Prerequisites**: None

**Affected Components**:
- Orchestrator/workflow executor
- Variable interpolation engine
- Command execution handlers
- Test output validation (tests may check log output)

**External Dependencies**: None

## Testing Strategy

### Unit Tests
- Verify log messages appear at correct levels
- Test that INFO level is clean and concise
- Test that DEBUG level includes all context

### Integration Tests
- Run workflows with default verbosity (no flags)
- Verify output is clean and easy to follow
- Run workflows with -v flag
- Verify all context information is still available

### Manual Testing
- Run `prodigy run` without flags → Check clean output
- Run `prodigy run -v` → Check DEBUG context appears
- Run `prodigy run -vv` → Check TRACE information appears
- Verify no duplicate "Executing" messages

## Documentation Requirements

### Code Documentation
- Add comments explaining log level choices
- Document logging guidelines for contributors

### User Documentation
- Update CLAUDE.md to reflect new logging behavior
- Document verbosity flags and what they show:
  - Default: Clean progress output
  - `-v`: Includes execution context
  - `-vv`: Includes trace-level debugging

### Architecture Updates
- No ARCHITECTURE.md updates needed
- This is a refinement, not architectural change

## Implementation Notes

### Search Patterns

Find duplicate execution logging:
```bash
rg "Executing step" -A 5
rg "Executing:"
```

Find verbose context logging:
```bash
rg "=== Step Execution Context ===" -A 10
rg "INFO.*Working Directory"
rg "INFO.*Environment Variables"
```

### Testing Impact

Some tests may validate specific log output. Update these tests to:
- Use appropriate verbosity flags in tests
- Check for messages at correct log levels
- Verify DEBUG messages with RUST_LOG=debug

### Rollback Plan

If issues arise:
- Git revert is straightforward (logging changes are isolated)
- No database or state changes involved
- Tests will catch regressions

## Migration and Compatibility

**Breaking Changes**: None

**Migration Required**: None

**Compatibility Notes**:
- Users accustomed to verbose output can use `-v` flag
- CI/CD scripts may need `-v` flag if they parse logs
- No changes to workflow file format or behavior

## Success Metrics

**Qualitative**:
- Users report cleaner, easier-to-follow output
- New users can understand workflow progress without documentation

**Quantitative**:
- INFO level log lines reduced by ~70% per step
- Duplicate messages eliminated (0 occurrences)
- All context information still accessible at DEBUG level
