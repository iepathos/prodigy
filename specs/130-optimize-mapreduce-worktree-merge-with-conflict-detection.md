---
number: 130
title: Optimize MapReduce Worktree Merge with Conflict Detection
category: optimization
priority: medium
status: draft
dependencies: [117]
created: 2025-10-12
---

# Specification 130: Optimize MapReduce Worktree Merge with Conflict Detection

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 117 (MapReduce Custom Merge Workflows)

## Context

Currently, the MapReduce worktree merge process (implemented in commit 91803550) uses Claude's `/prodigy-merge-worktree` command for **all** agent worktree → parent worktree merges. This approach provides intelligent conflict resolution but has performance implications:

- **Every merge invokes Claude**: Even clean merges that would succeed with a simple `git merge --no-ff` go through the full Claude command execution pipeline
- **Unnecessary overhead**: Clean merges (no conflicts) don't benefit from Claude's intelligence but still pay the execution cost
- **Serial bottleneck**: The merge queue serializes all merges, and the Claude execution time compounds across many agents

The optimization opportunity: **Check if a merge would conflict before deciding whether to use Claude**. For clean merges (majority case), use fast `git merge` directly. For conflicted merges, fall back to Claude's intelligent conflict resolution.

## Objective

Optimize the MapReduce agent worktree merge process by implementing conflict detection before merge execution, using fast git merges for clean cases and Claude only when conflicts are detected.

## Requirements

### Functional Requirements

1. **Pre-merge Conflict Detection**
   - Before attempting any merge, check if the merge would create conflicts
   - Use `git merge --no-commit --no-ff` in a test scenario to detect conflicts
   - Determine merge strategy (fast git vs Claude) based on conflict detection result

2. **Fast Path for Clean Merges**
   - When no conflicts detected: Execute `git merge --no-ff` directly in parent worktree
   - Skip Claude invocation entirely for these cases
   - Maintain all existing merge semantics (no-ff, branch tracking, etc.)

3. **Claude Fallback for Conflicted Merges**
   - When conflicts detected: Use existing `/prodigy-merge-worktree` Claude command
   - Preserve all intelligent conflict resolution capabilities
   - Log the conflict reason for debugging

4. **Merge Queue Integration**
   - Maintain serialized merge processing via `MergeQueue`
   - Both fast and Claude merge paths execute within the queue
   - Ensure thread safety and proper error handling

5. **Metrics and Observability**
   - Log which merge path was taken (fast vs Claude)
   - Track merge performance metrics (duration, conflict rate)
   - Include metrics in event logs for analysis

### Non-Functional Requirements

1. **Performance**
   - Clean merges should complete in <1s (vs current 5-30s with Claude)
   - Overall MapReduce job time should decrease proportionally to clean merge ratio
   - No performance regression for conflicted merges

2. **Reliability**
   - Conflict detection must be accurate (no false negatives)
   - Fallback to Claude must be automatic and seamless
   - Existing error handling and retry logic preserved

3. **Maintainability**
   - Clear separation between conflict detection and merge execution
   - Reusable conflict detection logic for other merge scenarios
   - Comprehensive test coverage for both merge paths

## Acceptance Criteria

- [ ] `GitOperations` has new method `detect_merge_conflicts()` that checks for conflicts without modifying worktrees
- [ ] Conflict detection returns result indicating: clean merge possible, conflicts exist, or detection failed
- [ ] `merge_agent_to_parent()` in `lifecycle.rs` checks for conflicts before merge
- [ ] Clean merges execute `git merge --no-ff` directly without Claude invocation
- [ ] Conflicted merges fall back to `/prodigy-merge-worktree` Claude command
- [ ] Detection failures fall back to Claude command (safe default)
- [ ] Merge events include merge path used (fast_git vs claude vs fallback)
- [ ] Metrics logged: merge_duration, conflict_detected, merge_strategy
- [ ] Unit tests for conflict detection (clean, conflicted, edge cases)
- [ ] Integration tests demonstrating performance improvement
- [ ] Documentation updated in CLAUDE.md and module docs
- [ ] No existing tests break (backward compatibility)

