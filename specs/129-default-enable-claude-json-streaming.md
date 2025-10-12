---
number: 129
title: Default Enable Claude JSON Streaming
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-10-12
---

# Specification 129: Default Enable Claude JSON Streaming

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently uses `PRODIGY_CLAUDE_STREAMING` environment variable to control whether Claude commands execute in streaming JSON mode (`--output-format stream-json`) or legacy print mode (`--print`). The current implementation defaults to `"true"` (streaming enabled), but requires explicit checks in multiple locations.

The streaming mode provides critical benefits for workflow auditability:
- Complete JSON logs of all Claude interactions saved to `~/.prodigy/logs/claude-streaming/`
- Real-time observability of Claude tool invocations
- Detailed debugging information for failed MapReduce agents
- Token usage tracking for performance analysis
- Full conversation history for reproducing issues

**Current Problem**: The default behavior is correct (streaming enabled), but the implementation requires explicit environment variable checking with fallback logic scattered across the codebase. This creates maintenance burden and inconsistency risk.

**Desired Behavior**: Streaming should be the permanent default for all Claude executions to ensure workflow auditability. Users should be able to disable streaming via environment variable (`PRODIGY_CLAUDE_STREAMING=false`) if needed for specific scenarios (e.g., CI/CD environments with storage constraints).

## Objective

Simplify the Claude execution mode logic by making streaming the default behavior, eliminating the need for explicit environment variable checks in workflow executors while preserving the ability to disable streaming when necessary.

## Requirements

### Functional Requirements

1. **Default Streaming Behavior**
   - All Claude commands execute in streaming mode (`--output-format stream-json`) by default
   - No environment variable required to enable streaming
   - JSON logs saved to `~/.prodigy/logs/claude-streaming/` for all executions

2. **Opt-Out Mechanism**
   - `PRODIGY_CLAUDE_STREAMING=false` explicitly disables streaming
   - Reverts to legacy `--print` mode when disabled
   - No JSON log files created when disabled

3. **Backward Compatibility**
   - Existing workflows continue to work without modification
   - `PRODIGY_CLAUDE_STREAMING=true` remains valid (no-op, already default)
   - Tests that set the environment variable continue to pass

### Non-Functional Requirements

- **Simplicity**: Reduce code complexity by removing explicit environment variable setting
- **Auditability**: Ensure all Claude executions are logged by default
- **Performance**: No performance impact (streaming is already the current default)
- **Maintainability**: Single source of truth for default behavior

## Acceptance Criteria

- [ ] `claude.rs`: Remove explicit `PRODIGY_CLAUDE_STREAMING` checks, default to streaming mode
- [ ] `context.rs`: Remove code that sets `PRODIGY_CLAUDE_STREAMING=true`
- [ ] `validation.rs`: Remove code that sets `PRODIGY_CLAUDE_STREAMING=true`
- [ ] `command/claude.rs`: Remove code that sets `PRODIGY_CLAUDE_STREAMING=true`
- [ ] All existing tests pass without modification
- [ ] Claude commands execute in streaming mode by default (no env var set)
- [ ] `PRODIGY_CLAUDE_STREAMING=false` disables streaming and uses print mode
- [ ] `PRODIGY_CLAUDE_STREAMING=true` continues to work (treated as default)
- [ ] JSON logs created for all default executions
- [ ] Documentation updated to reflect new default behavior

## Technical Details

### Implementation Approach

**Phase 1: Update Core Execution Logic**

1. Modify `src/cook/execution/claude.rs`:
   ```rust
   // Current logic:
   let streaming_enabled = env_vars
       .get("PRODIGY_CLAUDE_STREAMING")
       .is_some_and(|v| v == "true");

   // New logic:
   let streaming_disabled = env_vars
       .get("PRODIGY_CLAUDE_STREAMING")
       .is_some_and(|v| v == "false");

   if !streaming_disabled {
       // Default: streaming mode
       self.execute_with_streaming(...)
   } else {
       // Explicit opt-out: print mode
       self.execute_with_print(...)
   }
   ```

2. Simplify helper function:
   ```rust
   // Current:
   fn should_use_streaming(env_vars: &HashMap<String, String>) -> bool {
       env_vars
           .get("PRODIGY_CLAUDE_STREAMING")
           .map(|v| v == "true")
           .unwrap_or(false)  // Currently requires explicit "true"
   }

   // New:
   fn should_use_streaming(env_vars: &HashMap<String, String>) -> bool {
       !env_vars
           .get("PRODIGY_CLAUDE_STREAMING")
           .is_some_and(|v| v == "false")  // Default true, only false if explicitly set
   }
   ```

