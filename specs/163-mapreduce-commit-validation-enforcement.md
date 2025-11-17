---
number: 163
title: MapReduce Commit Validation Enforcement
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-11-17
---

# Specification 163: MapReduce Commit Validation Enforcement

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The `commit_required: true` flag in MapReduce workflow steps currently fails to enforce commit validation. This results in silent data loss where agents complete successfully without creating required commits, producing no output while reporting success.

A real-world incident demonstrated this severity:
- 6 parallel agents executed MapReduce workflow
- Only 1 agent created a commit (reddit.md)
- 5 agents produced NO commits (devto, hashnode, linkedin, medium, substack)
- All 5 empty agents reported "Successfully merged agent branch"
- Workflow reported: "Map phase completed: 6 successful, 0 failed" ✅
- Result: 83% data loss (5/6 files missing) with false success indication

This bug affects production workflows and undermines trust in MapReduce execution guarantees.

## Objective

Implement robust commit validation enforcement for MapReduce workflows that:
1. Detects when `commit_required: true` is set on agent template steps
2. Verifies commits were actually created after command execution
3. Fails agents that don't create required commits with clear error messages
4. Reports validation failures accurately in MapReduce summaries
5. Works correctly across worktree boundaries in parallel agent execution

## Requirements

### Functional Requirements

#### FR1: Commit Detection for Agent Commands
- **FR1.1**: Capture git HEAD SHA before executing each agent command with `commit_required: true`
- **FR1.2**: Capture git HEAD SHA after command execution completes
- **FR1.3**: Compare before/after SHAs in the **agent's worktree** (not parent worktree)
- **FR1.4**: Support both Claude and shell commands with `commit_required` flag
- **FR1.5**: Handle multiple sequential commands in agent template correctly

#### FR2: Validation Failure Handling
- **FR2.1**: Mark agent as failed when no commits detected for `commit_required: true` step
- **FR2.2**: Include detailed error message specifying:
  - Agent ID and item ID
  - Command that was expected to create commits
  - Base commit SHA (what the branch is still pointing to)
  - Step index that failed validation
- **FR2.3**: Add failed agent to DLQ with commit validation failure details
- **FR2.4**: Do NOT merge empty agent branches to parent worktree

#### FR3: Success Path Validation
- **FR3.1**: Allow agents to succeed when commits ARE created for `commit_required: true` steps
- **FR3.2**: Log commit SHAs and file changes for successful validation
- **FR3.3**: Include commit validation metadata in `AgentResult`
- **FR3.4**: Track commit count in `AgentCompleted` events

#### FR4: Reporting and Observability
- **FR4.1**: Update MapReduce summary to accurately reflect validation failures
- **FR4.2**: Distinguish "no commits to merge" from "successful merge" in logs
- **FR4.3**: Display validation failure reason in agent failure messages
- **FR4.4**: Include commit validation status in event stream

### Non-Functional Requirements

#### NFR1: Performance
- **NFR1.1**: Commit validation must add < 100ms overhead per agent
- **NFR1.2**: Git operations must run in agent worktree (not parent)
- **NFR1.3**: Validation checks must be non-blocking for parallel agents

#### NFR2: Reliability
- **NFR2.1**: Validation must work correctly in worktree isolation
- **NFR2.2**: Race conditions between command completion and git operations must be handled
- **NFR2.3**: Validation must survive git index lock contention
- **NFR2.4**: No false positives (failing agents that DID create commits)
- **NFR2.5**: No false negatives (passing agents that did NOT create commits)

#### NFR3: Backward Compatibility
- **NFR3.1**: Existing workflows without `commit_required` continue to work unchanged
- **NFR3.2**: Standard (non-MapReduce) workflows maintain current behavior
- **NFR3.3**: Resume functionality works with commit validation failures

#### NFR4: Debuggability
- **NFR4.1**: Validation failures must include enough context to debug
- **NFR4.2**: Git HEAD before/after SHAs logged at debug level
- **NFR4.3**: Worktree path included in validation error messages