## Technical Details

### Implementation Approach

**Phase 1: Conflict Detection Infrastructure**
- Add `detect_merge_conflicts()` to `GitOperations`
- Implement safe conflict detection using `git merge-tree` or temporary index
- Return enum: `MergeConflictStatus { Clean, Conflicted(Vec<String>), DetectionFailed(String) }`

**Phase 2: Merge Path Selection**
- Modify `AgentLifecycleManager::merge_agent_to_parent()` to call conflict detection
- Branch on conflict status: clean → fast git, conflicted → Claude, failed → Claude (safe)
- Add logging at each decision point

**Phase 3: Fast Merge Execution**
- Extract direct git merge logic into reusable function
- Execute `git merge --no-ff` in parent worktree for clean merges
- Maintain existing branch cleanup and error handling

**Phase 4: Metrics and Observability**
- Add merge strategy to `MapReduceEvent::AgentCompleted`
- Log conflict detection results and merge duration
- Include in DLQ failure details if merge fails

**Phase 5: Testing and Validation**
- Unit tests for conflict detection (various scenarios)
- Integration tests comparing fast vs Claude merge paths
- Performance benchmarks showing improvement

### Architecture Changes

**Modified Components**:
- `src/cook/execution/mapreduce/resources/git.rs`: Add `detect_merge_conflicts()`
- `src/cook/execution/mapreduce/agent/lifecycle.rs`: Update `merge_agent_to_parent()` with conflict detection
- `src/cook/execution/events/event_types.rs`: Add merge strategy field to events
- `src/cook/execution/mapreduce/merge_queue.rs`: Update to track merge path

**No Breaking Changes**:
- External API unchanged
- Existing workflows continue to work
- Backward compatible with existing tests

### Data Structures

```rust
// src/cook/execution/mapreduce/resources/git.rs

/// Result of conflict detection
#[derive(Debug, Clone)]
pub enum MergeConflictStatus {
    /// Merge can proceed cleanly (no conflicts)
    Clean,

    /// Merge would create conflicts (file paths included)
    Conflicted(Vec<String>),

    /// Conflict detection failed (error message included)
    /// Safe default: fall back to Claude
    DetectionFailed(String),
}

impl GitOperations {
    /// Detect if merging a branch would create conflicts
    ///
    /// Uses git merge-tree to perform a trial merge without
    /// modifying any worktrees. Returns conflict status.
    pub async fn detect_merge_conflicts(
        &self,
        source_branch: &str,
        target_worktree: &Path,
    ) -> MapReduceResult<MergeConflictStatus> {
        // Implementation using git merge-tree
        // ...
    }
}

// src/cook/execution/events/event_types.rs

/// Merge strategy used for agent merge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// Fast path: direct git merge (no conflicts detected)
    FastGit,

    /// Claude path: used for conflicted merges
    Claude,

    /// Fallback path: conflict detection failed, used Claude to be safe
    FallbackClaude,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCompletedEvent {
    pub agent_id: String,
    pub item_id: String,
    pub duration: chrono::Duration,
    pub commits: Vec<String>,
    pub json_log_location: Option<String>,

    // New field
    pub merge_strategy: Option<MergeStrategy>,
    pub merge_duration_ms: Option<u64>,
}
```

### APIs and Interfaces

**New Method**: `GitOperations::detect_merge_conflicts()`
```rust
/// Detect if merging a branch would create conflicts
///
/// # Arguments
/// * `source_branch` - Branch to merge from
/// * `target_worktree` - Worktree to merge into
///
/// # Returns
/// * `Clean` - Merge can proceed without conflicts
/// * `Conflicted(paths)` - Merge would have conflicts in listed files
/// * `DetectionFailed(error)` - Could not determine, fall back to Claude
pub async fn detect_merge_conflicts(
    &self,
    source_branch: &str,
    target_worktree: &Path,
) -> MapReduceResult<MergeConflictStatus>
```