**Phase 2: Remove Redundant Environment Variable Setting**

Remove explicit `PRODIGY_CLAUDE_STREAMING=true` setting from:

1. `src/cook/workflow/executor/context.rs` (line ~388-392):
   ```rust
   // REMOVE THIS BLOCK:
   if std::env::var("PRODIGY_CLAUDE_STREAMING").unwrap_or_else(|_| "true".to_string())
       == "true"
   {
       env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
   }
   ```

2. `src/cook/workflow/executor/validation.rs` (line ~599-603):
   ```rust
   // REMOVE THIS BLOCK:
   if std::env::var("PRODIGY_CLAUDE_STREAMING").unwrap_or_else(|_| "true".to_string())
       == "true"
   {
       env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
   }
   ```

3. `src/cook/execution/mapreduce/command/claude.rs` (line ~53-57):
   ```rust
   // REMOVE THIS BLOCK:
   if std::env::var("PRODIGY_CLAUDE_STREAMING").unwrap_or_else(|_| "true".to_string())
       == "true"
   {
       env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
   }
   ```

4. `src/cook/execution/mapreduce/coordination/executor.rs` (line ~310):
   ```rust
   // REMOVE THIS LINE:
   env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
   ```

**Phase 3: Update Tests**

1. Verify tests that set `PRODIGY_CLAUDE_STREAMING=true` still pass (no-op)
2. Verify tests that set `PRODIGY_CLAUDE_STREAMING=false` disable streaming
3. Verify tests without the environment variable use streaming mode
4. Update test assertions to match new default behavior

**Phase 4: Update Documentation**

1. Update `CLAUDE.md` section on environment variables:
   ```markdown
   # OLD:
   - `PRODIGY_CLAUDE_STREAMING="true"` - Enables streaming mode for Claude commands (when verbosity >= 1)

   # NEW:
   - `PRODIGY_CLAUDE_STREAMING="false"` - Disables streaming mode (streaming enabled by default)
   ```

2. Update `book/src/configuration.md` environment variable table

3. Add migration note for users who explicitly set `PRODIGY_CLAUDE_STREAMING=false`

### Architecture Changes

**Before** (Current State):
```
Workflow Executor
    ↓
Set PRODIGY_CLAUDE_STREAMING=true (multiple locations)
    ↓
Claude Executor checks env var
    ↓
If "true" → streaming mode
If not set → print mode (legacy)
```

**After** (New State):
```
Workflow Executor
    ↓
Claude Executor checks env var
    ↓
If "false" → print mode (opt-out)
Otherwise → streaming mode (default)
```

### Data Structures

No new data structures required. Changes are limited to:
- Boolean logic inversion in environment variable checking
- Removal of redundant environment variable setting code

### APIs and Interfaces

**Public API Changes**: None - behavior remains the same for users

**Internal Changes**:
- `execute_claude_command()` internal logic simplified
- Environment variable semantics inverted (opt-out vs opt-in)

## Dependencies

### Prerequisites
None - this is a self-contained refactoring

### Affected Components
- `src/cook/execution/claude.rs` - Core execution logic
- `src/cook/workflow/executor/context.rs` - Workflow environment setup
- `src/cook/workflow/executor/validation.rs` - Validation environment setup
- `src/cook/execution/mapreduce/command/claude.rs` - MapReduce environment setup
- `src/cook/execution/mapreduce/coordination/executor.rs` - Coordination environment setup
- Tests that verify streaming behavior
- Documentation (CLAUDE.md, book/src/configuration.md)

### External Dependencies
None

## Testing Strategy

### Unit Tests

1. **Test Default Streaming Behavior**
   ```rust
   #[tokio::test]
   async fn test_claude_default_streaming_enabled() {
       let executor = ClaudeExecutorImpl::new(MockCommandRunner::new());
       let env_vars = HashMap::new(); // No PRODIGY_CLAUDE_STREAMING set

       let result = executor
           .execute_claude_command("/test", Path::new("/tmp"), env_vars)
           .await
           .unwrap();

       // Verify streaming mode was used (JSON log created)
       assert!(result.json_log_location().is_some());
   }
   ```

