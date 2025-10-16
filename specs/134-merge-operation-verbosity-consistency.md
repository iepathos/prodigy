---
number: 134
title: Merge Operation Verbosity Consistency
category: optimization
priority: medium
status: draft
dependencies: []
created: 2025-10-15
---

# Specification 134: Merge Operation Verbosity Consistency

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

When running workflows with merge operations, Claude streaming JSON logs are output to the console even when no verbosity flags (`-v`, `-vv`, `-vvv`) are specified. This behavior is inconsistent with other workflow operations and creates cluttered console output.

### Current Behavior

**Regular Workflow Steps**: Correctly respect verbosity settings
- With `verbosity = 0`: Only show progress indicators and INFO logs, streaming JSON confined to log files
- With `verbosity >= 1`: Show streaming JSON output to console

**Merge Operations**: Incorrectly ignore verbosity settings
- Always output streaming JSON to console, regardless of verbosity level
- Makes console output difficult to read (50+ lines of JSON)
- Obscures important progress information

### Root Cause Analysis

The issue exists in multiple merge code paths that don't propagate verbosity settings to Claude execution:

1. **WorktreeManager::execute_claude_merge()** (src/worktree/manager.rs:402-425)
   - Builds environment variables but doesn't check verbosity before setting streaming flags
   - Line 407: `self.build_claude_environment_variables()` doesn't receive verbosity context

2. **WorktreeManager::execute_merge_claude_command()** (src/worktree/manager.rs:1040-1092)
   - Used in custom merge workflows
   - Lines 1063-1065: **Correctly** sets `PRODIGY_CLAUDE_STREAMING=true` only if `verbosity >= 1`
   - But doesn't control console output independently

3. **MapReduceExecutor merge operations** (src/cook/execution/mapreduce/coordination/executor.rs:328-360)
   - Lines 337-338: Only sets `PRODIGY_AUTOMATION=true`, no verbosity control
   - Missing verbosity-based streaming configuration

4. **MergeQueue operations** (src/cook/execution/mapreduce/merge_queue.rs:93-103)
   - Lines 93-94: Only sets `PRODIGY_AUTOMATION=true`, no verbosity control
   - Missing verbosity-based streaming configuration

### Desired Behavior

All merge operations should respect global verbosity settings:
- **Default (`verbosity = 0`)**: Clean output with progress indicators only, streaming JSON confined to log files
- **Verbose (`verbosity >= 1`)**: Show Claude streaming JSON output for debugging

## Objective

Make all merge operations respect the global verbosity setting, ensuring consistent behavior across workflow execution and merge operations. Users should only see Claude streaming JSON when explicitly requesting verbose output.

## Requirements

### Functional Requirements

1. **Verbosity-Aware Environment Variables**
   - All merge operations must check verbosity level before executing Claude commands
   - Set `PRODIGY_CLAUDE_STREAMING` based on verbosity (only `"true"` if `verbosity >= 1`)
   - Optionally set `PRODIGY_CLAUDE_CONSOLE_OUTPUT` based on verbosity for explicit console control

2. **Consistent Merge Behavior**
   - `WorktreeManager::execute_claude_merge()`: Respect verbosity from `self.verbosity`
   - `WorktreeManager::execute_merge_claude_command()`: Already correct, verify it works
   - `MapReduceExecutor` merge operations: Pass verbosity to Claude executor
   - `MergeQueue` merge operations: Pass verbosity to Claude executor

3. **Consistent with Existing Patterns**
   - Follow the same pattern used in `execute_merge_claude_command()` (manager.rs:1063-1065)
   - Reuse `WorktreeManager::build_claude_environment_variables()` but make it verbosity-aware
   - Ensure all merge code paths use the same environment variable logic

4. **Preserve Functionality**
   - Streaming JSON logs must still be saved to log files (for debugging and audit trail)
   - Log file paths must still be displayed: `📁 Claude streaming log: /path/to/file.jsonl`
   - Only console output should be controlled by verbosity

### Non-Functional Requirements

1. **Backward Compatibility**
   - Environment variable override `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true` must still force console output
   - Existing tests must continue to pass
   - No breaking changes to public API

2. **Consistency**
   - All merge code paths must use identical verbosity logic
   - No special cases or exceptions
   - Clear, predictable behavior

3. **Maintainability**
   - Centralize verbosity-to-environment-variable logic in a single function
   - Document the behavior clearly in code comments
   - Add tests for both verbosity levels

## Acceptance Criteria