**Modified Method**: `AgentLifecycleManager::merge_agent_to_parent()`
```rust
async fn merge_agent_to_parent(
    &self,
    agent_branch: &str,
    env: &ExecutionEnvironment,
) -> LifecycleResult<()> {
    // 1. Detect conflicts
    let conflict_status = git_ops
        .detect_merge_conflicts(agent_branch, &env.working_dir)
        .await?;

    // 2. Choose merge strategy
    match conflict_status {
        MergeConflictStatus::Clean => {
            // Fast path: direct git merge
            info!("Clean merge detected, using fast git merge");
            self.execute_fast_git_merge(agent_branch, env).await?;
        }
        MergeConflictStatus::Conflicted(files) => {
            // Claude path: intelligent conflict resolution
            warn!("Conflicts detected in {} files, using Claude merge", files.len());
            debug!("Conflicted files: {:?}", files);
            self.execute_claude_merge(agent_branch, env).await?;
        }
        MergeConflictStatus::DetectionFailed(error) => {
            // Fallback: be safe, use Claude
            warn!("Conflict detection failed: {}, falling back to Claude", error);
            self.execute_claude_merge(agent_branch, env).await?;
        }
    }

    Ok(())
}
```

## Dependencies

**Prerequisites**:
- Spec 117 (MapReduce Custom Merge Workflows) - Provides the Claude merge command infrastructure

**Affected Components**:
- GitOperations service (add conflict detection)
- AgentLifecycleManager (modify merge logic)
- MergeQueue (track merge strategy)
- MapReduce event logging

**External Dependencies**: None (uses existing git commands)

## Testing Strategy

### Unit Tests

1. **Conflict Detection Tests**
   ```rust
   // test_detect_merge_conflicts_clean
   // - Create worktree with clean merge scenario
   // - Verify returns MergeConflictStatus::Clean

   // test_detect_merge_conflicts_conflicted
   // - Create worktree with conflicting changes
   // - Verify returns MergeConflictStatus::Conflicted with file paths

   // test_detect_merge_conflicts_invalid_branch
   // - Attempt detection with non-existent branch
   // - Verify returns MergeConflictStatus::DetectionFailed

   // test_detect_merge_conflicts_no_worktree
   // - Attempt detection without worktree context
   // - Verify returns DetectionFailed
   ```

2. **Merge Path Selection Tests**
   ```rust
   // test_merge_agent_uses_fast_git_when_clean
   // - Mock conflict detection returning Clean
   // - Verify git merge executed directly (no Claude)

   // test_merge_agent_uses_claude_when_conflicted
   // - Mock conflict detection returning Conflicted
   // - Verify Claude command executed

   // test_merge_agent_uses_claude_when_detection_fails
   // - Mock conflict detection returning DetectionFailed
   // - Verify Claude command executed (safe fallback)
   ```

3. **Fast Merge Execution Tests**
   ```rust
   // test_execute_fast_git_merge_success
   // - Verify direct git merge completes successfully
   // - Check merge commit created with --no-ff

   // test_execute_fast_git_merge_failure
   // - Simulate git merge failure
   // - Verify proper error handling and cleanup
   ```

### Integration Tests

1. **End-to-End MapReduce with Clean Merges**
   - Run MapReduce workflow with 10 agents, all clean merges
   - Verify all agents use fast git merge path
   - Measure total job duration (should be significantly faster)

2. **Mixed Clean and Conflicted Merges**
   - Run MapReduce with 5 clean + 5 conflicted merges
   - Verify correct path selection for each
   - Verify conflicted merges still resolve correctly via Claude

3. **Fallback Behavior on Detection Failure**
   - Simulate conflict detection failure scenarios
   - Verify graceful fallback to Claude path
   - Verify no data loss or corruption

### Performance Tests

1. **Benchmark Clean Merge Performance**
   - Measure time for 100 clean merges: fast git vs Claude
   - Expected: >10x improvement for clean merges

