---
number: 136
title: MapReduce Agent Cleanup Failure Handling
category: parallel
priority: high
status: draft
dependencies: []
created: 2025-10-20
---

# Specification 136: MapReduce Agent Cleanup Failure Handling

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Currently, when a MapReduce agent successfully completes its work and merges changes to the parent worktree, a cleanup failure (worktree deletion) causes the entire agent to be marked as **failed**, even though the actual work succeeded and was merged.

This creates false negatives in workflow execution where:

1. **Agent completes all steps successfully**: plan, implement, test, lint
2. **Agent merges changes to parent successfully**: all work is preserved
3. **Cleanup fails** (e.g., "Directory not empty" on macOS)
4. **Agent marked as failed**: Despite successful work and merge

### Current Behavior

From `executor.rs:1131-1141`:

```rust
Ok(()) => {
    info!("Successfully merged agent {} (item {})", agent_id, item_id);
    // Cleanup the worktree after successful merge
    agent_manager.cleanup_agent(handle).await.map_err(|e| {
        MapReduceError::ProcessingError(format!(
            "Failed to cleanup agent after merge: {}",
            e
        ))
    })?;  // ⚠️ The ? operator fails the entire agent if cleanup fails!
    true
}
```

### Inconsistent Error Handling

When merge **succeeds**: cleanup failure causes agent failure (wrong!)
When merge **fails**: cleanup failure is ignored with `let _ =` (correct!)

```rust
Err(e) => {
    warn!("Failed to merge agent...");
    let _ = agent_manager.cleanup_agent(handle).await;  // ✅ Cleanup errors ignored
    false
}
```

### Real-World Impact

From production logs (2025-10-20):

```
06:58:22.108062Z  INFO Successfully merged agent branch ... to parent  ✅
06:58:22.108081Z  INFO Successfully merged agent ... (item item_9)  ✅
06:58:26.773209Z  INFO MapReduce event: AgentFailed ...  ❌
                       "Failed to cleanup agent after merge:
                        Failed to remove worktree: Directory not empty"
```

**Result**: Agent 9 marked as failed in DLQ despite successful merge, reducing success rate from 10/10 to 9/10.

## Objective

Implement robust cleanup failure handling for MapReduce agents that distinguishes between critical failures (work not completed/merged) and non-critical failures (cleanup issues), ensuring agents are only marked as failed when their actual work fails.

## Requirements

### Functional Requirements

1. **Separate Work Success from Cleanup Success**
   - Agent success determination based solely on work completion and merge status
   - Cleanup failures treated as warnings, not errors
   - Preserve all work results even if cleanup fails

2. **Consistent Cleanup Error Handling**
   - Apply same cleanup failure handling regardless of merge success/failure
   - Use non-failing error handling (`if let Err(e)` or `let _ =`) for cleanup
   - Log cleanup failures at WARN level, not ERROR level

3. **Enhanced Cleanup Observability**
   - Log cleanup failures with full context (agent ID, item ID, error)
   - Track cleanup failures separately in metrics/events
   - Include cleanup status in agent completion events (not as failures)

4. **Graceful Degradation**
   - Continue workflow execution when cleanup fails
   - Orphaned worktrees tracked for later cleanup
   - Provide utility command to clean up stuck worktrees

### Non-Functional Requirements

1. **Correctness**: Agent status accurately reflects work completion, not cleanup status
2. **Observability**: Cleanup failures visible in logs but don't pollute failure metrics
3. **Maintainability**: Consistent cleanup patterns across all agent lifecycle stages
4. **Resilience**: Workflow continues despite filesystem issues or transient errors

## Acceptance Criteria

- [ ] Agent marked as **successful** when work completes and merges, regardless of cleanup status
- [ ] Agent marked as **failed** only when work or merge fails
- [ ] Cleanup failures logged at WARN level with full context
- [ ] Cleanup error handling consistent across all merge paths (success/failure)
- [ ] MapReduce event `AgentCompleted` includes cleanup status as metadata (not as failure)
- [ ] Orphaned worktrees tracked and reported separately from agent failures
- [ ] Existing tests pass with updated cleanup handling
- [ ] New tests verify agent success despite cleanup failure
- [ ] Documentation updated to explain cleanup failure handling

## Technical Details

### Implementation Approach

1. **Update `executor.rs` Cleanup Handling**

   **Current (line 1131-1141)**:
   ```rust
   Ok(()) => {
       info!("Successfully merged agent {} (item {})", agent_id, item_id);
       // Cleanup the worktree after successful merge
       agent_manager.cleanup_agent(handle).await.map_err(|e| {
           MapReduceError::ProcessingError(format!(
               "Failed to cleanup agent after merge: {}",
               e
           ))
       })?;
       true
   }
   ```

   **Fixed**:
   ```rust
   Ok(()) => {
       info!("Successfully merged agent {} (item {})", agent_id, item_id);
       // Cleanup the worktree after successful merge
       if let Err(e) = agent_manager.cleanup_agent(handle).await {
           warn!(
               "Failed to cleanup agent {} after successful merge: {}. \
                Work was successfully merged, worktree may need manual cleanup.",
               agent_id, e
           );
       }
       true
   }
   ```

2. **Add Cleanup Status to Agent Results**

   Extend `AgentResult` or add separate `CleanupStatus` field:
   ```rust
   pub struct AgentResult {
       pub agent_id: String,
       pub item_id: String,
       pub success: bool,
       pub error: Option<String>,
       pub commits: Vec<String>,
       pub cleanup_status: CleanupStatus,
   }

   pub enum CleanupStatus {
       Success,
       Failed { error: String },
       Skipped,
   }
   ```