- [ ] `WorktreeManager::build_claude_environment_variables()` becomes verbosity-aware
- [ ] `WorktreeManager::execute_claude_merge()` uses verbosity-aware environment variables
- [ ] MapReduceExecutor merge operations use verbosity-aware environment variables
- [ ] MergeQueue merge operations use verbosity-aware environment variables
- [ ] With `verbosity = 0`: No Claude JSON streaming output to console
- [ ] With `verbosity >= 1`: Claude JSON streaming output appears on console
- [ ] `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true` override still works regardless of verbosity
- [ ] Streaming JSON logs still saved to log files in all cases
- [ ] Log file path still displayed: `📁 Claude streaming log: /path/to/file.jsonl`
- [ ] All existing tests pass without modification
- [ ] New tests verify verbosity behavior in merge operations
- [ ] Documentation updated to reflect verbosity control

## Technical Details

### Implementation Approach

**Phase 1: Centralize Verbosity Logic**
- Extract verbosity-to-environment logic into pure helper function
- Function: `build_claude_merge_env_vars(verbosity: u8, automation: bool) -> HashMap<String, String>`
- Place in `WorktreeManager` or new `merge_helpers` module
- Returns environment variables with proper verbosity flags

**Phase 2: Update WorktreeManager Methods**
- Modify `build_claude_environment_variables()` to accept verbosity parameter
- Update `execute_claude_merge()` to use verbosity-aware env vars
- Verify `execute_merge_claude_command()` continues working correctly

**Phase 3: Update MapReduce Merge Paths**
- Update `MapReduceExecutor::merge_mapreduce_to_parent_worktree()` to use verbosity-aware env vars
- Update `MergeQueue` merge logic to use verbosity-aware env vars
- Propagate verbosity from executor to merge queue

**Phase 4: Testing and Validation**
- Add unit tests for `build_claude_merge_env_vars()` pure function
- Add integration tests for merge operations with verbosity=0 and verbosity=1
- Verify environment variable override still works
- Test all merge code paths

### Architecture Changes

**Modified Components**:
- `src/worktree/manager.rs`: Make `build_claude_environment_variables()` verbosity-aware
- `src/cook/execution/mapreduce/coordination/executor.rs`: Use verbosity in merge operations
- `src/cook/execution/mapreduce/merge_queue.rs`: Use verbosity in queue merge operations
- `src/cook/orchestrator/*.rs`: Ensure verbosity is propagated to all merge operations

**New Helper Function**:
```rust
// src/worktree/manager.rs or new merge_helpers.rs

/// Build Claude environment variables for merge operations with verbosity control
///
/// # Arguments
/// * `verbosity` - Verbosity level (0 = quiet, 1+ = verbose)
/// * `automation` - Whether this is automated execution (sets PRODIGY_AUTOMATION)
///
/// # Returns
/// HashMap of environment variables to pass to Claude executor
///
/// # Behavior
/// - Always sets PRODIGY_AUTOMATION if automation=true
/// - Sets PRODIGY_CLAUDE_STREAMING based on verbosity:
///   - verbosity = 0: Not set (uses default, which enables streaming to file)
///   - verbosity >= 1: Set to "true" (enables streaming to console)
/// - Respects PRODIGY_CLAUDE_CONSOLE_OUTPUT override if set in environment
fn build_claude_merge_env_vars(verbosity: u8, automation: bool) -> HashMap<String, String> {
    let mut env_vars = HashMap::new();

    if automation {
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());
    }

    // Only enable console output if verbosity >= 1
    // This controls whether JSON streaming appears on console
    if verbosity >= 1 {
        env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
    }

    // Respect environment variable override for console output
    if std::env::var("PRODIGY_CLAUDE_CONSOLE_OUTPUT").unwrap_or_default() == "true" {
        env_vars.insert("PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(), "true".to_string());
    }

    env_vars
}
```

**Modified Methods**:

```rust
// src/worktree/manager.rs

impl WorktreeManager {
    /// Build Claude environment variables for merge operations
    fn build_claude_environment_variables(&self) -> HashMap<String, String> {
        build_claude_merge_env_vars(self.verbosity, true)
    }

    async fn execute_claude_merge(&self, worktree_branch: &str) -> Result<String> {
        if self.verbosity >= 1 {
            eprintln!("Running claude /prodigy-merge-worktree with branch: {worktree_branch}");
        }

        // Use verbosity-aware environment variables
        let env_vars = self.build_claude_environment_variables();
        let claude_executor = self.create_claude_executor();

        let result = claude_executor
            .execute_claude_command(
                &format!("/prodigy-merge-worktree {worktree_branch}"),
                &self.repo_path,
                env_vars,
            )
            .await
            .context("Failed to execute claude /prodigy-merge-worktree")?;

        Self::validate_claude_result(&result)?;
        if self.verbosity == 0 {
            // Clean output - only show the final result message
            println!("{}", result.stdout);
        }
        Ok(result.stdout)
    }
}
```