2. **Test Explicit Opt-Out**
   ```rust
   #[tokio::test]
   async fn test_claude_streaming_disabled() {
       let executor = ClaudeExecutorImpl::new(MockCommandRunner::new());
       let mut env_vars = HashMap::new();
       env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "false".to_string());

       let result = executor
           .execute_claude_command("/test", Path::new("/tmp"), env_vars)
           .await
           .unwrap();

       // Verify print mode was used (no JSON log)
       assert!(result.json_log_location().is_none());
   }
   ```

3. **Test Backward Compatibility**
   ```rust
   #[tokio::test]
   async fn test_claude_streaming_explicit_true() {
       let executor = ClaudeExecutorImpl::new(MockCommandRunner::new());
       let mut env_vars = HashMap::new();
       env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());

       let result = executor
           .execute_claude_command("/test", Path::new("/tmp"), env_vars)
           .await
           .unwrap();

       // Verify streaming mode was used (same as default)
       assert!(result.json_log_location().is_some());
   }
   ```

### Integration Tests

1. **MapReduce Workflow Test**
   - Verify MapReduce agents create JSON logs by default
   - Verify DLQ items include `json_log_location`
   - Verify event logs capture streaming data

2. **Standard Workflow Test**
   - Verify workflow steps create JSON logs by default
   - Verify validation steps create JSON logs by default
   - Verify failure logs include JSON log location

3. **Opt-Out Test**
   - Set `PRODIGY_CLAUDE_STREAMING=false` globally
   - Run workflow
   - Verify no JSON logs created

### Performance Tests

No performance impact expected - streaming is already the default behavior. Verify:
- No regression in workflow execution time
- No change in memory usage
- No change in disk I/O patterns

### User Acceptance

1. **Developer Workflow**:
   - Run typical workflow without setting environment variables
   - Verify JSON logs appear in `~/.prodigy/logs/claude-streaming/`
   - Check that log paths are displayed during execution

2. **CI/CD Workflow**:
   - Set `PRODIGY_CLAUDE_STREAMING=false` in CI environment
   - Verify no JSON logs created
   - Verify workflow executes successfully

## Documentation Requirements

### Code Documentation

1. Update docstring for `execute_claude_command`:
   ```rust
   /// Execute a Claude command
   ///
   /// By default, executes in streaming mode with `--output-format stream-json`,
   /// creating detailed JSON logs in `~/.prodigy/logs/claude-streaming/`.
   ///
   /// Set `PRODIGY_CLAUDE_STREAMING=false` in env_vars to disable streaming
   /// and use legacy `--print` mode instead.
   ```

2. Add comment explaining inverted logic:
   ```rust
   // Streaming is enabled by default for auditability
   // Only disabled if explicitly set to "false"
   let streaming_disabled = env_vars
       .get("PRODIGY_CLAUDE_STREAMING")
       .is_some_and(|v| v == "false");
   ```

### User Documentation

1. **CLAUDE.md** - Update environment variables section:
   ```markdown
   ## Environment Variables

   When executing Claude commands, Prodigy sets these environment variables:
   - `PRODIGY_AUTOMATION="true"` - Signals automated execution mode

   ### User-Configurable Variables

   - `PRODIGY_CLAUDE_STREAMING="false"` - Disable JSON streaming (enabled by default)
     - Default: Streaming enabled, logs saved to `~/.prodigy/logs/claude-streaming/`
     - When disabled: Uses legacy `--print` mode, no JSON logs created
     - Use case: CI/CD environments with storage constraints
   ```

2. **book/src/configuration.md** - Update table:
   ```markdown
   | Variable | Default | Description |
   |----------|---------|-------------|
   | `PRODIGY_AUTOMATION` | `"true"` | Signals automated execution mode |
   | `PRODIGY_CLAUDE_STREAMING` | `enabled` | Set to `"false"` to disable JSON logs |
   ```

3. **book/src/troubleshooting.md** - Add section:
   ```markdown
   ### Disabling JSON Logging

   By default, Prodigy logs all Claude interactions to `~/.prodigy/logs/claude-streaming/`.
   To disable logging (e.g., in CI/CD with storage constraints):

   ```bash
   export PRODIGY_CLAUDE_STREAMING=false
   prodigy run workflow.yml
   ```

   Note: Disabling logging reduces debugging capability if workflows fail.
   ```

### Architecture Updates