3. **Track Orphaned Worktrees**

   When cleanup fails, record the worktree path:
   ```rust
   if let Err(e) = agent_manager.cleanup_agent(handle).await {
       warn!("Failed to cleanup agent {}: {}", agent_id, e);
       self.orphaned_worktrees.push(OrphanedWorktree {
           agent_id: agent_id.clone(),
           worktree_path: handle.worktree_path.clone(),
           error: e.to_string(),
           timestamp: Utc::now(),
       });
   }
   ```

4. **Provide Cleanup Utility**

   Add command to clean up orphaned worktrees:
   ```bash
   prodigy worktree clean-orphaned
   ```

### Architecture Changes

- **No breaking changes**: Internal implementation only
- **Event schema extension**: Add optional `cleanup_status` field to `AgentCompleted` event
- **New tracking**: Orphaned worktree registry for visibility

### Error Handling

1. **Cleanup failures**: Log and track, but don't fail agent
2. **Filesystem errors**: Retry with exponential backoff (optional enhancement)
3. **Permissions errors**: Warn user about manual intervention needed

## Dependencies

- **Prerequisites**: None - bug fix for existing functionality
- **Affected Components**:
  - `src/cook/execution/mapreduce/coordination/executor.rs` (main fix)
  - `src/cook/execution/mapreduce/resources/agent_manager.rs` (cleanup logic)
  - `src/cook/execution/mapreduce/events.rs` (event schema extension)

## Testing Strategy

### Unit Tests

1. **Test agent success with cleanup failure**
   ```rust
   #[tokio::test]
   async fn test_agent_success_despite_cleanup_failure() {
       // Simulate successful merge followed by cleanup error
       // Assert agent marked as successful
       // Assert cleanup failure logged but not propagated
   }
   ```

2. **Test consistent cleanup handling**
   ```rust
   #[tokio::test]
   async fn test_cleanup_failure_handling_consistency() {
       // Test cleanup failure after merge success
       // Test cleanup failure after merge failure
       // Assert same cleanup error handling in both paths
   }
   ```

3. **Test orphaned worktree tracking**
   ```rust
   #[tokio::test]
   async fn test_orphaned_worktree_registry() {
       // Simulate cleanup failure
       // Assert worktree tracked in orphaned list
       // Assert list accessible for cleanup utility
   }
   ```

### Integration Tests

1. **End-to-end MapReduce with cleanup failure**
   - Run workflow where one agent has cleanup failure
   - Assert workflow completes successfully
   - Assert agent work is merged despite cleanup failure
   - Assert reduce phase receives correct agent results

2. **Verify cleanup status in events**
   - Check `AgentCompleted` events include cleanup status
   - Verify DLQ does not contain items from cleanup failures
   - Confirm metrics show correct success rate

### Manual Testing

1. Create scenario that triggers cleanup failure:
   ```bash
   # In agent worktree, create file not tracked by git
   touch /path/to/agent-worktree/untracked-file.txt
   chmod 000 /path/to/agent-worktree/untracked-file.txt
   ```

2. Verify agent marked as successful despite cleanup failure
3. Check logs show warning, not error
4. Confirm workflow continues

## Documentation Requirements

### Code Documentation

- Add detailed comments explaining cleanup failure handling
- Document `CleanupStatus` enum and its usage
- Explain rationale for non-failing cleanup in code comments

### User Documentation

- Update CLAUDE.md to explain cleanup failure behavior
- Document `prodigy worktree clean-orphaned` command
- Add troubleshooting section for cleanup failures

### Architecture Updates

- Document cleanup failure handling strategy
- Explain orphaned worktree tracking mechanism
- Clarify distinction between work failure and cleanup failure

## Implementation Notes

### Filesystem Edge Cases

1. **macOS Directory Deletion**: May fail with "Directory not empty" due to:
   - `.DS_Store` files created asynchronously
   - File system events not yet flushed
   - **Solution**: Retry with short delay, or force remove

2. **NFS/Network Filesystems**: May have delayed deletions
   - **Solution**: Log warning, mark for later cleanup

3. **Permission Issues**: User may not have delete permissions
   - **Solution**: Provide clear error message with manual cleanup instructions

### Cleanup Retry Strategy (Optional Enhancement)

```rust
async fn cleanup_agent_with_retry(handle: AgentHandle) -> Result<(), CleanupError> {
    let mut attempts = 0;
    let max_attempts = 3;
    let mut delay = Duration::from_millis(100);

    loop {
        match cleanup_agent_inner(handle).await {
            Ok(()) => return Ok(()),
            Err(e) if attempts < max_attempts => {
                attempts += 1;
                warn!("Cleanup attempt {} failed: {}, retrying...", attempts, e);
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### Monitoring and Metrics

Track cleanup failures separately:
- `mapreduce.agent.cleanup.failures` (counter)
- `mapreduce.agent.cleanup.retry_count` (histogram)
- `mapreduce.orphaned_worktrees` (gauge)

## Migration and Compatibility

### No Breaking Changes

- Internal implementation fix only
- Event schema extension is backward compatible (optional field)
- Existing workflows benefit immediately

### Deployment

- No migration required
- No configuration changes needed
- Can be deployed incrementally

### Rollback

- Safe to rollback if issues discovered
- No data migration or schema changes to revert