## Acceptance Criteria

### AC1: Test Coverage
- [ ] Unit test: Verify commit detection logic works correctly
- [ ] Unit test: Ensure HEAD comparison happens in correct worktree
- [ ] Integration test: MapReduce workflow with `commit_required: true` on all agents
- [ ] Integration test: Mixed workflow (some agents commit, some don't)
- [ ] Integration test: Multi-step agent template with `commit_required` on different steps
- [ ] Integration test: Verify failed agents appear in DLQ with correct failure details

### AC2: Test Workflow Execution
- [ ] Test workflow with 3 agents:
  - Agent 1: Claude command that DOES create commit → SUCCESS
  - Agent 2: Claude command that does NOT create commit → FAIL
  - Agent 3: Shell command that DOES create commit → SUCCESS
- [ ] Workflow reports: "2 successful, 1 failed"
- [ ] Failed agent (Agent 2) is in DLQ with "commit_required validation failed" error
- [ ] Only commits from Agent 1 and Agent 3 are merged to parent

### AC3: Error Messages
- [ ] Validation failure includes clear error:
  ```
  Agent mapreduce-xxx_agent_1 (item_1) FAILED: commit_required validation
    Expected: New commits after executing /my-command
    Actual: Branch still at 276c59d (no changes)
    Worktree: /path/to/agent/worktree
    Duration: 29.00s
  ```
- [ ] MapReduce summary shows:
  ```
  ❌ Map phase completed: 2 successful, 1 failed (total: 3)
     Failed agents: agent_1 (item_1)
     Reason: commit_required validation failed
  ```

### AC4: Worktree Isolation
- [ ] Validation checks run `git rev-parse HEAD` in agent worktree directory
- [ ] Validation does NOT check parent worktree by mistake
- [ ] Commits created in agent worktree are detected correctly
- [ ] Multiple parallel agents don't interfere with each other's validation

### AC5: Event Stream Accuracy
- [ ] `AgentCompleted` events include `commits: Vec<String>` field
- [ ] `AgentFailed` events include commit validation failure details
- [ ] `MapPhaseCompleted` event has accurate successful/failed counts

### AC6: Edge Cases
- [ ] Agent with NO `commit_required` steps → SUCCESS (no validation)
- [ ] Agent with multiple commands, only one has `commit_required: true` → validates only that step
- [ ] Agent that creates multiple commits → SUCCESS (any commits satisfy requirement)
- [ ] Agent interrupted before commit validation → marked as failed (via cleanup/timeout)

### AC7: DLQ Integration
- [ ] Failed agents with commit validation errors appear in DLQ
- [ ] DLQ entry includes:
  - Item data
  - Failure reason: "commit_required validation failed"
  - Expected behavior: "Command should have created git commit"
  - Command that was executed
  - Worktree path for debugging
- [ ] `prodigy dlq retry` can re-execute failed agents

### AC8: Resume Compatibility
- [ ] Workflow interrupted during map phase can be resumed
- [ ] Resume preserves commit validation behavior
- [ ] Failed agents from before interruption remain failed after resume

## Technical Details

### Implementation Approach

#### Phase 1: Commit Detection Infrastructure (src/cook/execution/mapreduce/agent/)

**File**: `src/cook/execution/mapreduce/agent/commit_validator.rs` (new)

```rust
/// Validates that commits were created when required
pub struct CommitValidator {
    git_ops: Arc<dyn GitOperations>,
}

impl CommitValidator {
    /// Check if commits were created between two HEAD references
    pub async fn verify_commits_created(
        &self,
        worktree_path: &Path,
        head_before: &str,
        head_after: &str,
    ) -> Result<CommitValidationResult> {
        // Compare SHAs
        // Return Ok(CommitValidationResult::Valid { commits }) if head_after != head_before
        // Return Ok(CommitValidationResult::NoCommits) if equal
    }

    /// Get list of commits between two references
    pub async fn get_commits_between(
        &self,
        worktree_path: &Path,
        from_ref: &str,
        to_ref: &str,
    ) -> Result<Vec<CommitInfo>> {
        // git log --format="%H" from_ref..to_ref
        // Parse and return commit metadata
    }
}

pub enum CommitValidationResult {
    Valid { commits: Vec<CommitInfo> },
    NoCommits,
}

pub struct CommitInfo {
    pub sha: String,
    pub message: String,
    pub files_changed: Vec<String>,
}
```

#### Phase 2: Agent Execution Integration (src/cook/execution/mapreduce/agent/execution.rs)

Modify `execute_commands` method:

```rust
async fn execute_commands(
    &self,
    handle: &AgentHandle,
    item: &Value,
    env: &ExecutionEnvironment,
    context: &ExecutionContext,
) -> ExecutionResult<(String, Vec<String>, Vec<String>, Option<String>)> {
    let mut total_output = String::new();
    let mut all_commits = Vec::new();
    let all_files = Vec::new();
    let mut json_log_location: Option<String> = None;

    let commit_validator = CommitValidator::new(context.git_ops.clone());

    // Build interpolation context
    let interp_context = self.build_interpolation_context(item, &handle.config.item_id);

    // Execute each command
    for (idx, step) in handle.commands.iter().enumerate() {
        // Update state
        {
            let mut state = handle.state.write().await;
            state.update_progress(idx + 1, handle.commands.len());
            state.set_operation(format!(
                "Executing command {}/{}",
                idx + 1,
                handle.commands.len()
            ));
        }

        // COMMIT VALIDATION: Capture HEAD before command execution
        let head_before = if step.commit_required {
            Some(commit_validator.get_head(handle.worktree_path()).await?)
        } else {
            None
        };

        // Interpolate the step
        let interpolated_step = self
            .interpolate_workflow_step(step, &interp_context)
            .await?;

        // Execute the command
        let (result, log_location) = self
            .execute_single_command(&interpolated_step, handle.worktree_path(), env, context)
            .await?;

        // Store the log location from the last Claude command
        if log_location.is_some() {
            json_log_location = log_location;
        }

        // Collect output
        total_output.push_str(&result.stdout);
        if !result.stderr.is_empty() {
            total_output.push_str("\n[STDERR]: ");
            total_output.push_str(&result.stderr);
        }

        // Check for failure
        if !result.success {
            return Err(ExecutionError::CommandFailed(format!(
                "Command {} failed with exit code {}",
                idx + 1,
                result.exit_code.unwrap_or(-1)
            )));
        }

        // COMMIT VALIDATION: Check if commits were created
        if let Some(before_sha) = head_before {
            let head_after = commit_validator.get_head(handle.worktree_path()).await?;

            if head_after == before_sha {
                // NO COMMITS CREATED - FAIL VALIDATION
                return Err(ExecutionError::CommitValidationFailed {
                    agent_id: handle.config.agent_id.clone(),
                    item_id: handle.config.item_id.clone(),
                    step_index: idx,
                    command: step.display_name(),
                    base_commit: before_sha,
                    worktree_path: handle.worktree_path().to_string_lossy().to_string(),
                });
            } else {
                // Commits were created - collect metadata
                let commits = commit_validator
                    .get_commits_between(handle.worktree_path(), &before_sha, &head_after)
                    .await?;

                for commit in commits {
                    all_commits.push(commit.sha.clone());
                }

                tracing::debug!(
                    agent_id = %handle.config.agent_id,
                    commits = ?all_commits,
                    "Commit validation passed"
                );
            }
        }
    }

    Ok((total_output, all_commits, all_files, json_log_location))
}
```

#### Phase 3: Error Type Extension

**File**: `src/cook/execution/mapreduce/agent/execution.rs`

```rust
/// Error type for execution operations
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("Timeout occurred after {0} seconds")]
    Timeout(u64),

    #[error("Commit validation failed for agent {agent_id} (item {item_id}): Command '{command}' (step {step_index}) did not create required commits. Branch still at {base_commit}. Worktree: {worktree_path}")]
    CommitValidationFailed {
        agent_id: String,
        item_id: String,
        step_index: usize,
        command: String,
        base_commit: String,
        worktree_path: String,
    },

    // ... existing variants
}
```

#### Phase 4: Agent Result Enhancement

**File**: `src/cook/execution/mapreduce/agent/types.rs`

```rust
pub struct AgentResult {
    pub agent_id: String,
    pub item_id: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration: Duration,
    pub commits: Vec<String>,  // <-- Track commit SHAs
    pub files_changed: Vec<String>,
    pub json_log_location: Option<String>,
    pub cleanup_status: Option<CleanupStatus>,
}
```

#### Phase 5: Event Stream Updates

**File**: `src/cook/execution/mapreduce/coordination/events.rs`

Ensure `MapReduceEvent::AgentCompleted` includes commits:

```rust
pub enum MapReduceEvent {
    AgentCompleted {
        job_id: String,
        agent_id: String,
        item_id: String,
        duration: Duration,
        commits: Vec<String>,  // <-- Include commit metadata
        json_log_location: Option<String>,
    },

    AgentFailed {
        job_id: String,
        agent_id: String,
        item_id: String,
        error: String,
        duration: Duration,
        failure_reason: FailureReason,  // <-- Include structured reason
        json_log_location: Option<String>,
    },

    // ... other events
}

pub enum FailureReason {
    CommandFailed,
    Timeout,
    CommitValidationFailed,  // <-- New variant
    CleanupFailed,
    Other(String),
}
```

#### Phase 6: Merge Logic Update

**File**: `src/cook/execution/mapreduce/coordination/executor.rs`

Update merge logic to skip empty branches:

```rust
async fn merge_agent_to_parent(
    &self,
    agent_result: &AgentResult,
    parent_worktree: &Path,
) -> Result<()> {
    // Check if agent created any commits
    if agent_result.commits.is_empty() {
        tracing::warn!(
            agent_id = %agent_result.agent_id,
            item_id = %agent_result.item_id,
            "Skipping merge: Agent created no commits"
        );
        return Ok(());
    }

    // Proceed with merge...
}
```

### Architecture Changes

**New Module**: `src/cook/execution/mapreduce/agent/commit_validator.rs`
- Encapsulates commit detection logic
- Provides pure functions for SHA comparison
- Handles worktree-specific git operations

**Modified Modules**:
- `src/cook/execution/mapreduce/agent/execution.rs`: Add commit validation to command execution loop
- `src/cook/execution/mapreduce/agent/types.rs`: Extend `AgentResult` and `ExecutionError`
- `src/cook/execution/mapreduce/coordination/executor.rs`: Update merge logic and event emission
- `src/cook/execution/mapreduce/coordination/events.rs`: Add commit metadata to events

### Data Structures

**AgentResult Enhancement**:
```rust
pub struct AgentResult {
    // ... existing fields ...
    pub commits: Vec<String>,          // NEW: Commit SHAs created by agent
    pub commit_validation: Option<CommitValidationStatus>,  // NEW: Validation details
}

pub struct CommitValidationStatus {
    pub steps_validated: usize,
    pub commits_created: usize,
    pub validation_passed: bool,
}
```

### Testing Strategy

#### Unit Tests

**Test File**: `src/cook/execution/mapreduce/agent/commit_validator_tests.rs`

```rust
#[tokio::test]
async fn test_verify_commits_created_success() {
    // Setup mock git operations
    // Verify CommitValidator correctly detects new commits
}

#[tokio::test]
async fn test_verify_commits_created_no_commits() {
    // Setup mock git operations with same HEAD before/after
    // Verify NoCommits result returned
}

#[tokio::test]
async fn test_get_commits_between() {
    // Test commit metadata extraction
}

#[tokio::test]
async fn test_worktree_path_isolation() {
    // Verify git operations run in correct worktree
}
```

**Test File**: `src/cook/execution/mapreduce/agent/execution_commit_validation_tests.rs`

```rust
#[tokio::test]
async fn test_execute_commands_with_commit_required_success() {
    // Agent executes command, creates commit
    // Verify execution succeeds and commits tracked
}

#[tokio::test]
async fn test_execute_commands_with_commit_required_failure() {
    // Agent executes command, does NOT create commit
    // Verify ExecutionError::CommitValidationFailed returned
}

#[tokio::test]
async fn test_multiple_steps_mixed_commit_required() {
    // Some steps have commit_required, some don't
    // Verify only required steps are validated
}
```

#### Integration Tests

**Test File**: `tests/mapreduce_commit_validation_integration_test.rs`

```rust
#[tokio::test]
async fn test_mapreduce_commit_required_all_pass() {
    // Workflow with 3 agents, all create commits
    // Verify all succeed, map phase reports 3/3 success
}

#[tokio::test]
async fn test_mapreduce_commit_required_mixed_results() {
    // Workflow with 3 agents:
    //   - agent_0: Creates commit → success
    //   - agent_1: No commit → fail
    //   - agent_2: Creates commit → success
    // Verify:
    //   - Map phase reports 2 successful, 1 failed
    //   - agent_1 in DLQ with correct error
    //   - Only agent_0 and agent_2 commits merged
}

#[tokio::test]
async fn test_mapreduce_commit_required_all_fail() {
    // Workflow where NO agents create commits
    // Verify all agents fail validation
}

#[tokio::test]
async fn test_mapreduce_no_commit_required() {
    // Workflow without commit_required flag
    // Verify backward compatibility (agents succeed regardless)
}
```

**Test Workflow File**: `tests/workflows/test-commit-validation.yml`

```yaml
name: test-commit-required-validation
mode: mapreduce

map:
  input: |
    [
      {"id": 1, "should_commit": true},
      {"id": 2, "should_commit": false},
      {"id": 3, "should_commit": true}
    ]
  json_path: "$[*]"

  agent_template:
    - shell: |
        if [ "${item.should_commit}" = "true" ]; then
          echo "test ${item.id}" > test-${item.id}.txt
          git add test-${item.id}.txt
          git commit -m "Add test file ${item.id}"
        else
          echo "test no commit ${item.id}" > test-no-commit-${item.id}.txt
          # Intentionally DO NOT commit
        fi
      commit_required: true
```

**Expected Results**:
- Agent 0 (id=1): SUCCESS (commits test-1.txt)
- Agent 1 (id=2): FAIL (no commit despite commit_required)
- Agent 2 (id=3): SUCCESS (commits test-3.txt)
- Map phase: "2 successful, 1 failed"
- DLQ contains agent_1 with commit validation error

#### Performance Tests

**Benchmark**: Measure validation overhead
```rust
#[tokio::test]
async fn bench_commit_validation_overhead() {
    // Execute 100 agents with commit_required
    // Measure time delta vs without validation
    // Assert < 100ms average overhead per agent
}
```

## Dependencies

**Prerequisites**: None - this is a bug fix for existing functionality

**Affected Components**:
- MapReduce agent execution pipeline
- Event logging system
- DLQ integration
- Merge coordination logic

**External Dependencies**: None (uses existing git operations abstraction)

## Documentation Requirements

### Code Documentation
- Add rustdoc comments to `CommitValidator` explaining worktree isolation
- Document `ExecutionError::CommitValidationFailed` with usage examples
- Add inline comments explaining validation timing (before/after command execution)

### User Documentation

**File**: `docs/mapreduce/troubleshooting.md`

Add section:
```markdown
## Commit Validation Failures

If you see errors like:
```
Agent agent_1 (item_1) FAILED: commit_required validation
  Expected: New commits after executing /my-command
  Actual: Branch still at 276c59d (no changes)
```

This means your agent command did not create the required git commit.

**Common Causes**:
1. Slash command doesn't include `git add` + `git commit` steps
2. Git commit failed silently (check git user.name/user.email)
3. Command created files but didn't stage/commit them

**Solutions**:
1. Update slash command to explicitly commit changes
2. Verify git configuration in agent worktree
3. Add commit verification step to your command
```

**File**: `docs/workflow-basics/command-level-options.md`

Update `commit_required` documentation:
```markdown
### commit_required

**Type**: boolean
**Default**: false
**Applies to**: MapReduce agent templates, standard workflow steps

When set to `true`, Prodigy verifies that the command created at least one git commit. If no commits are detected, the step/agent fails with a validation error.

**MapReduce Behavior**:
- Validation runs in the agent's worktree
- Failed agents are sent to DLQ with "commit validation failed" error
- Empty agent branches are NOT merged to parent worktree
- Map phase summary accurately reflects validation failures

**Example**:
```yaml
agent_template:
  - claude: "/process-item ${item.path}"
    commit_required: true  # Agent must commit results
```

**Troubleshooting**: See [Commit Validation Failures](../mapreduce/troubleshooting.md#commit-validation-failures)
```

### CLAUDE.md Updates

Add section to MapReduce documentation:

```markdown
## Commit Validation (Spec 163)

MapReduce workflows enforce `commit_required: true` validation on agent template steps.

**Validation Behavior**:
- Git HEAD captured before command execution
- Git HEAD compared after command completion
- Comparison happens in agent's worktree (not parent)
- Agents that don't create commits are marked as failed
- Failed agents do NOT merge to parent worktree

**Error Message Format**:
```
Agent mapreduce-xxx_agent_1 (item_1) FAILED: commit_required validation
  Expected: New commits after executing /cross-post-adapt
  Actual: Branch still at 276c59d (no changes)
  Worktree: /Users/user/.prodigy/worktrees/prodigy/agent-mapreduce-xxx_agent_1
  Duration: 29.00s
```

**DLQ Integration**:
Failed agents appear in DLQ with structured failure details for debugging and retry.
```

## Implementation Notes

### Timing Considerations

**Critical**: Capture HEAD **immediately** before and after command execution to avoid race conditions:

```rust
// ✅ CORRECT: Tight coupling with command execution
let head_before = get_head().await?;
let result = execute_command().await?;
let head_after = get_head().await?;

// ❌ WRONG: Gap allows external git operations to interfere
let head_before = get_head().await?;
tokio::time::sleep(Duration::from_millis(100)).await;  // BAD
let head_after = get_head().await?;
```

### Worktree Path Handling

Always pass agent worktree path to git operations:

```rust
// ✅ CORRECT: Uses agent worktree
commit_validator.verify_commits_created(
    handle.worktree_path(),  // Agent's isolated worktree
    &head_before,
    &head_after
).await?;

// ❌ WRONG: Uses parent worktree
commit_validator.verify_commits_created(
    &env.working_dir,  // Parent worktree - WRONG
    &head_before,
    &head_after
).await?;
```

### Git Operations Best Practices

Use existing `GitOperations` abstraction for testability:

```rust
impl CommitValidator {
    pub fn new(git_ops: Arc<dyn GitOperations>) -> Self {
        Self { git_ops }
    }

    pub async fn get_head(&self, worktree_path: &Path) -> Result<String> {
        let output = self.git_ops
            .git_command_in_dir(
                &["rev-parse", "HEAD"],
                "get HEAD",
                worktree_path
            )
            .await?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
```

### Error Context

Provide maximum debugging context in validation errors:

```rust
ExecutionError::CommitValidationFailed {
    agent_id: "mapreduce-20251117_204149_agent_1".to_string(),
    item_id: "item_1".to_string(),
    step_index: 0,
    command: "/cross-post-adapt".to_string(),
    base_commit: "276c59d".to_string(),
    worktree_path: "/Users/glen/.prodigy/worktrees/.../agent-xxx".to_string(),
}
```

This allows users to:
1. Identify which agent failed
2. See which command was expected to commit
3. Verify the worktree location
4. Check git log at base_commit to understand state

## Migration and Compatibility

### Breaking Changes

**None** - This is a bug fix that enforces existing documented behavior.

### Backward Compatibility

**Guaranteed**:
- Workflows without `commit_required` flag are unaffected
- Standard (non-MapReduce) workflows maintain current behavior
- Existing commit validation logic for non-MapReduce workflows unchanged

### Migration Path

**For Users with Failing Workflows**:

If this fix causes your existing workflows to fail:

1. **Investigate why commits aren't being created**:
   ```bash
   # Check agent worktree
   cd /path/to/agent/worktree  # From error message
   git log
   git status
   ```

2. **Fix slash commands to explicitly commit**:
   ```markdown
   ### Final Step: Commit Changes

   You MUST create a git commit with your changes:

   1. Stage the file:
      ```bash
      git add path/to/file.txt
      ```

   2. Create commit:
      ```bash
      git commit -m "Description of changes"
      ```

   3. Verify commit was created:
      ```bash
      git log -1
      ```
   ```

3. **Or remove `commit_required: true`** if commits aren't actually required:
   ```yaml
   agent_template:
     - claude: "/my-command"
       # commit_required: true  # Remove if not actually needed
   ```

### Rollback Plan

If critical issues arise, commit validation can be disabled via feature flag:

```rust
// In ExecutionContext or config
pub struct ExecutionConfig {
    pub enable_commit_validation: bool,  // Default: true
}

// In execution.rs
if context.config.enable_commit_validation && step.commit_required {
    // Run validation
}
```

Environment variable override:
```bash
export PRODIGY_DISABLE_COMMIT_VALIDATION=true
prodigy run workflow.yml
```

**Note**: This should only be used as emergency rollback, not long-term configuration.

## Implementation Phases

### Phase 1: Core Infrastructure (Days 1-2)
- [ ] Create `CommitValidator` module
- [ ] Implement `verify_commits_created` function
- [ ] Add `CommitValidationFailed` error variant
- [ ] Write unit tests for commit detection

### Phase 2: Agent Execution Integration (Days 3-4)
- [ ] Integrate validation into `execute_commands`
- [ ] Extend `AgentResult` with commit metadata
- [ ] Update error handling and propagation
- [ ] Add execution-level tests

### Phase 3: Event Stream & Reporting (Day 5)
- [ ] Update `MapReduceEvent` with commit data
- [ ] Modify event emission logic
- [ ] Update DLQ integration for validation failures
- [ ] Fix merge logic to skip empty branches

### Phase 4: Integration Testing (Days 6-7)
- [ ] Create test workflows
- [ ] Write integration tests for all scenarios
- [ ] Test DLQ retry with validation failures
- [ ] Verify resume compatibility

### Phase 5: Documentation & Deployment (Day 8)
- [ ] Update user documentation
- [ ] Add troubleshooting guide
- [ ] Update CLAUDE.md
- [ ] Final testing and deployment

## Success Metrics

### Correctness Metrics
- **Zero false negatives**: Agents that don't commit are always caught
- **Zero false positives**: Agents that do commit always pass
- **100% test coverage**: All validation paths covered by tests

### Performance Metrics
- **< 100ms overhead**: Validation adds minimal execution time
- **No blocking**: Parallel agents don't interfere with validation

### Observability Metrics
- **Clear error messages**: Users can debug validation failures
- **Accurate reporting**: MapReduce summaries reflect true success/failure rates
- **DLQ completeness**: All validation failures captured for retry

## Related Issues

**Bug Report**: `BUG_REPORT_commit_required_not_enforced.md`
**Severity**: High - Silent data loss
**Impact**: Production MapReduce workflows
**Reporter**: Glen Baker
**Date**: 2025-11-17

## References

- Spec 127: Worktree Isolation (MapReduce execution context)
- Spec 134: MapReduce Checkpoint and Resume (state preservation)
- Spec 136: Cleanup Failure Handling (agent cleanup guarantees)
- `src/cook/execution/mapreduce/agent/execution.rs`: Agent command execution
- `src/cook/execution/mapreduce/coordination/executor.rs`: Merge coordination