```rust
// src/cook/execution/mapreduce/coordination/executor.rs

impl MapReduceExecutor {
    async fn merge_mapreduce_to_parent_worktree(
        &self,
        mapreduce_branch: &str,
        parent_worktree: &Path,
    ) -> MapReduceResult<()> {
        info!(
            "Executing Claude-assisted merge: /prodigy-merge-worktree {}",
            mapreduce_branch
        );

        // Build environment variables with verbosity awareness
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Respect verbosity setting for console output
        if self.verbosity >= 1 {
            env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
        }

        // Respect environment variable override
        if std::env::var("PRODIGY_CLAUDE_CONSOLE_OUTPUT").unwrap_or_default() == "true" {
            env_vars.insert("PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(), "true".to_string());
        }

        let merge_result = self
            .claude_executor
            .execute_claude_command(
                &format!("/prodigy-merge-worktree {}", mapreduce_branch),
                &parent_worktree,
                env_vars,
            )
            .await
            .map_err(|e| {
                MapReduceError::MergeError(format!(
                    "Failed to merge MapReduce branch to parent: {}",
                    e
                ))
            })?;

        if !merge_result.success {
            return Err(MapReduceError::MergeError(format!(
                "Claude merge failed: {}",
                merge_result.stderr
            )));
        }

        Ok(())
    }
}
```

```rust
// src/cook/execution/mapreduce/merge_queue.rs

// Update merge queue to receive and use verbosity
impl MergeQueue {
    async fn process_merge_request(
        request: MergeRequest,
        executor: Arc<dyn ClaudeExecutor>,
        verbosity: u8,  // NEW PARAMETER
    ) -> MergeResult {
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Respect verbosity setting
        if verbosity >= 1 {
            env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
        }

        // Respect environment variable override
        if std::env::var("PRODIGY_CLAUDE_CONSOLE_OUTPUT").unwrap_or_default() == "true" {
            env_vars.insert("PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(), "true".to_string());
        }

        match executor
            .execute_claude_command(
                &format!("/prodigy-merge-worktree {}", request.branch_name),
                &request.env.working_dir,
                env_vars,
            )
            .await
        {
            // ... rest of implementation
        }
    }
}
```

### Data Structures

**No new data structures required** - uses existing `HashMap<String, String>` for environment variables.

**Modified Signatures**:
```rust
// WorktreeManager - no signature change (uses self.verbosity)
fn build_claude_environment_variables(&self) -> HashMap<String, String>

// New pure helper function
fn build_claude_merge_env_vars(verbosity: u8, automation: bool) -> HashMap<String, String>

// MergeQueue - add verbosity parameter
async fn process_merge_request(
    request: MergeRequest,
    executor: Arc<dyn ClaudeExecutor>,
    verbosity: u8,  // NEW
) -> MergeResult
```

### APIs and Interfaces

**No breaking changes** - all modifications are internal implementation details.

**Behavior Changes** (user-visible):
- Merge operations with `verbosity = 0` now produce clean console output
- Merge operations with `verbosity >= 1` show streaming JSON (debug mode)
- Environment variable `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true` still overrides verbosity

## Dependencies

**Prerequisites**: None

**Affected Components**:
- WorktreeManager (merge execution)
- MapReduceExecutor (MapReduce merge operations)
- MergeQueue (agent merge queue)
- ClaudeExecutor (receives environment variables)

**External Dependencies**: None

## Testing Strategy

### Unit Tests

1. **Pure Function Tests**
   ```rust
   #[test]
   fn test_build_claude_merge_env_vars_quiet() {
       let env = build_claude_merge_env_vars(0, true);
       assert_eq!(env.get("PRODIGY_AUTOMATION"), Some(&"true".to_string()));
       assert!(env.get("PRODIGY_CLAUDE_STREAMING").is_none());
   }

   #[test]
   fn test_build_claude_merge_env_vars_verbose() {
       let env = build_claude_merge_env_vars(1, true);
       assert_eq!(env.get("PRODIGY_AUTOMATION"), Some(&"true".to_string()));
       assert_eq!(env.get("PRODIGY_CLAUDE_STREAMING"), Some(&"true".to_string()));
   }

   #[test]
   fn test_build_claude_merge_env_vars_override() {
       std::env::set_var("PRODIGY_CLAUDE_CONSOLE_OUTPUT", "true");
       let env = build_claude_merge_env_vars(0, true);
       assert_eq!(env.get("PRODIGY_CLAUDE_CONSOLE_OUTPUT"), Some(&"true".to_string()));
       std::env::remove_var("PRODIGY_CLAUDE_CONSOLE_OUTPUT");
   }
   ```

2. **WorktreeManager Tests**
   ```rust
   #[tokio::test]
   async fn test_execute_claude_merge_respects_verbosity() {
       // Test that execute_claude_merge uses verbosity-aware env vars
       // Verify env_vars passed to claude_executor.execute_claude_command()
   }
   ```

### Integration Tests