2. **End-to-End Job Time Improvement**
   - Compare total MapReduce job time before/after optimization
   - Expected: 30-80% reduction (depending on conflict ratio)

3. **Conflict Detection Overhead**
   - Measure conflict detection time vs full Claude execution
   - Expected: <100ms for detection vs 5-30s for Claude

## Documentation Requirements

### Code Documentation

- Document `detect_merge_conflicts()` with usage examples
- Add docstrings explaining merge path selection logic
- Document `MergeConflictStatus` enum variants
- Add inline comments for complex git operations

### User Documentation

**Update CLAUDE.md**:

```markdown
## MapReduce Merge Optimization

Prodigy optimizes MapReduce agent merges using conflict detection:

### How It Works

1. **Conflict Detection**: Before merging an agent worktree to the parent, Prodigy checks if the merge would create conflicts
2. **Fast Path**: Clean merges (no conflicts) use direct git merge (~1s)
3. **Claude Path**: Conflicted merges use Claude for intelligent resolution (~5-30s)
4. **Safe Fallback**: If detection fails, Claude is used to ensure correctness

### Performance Impact

- **Clean merge rate**: Typically 80-95% in most workflows
- **Time savings**: 10-20x faster for clean merges
- **Overall improvement**: 30-80% reduction in MapReduce job time

### Observability

Merge strategy is logged in events and visible in verbose mode:

```bash
# View merge strategies used
prodigy events show <job_id> | grep merge_strategy

# Common output:
# - merge_strategy: fast_git (clean merge, <1s)
# - merge_strategy: claude (conflicts detected, ~10s)
# - merge_strategy: fallback_claude (detection failed, ~10s)
```

### Configuration

No configuration needed - optimization is automatic. Disable if needed:

```yaml
# Workflow configuration (future enhancement)
mapreduce:
  optimization:
    conflict_detection: false  # Force Claude for all merges
```
```

### Architecture Documentation

**Update architecture documentation**:

```markdown
## MapReduce Agent Merge Flow

### Optimized Merge Path (Post Spec-130)

```
Agent completes work
    ↓
Create agent branch
    ↓
Add to merge queue (serialized)
    ↓
Conflict detection ← NEW
    │
    ├─→ Clean? → Fast git merge (1s) → Success
    │
    ├─→ Conflicted? → Claude merge (10s) → Success
    │
    └─→ Detection failed? → Claude merge (safe fallback) → Success
    ↓
Cleanup agent worktree
```

### Conflict Detection Algorithm

Uses `git merge-tree` to perform a trial three-way merge:

1. Find common ancestor commit
2. Compute merge result without modifying worktrees
3. Check for conflict markers in output
4. Return conflict status with file paths if conflicts exist

**Advantages**:
- No worktree modification during detection
- Fast (< 100ms for typical repos)
- Accurate conflict detection
- Safe (never corrupts existing state)
```

## Implementation Notes

### Conflict Detection Implementation Options

**Option 1: git merge-tree (Recommended)**
```rust
// Uses git merge-tree to perform trial merge
// Pros: Fast, accurate, no side effects
// Cons: Requires git 2.32+ for --write-tree flag

let output = Command::new("git")
    .args(["merge-tree", "--write-tree", "HEAD", agent_branch])
    .current_dir(parent_worktree)
    .output()
    .await?;

// Check output for conflict markers
let has_conflicts = output.stdout.contains(b"<<<<<");
```

**Option 2: git merge --no-commit (Alternative)**
```rust
// Attempt merge with --no-commit flag
// Pros: Works on all git versions
// Cons: Modifies index, requires reset afterward

// Requires careful cleanup to reset index state
```

### Git Merge Semantics

Preserve all existing merge semantics:
- `--no-ff`: Always create merge commit (never fast-forward)
- Branch naming: `agent-{id}-{item_id}`
- Merge message: "Merge agent {id} (item {item_id})"

### Error Handling

**Conflict Detection Errors**:
- Invalid branch: Return `DetectionFailed`
- Git command failure: Return `DetectionFailed`
- Parsing errors: Return `DetectionFailed`
- **Always err on the side of safety**: Use Claude when in doubt