Update `ARCHITECTURE.md` section on Claude execution:
```markdown
## Claude Execution Modes

Prodigy supports two Claude execution modes:

1. **Streaming Mode (Default)**:
   - Uses `--output-format stream-json --verbose`
   - Creates JSON logs in `~/.prodigy/logs/claude-streaming/`
   - Provides real-time observability via event streaming
   - Enables detailed debugging of workflow failures

2. **Print Mode (Legacy)**:
   - Uses `--print` flag
   - No JSON logs created
   - Minimal output for resource-constrained environments
   - Enabled via `PRODIGY_CLAUDE_STREAMING=false`

The default streaming mode ensures workflow auditability and debugging capability.
```

## Implementation Notes

### Rationale for Default Streaming

1. **Auditability**: Every Claude interaction should be logged for compliance and debugging
2. **Debugging**: JSON logs are essential for troubleshooting workflow failures
3. **Observability**: Streaming provides real-time visibility into long-running workflows
4. **MapReduce**: DLQ debugging relies on JSON log locations for failed agents

### Why Not Remove Print Mode Entirely?

Print mode is retained for edge cases:
- CI/CD environments with strict storage limits
- Embedded systems with limited disk space
- Scenarios where logging is handled externally
- Legacy compatibility during migration

### Environment Variable Semantics

**Design Choice**: Use `PRODIGY_CLAUDE_STREAMING=false` to disable (opt-out) rather than requiring `=true` to enable (opt-in).

**Reasoning**:
- Makes the default behavior explicit (streaming enabled unless disabled)
- Reduces boilerplate in workflow configuration
- Aligns with security best practice (audit by default)
- Simplifies code (no need to set env var in multiple places)

### Testing Considerations

**Critical Test Coverage**:
1. Default behavior (no env var) → streaming enabled
2. Explicit opt-out (`=false`) → streaming disabled
3. Explicit opt-in (`=true`) → streaming enabled (backward compat)
4. Invalid values (e.g., `=invalid`) → streaming enabled (fail-safe default)

**Edge Cases**:
- Empty string: `PRODIGY_CLAUDE_STREAMING=""` → streaming enabled
- Case sensitivity: `PRODIGY_CLAUDE_STREAMING=False` → streaming enabled (only lowercase "false" disables)

## Migration and Compatibility

### Breaking Changes

**None** - This change is backward compatible:
- Workflows that don't set the environment variable: **No change** (streaming already default)
- Workflows that set `PRODIGY_CLAUDE_STREAMING=true`: **No change** (same behavior)
- Workflows that set `PRODIGY_CLAUDE_STREAMING=false`: **No change** (opt-out still works)

### Migration Path

**For Existing Users**:
1. No action required - behavior remains the same
2. Users who want to disable streaming: Set `PRODIGY_CLAUDE_STREAMING=false`
3. Code that explicitly sets `=true` can be removed (no-op now)

**For New Users**:
1. Streaming enabled by default - no configuration needed
2. JSON logs appear in `~/.prodigy/logs/claude-streaming/`
3. To disable: Set `PRODIGY_CLAUDE_STREAMING=false`

### Version Compatibility

- **Before this change**: Streaming only when explicitly enabled (opt-in)
- **After this change**: Streaming always enabled unless explicitly disabled (opt-out)
- **Compatibility**: Workflows work in both versions without modification

## Success Metrics

- **Code Simplification**: Remove 20+ lines of redundant environment variable setting code
- **Test Coverage**: 100% pass rate for all existing streaming tests
- **Auditability**: JSON logs created for 100% of Claude executions (unless explicitly disabled)
- **Performance**: No measurable performance difference vs. current implementation
- **User Feedback**: No reported issues with default streaming behavior

## Future Enhancements

### Log Rotation and Cleanup

Consider adding automatic log rotation:
```bash
# Future feature: Automatic cleanup of old logs
prodigy logs clean --older-than 7d
```

### Streaming Analytics

Future dashboard for analyzing streaming logs:
- Token usage trends
- Command duration distribution
- Error rate by command type
- Tool usage frequency

### Structured Log Format

Enhance JSON log format with additional metadata:
- Workflow ID correlation
- Step number
- Retry attempt number
- Performance metrics

## Related Specifications

- **Spec 121**: Claude Command Observability - JSON log location tracking
- **Spec 126**: GitHub Workflow Template System - CI/CD integration patterns

## References

- Claude CLI documentation: `--output-format stream-json` flag behavior
- Prodigy streaming architecture: `docs/streaming.md`
- Event logging implementation: `src/cook/execution/events/streaming.rs`