1. **End-to-End Merge with Verbosity 0**
   - Run workflow with merge operation, no `-v` flag
   - Verify console output contains only progress indicators
   - Verify no Claude JSON streaming in console output
   - Verify streaming log file is created and populated

2. **End-to-End Merge with Verbosity 1**
   - Run workflow with merge operation, `-v` flag
   - Verify Claude JSON streaming appears in console output
   - Verify streaming log file is created and populated

3. **Environment Variable Override**
   - Set `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true`
   - Run workflow with verbosity=0
   - Verify Claude JSON streaming appears despite verbosity=0
   - Verify override takes precedence

4. **MapReduce Merge Operations**
   - Run MapReduce workflow with verbosity=0
   - Verify agent merges don't output JSON to console
   - Verify final merge doesn't output JSON to console
   - Run again with verbosity=1, verify JSON appears

### Performance Tests

**No performance impact expected** - just environment variable changes.

### Regression Tests

- Run all existing merge workflow tests
- Verify no changes to merge semantics or functionality
- Verify log files still created correctly
- Verify merge success/failure detection unchanged

## Documentation Requirements

### Code Documentation

- Document `build_claude_merge_env_vars()` with clear behavior description
- Add inline comments explaining verbosity logic in each merge path
- Update WorktreeManager docstrings to mention verbosity control

### User Documentation

**Update CLAUDE.md**:

```markdown
## Merge Operation Output Control

Merge operations respect the global verbosity setting:

### Default Mode (No Verbosity Flags)

```bash
prodigy run workflows/debtmap.yml
```

**Output**:
```
✅ Cook session completed successfully!
Merge session-abc123 to master [Y/n]: Y
🔄 Merging worktree 'session-abc123' into 'master' using Claude-assisted merge...
📁 Claude streaming log: ~/.prodigy/logs/claude-streaming/20251015_123456-uuid.jsonl
✅ Worktree changes merged successfully!
```

- Clean, minimal output
- No Claude JSON streaming to console
- Streaming logs saved to file for debugging

### Verbose Mode (-v Flag)

```bash
prodigy run workflows/debtmap.yml -v
```

**Output**:
```
✅ Cook session completed successfully!
Merge session-abc123 to master [Y/n]: Y
🔄 Merging worktree 'session-abc123' into 'master' using Claude-assisted merge...
📁 Claude streaming log: ~/.prodigy/logs/claude-streaming/20251015_123456-uuid.jsonl
{"type":"system","subtype":"init",...}
{"type":"assistant","message":{...}}
{"type":"user","message":{...}}
... (Claude streaming JSON continues)
✅ Worktree changes merged successfully!
```

- Shows Claude streaming JSON for debugging
- Useful for troubleshooting merge issues
- Full interaction visible in real-time

### Environment Variable Override

```bash
PRODIGY_CLAUDE_CONSOLE_OUTPUT=true prodigy run workflows/debtmap.yml
```

Forces Claude streaming output regardless of verbosity level.
```

**Update workflow-syntax.md** to document merge verbosity behavior.

## Implementation Notes

### Verbosity Propagation

Ensure verbosity is available in all merge contexts:
- `WorktreeManager` already has `self.verbosity`
- `MapReduceExecutor` needs to store verbosity from creation
- `MergeQueue` needs to receive verbosity as parameter

### Environment Variable Precedence

Order of precedence for console output control:
1. `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true` (highest priority - always show)
2. `verbosity >= 1` (show if verbose)
3. Default (verbosity = 0) (don't show)

### Logging Behavior

**Always** save streaming JSON to log files, regardless of verbosity:
- Log file creation and writing is independent of console output
- `📁 Claude streaming log: /path` message always displayed
- Users can tail log files for real-time debugging if needed

## Migration and Compatibility

### Breaking Changes

**None** - this is an internal implementation fix.

### Migration Path

**Automatic** - no user action required:
- Existing workflows benefit immediately
- No configuration changes needed
- Behavior becomes consistent automatically

### Compatibility Guarantees

- All existing workflows continue to work
- All existing tests pass unchanged
- Environment variable override preserved
- Log file creation unchanged
- Merge functionality unchanged

## Success Metrics

- [ ] All merge operations produce clean console output with verbosity=0
- [ ] Merge operations show JSON streaming with verbosity >= 1
- [ ] Environment variable override works in all merge contexts
- [ ] All existing tests pass without modification
- [ ] New tests verify verbosity behavior
- [ ] User-visible consistency across all workflow operations
- [ ] No performance regression

## Related Issues

**Bug Report**: PRODIGY_BUG_MERGE_VERBOSITY.md

**Root Cause**: Merge operations don't respect global verbosity settings, creating inconsistent and cluttered console output.

**Impact**: Medium severity - affects user experience but doesn't break functionality.