**Fast Merge Errors**:
- Merge conflict despite clean detection: Return error, log warning about detection failure
- Git command failure: Return error with context
- **Do NOT retry** - detection said it was clean, failure indicates real issue

**Claude Merge Errors**:
- Existing behavior preserved
- Include conflict information in context passed to Claude

### Logging and Debugging

**Decision Point Logging**:
```
INFO  Detecting merge conflicts for agent-123 → parent
DEBUG Conflict detection result: Clean (0 conflicts)
INFO  Using fast git merge path
INFO  Git merge completed in 847ms
```

```
INFO  Detecting merge conflicts for agent-456 → parent
DEBUG Conflict detection result: Conflicted (3 files)
DEBUG Conflicted files: [src/main.rs, tests/test.rs, Cargo.toml]
WARN  Conflicts detected, using Claude merge path
INFO  Claude merge completed in 12.3s
```

**Metrics Logging**:
```json
{
  "event": "agent_completed",
  "agent_id": "agent-123",
  "merge_strategy": "fast_git",
  "merge_duration_ms": 847,
  "conflict_detection_duration_ms": 45
}
```

## Migration and Compatibility

### Breaking Changes

**None** - This is an internal optimization. External API and behavior unchanged.

### Migration Path

**Automatic Migration**:
- Optimization is enabled immediately upon deployment
- No configuration or workflow changes required
- Existing workflows benefit automatically

**Rollback Plan**:
- If issues discovered: Add feature flag to disable optimization
- Fall back to always using Claude merge
- No data loss or corruption possible (optimization only affects merge path)

### Compatibility Guarantees

- Merge semantics unchanged (--no-ff, branch naming, etc.)
- Error handling behavior preserved
- Event logging backward compatible (new fields optional)
- Existing tests continue to pass
- No changes to workflow YAML syntax

## Success Metrics

### Performance Metrics

- [ ] Clean merge execution time: < 1s (baseline: 5-30s with Claude)
- [ ] Conflict detection time: < 100ms
- [ ] Overall MapReduce job time reduction: 30-80% (depends on clean merge ratio)
- [ ] No performance regression for conflicted merges

### Reliability Metrics

- [ ] Conflict detection accuracy: >99% (false negative rate < 1%)
- [ ] Zero merge corruption incidents
- [ ] Fallback to Claude in 100% of detection failure cases
- [ ] All existing tests pass without modification

### Adoption Metrics

- [ ] Optimization enabled by default
- [ ] >90% of clean merges use fast git path
- [ ] <5% of merges result in fallback due to detection failure
- [ ] User-visible performance improvement in real workflows

## Future Enhancements

### Short-term (After Initial Implementation)

1. **Parallel Conflict Detection**
   - Detect conflicts for multiple agents concurrently
   - Further reduce overall job time

2. **Conflict Detection Caching**
   - Cache detection results for recently checked branches
   - Avoid redundant detection for same state

3. **Enhanced Metrics Dashboard**
   - Real-time visualization of merge strategies
   - Conflict rate trends over time
   - Performance improvement attribution

### Long-term (Future Specs)

1. **Predictive Conflict Detection**
   - Use ML to predict conflict likelihood before agent execution
   - Pre-emptively avoid conflicting work assignments

2. **Partial Merge Strategies**
   - Merge non-conflicting files immediately
   - Use Claude only for conflicting subset
   - Hybrid fast/Claude approach

3. **User-Configurable Thresholds**
   - Allow users to tune detection sensitivity
   - Configure when to use Claude vs git
   - Per-workflow optimization settings

## References

- **Spec 117**: MapReduce Custom Merge Workflows (provides Claude merge infrastructure)
- **Commit 91803550**: Initial implementation of Claude-based MapReduce merge
- **Git merge-tree documentation**: https://git-scm.com/docs/git-merge-tree
- **MapReduce merge queue**: `src/cook/execution/mapreduce/merge_queue.rs`
