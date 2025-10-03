---
number: 117
title: MapReduce Custom Merge Workflows
category: parallel
priority: high
status: draft
dependencies: []
created: 2025-10-02
---

# Specification 117: MapReduce Custom Merge Workflows

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

MapReduce workflows in Prodigy involve two distinct merge points:

1. **Map Agent → Parent Worktree**: Each parallel agent's worktree merges back to the parent worktree after processing work items
2. **Parent Worktree → Original Branch**: After the reduce phase completes, the parent worktree merges to the original branch (main/master)

Currently, both merge points use default git merge behavior with no customization. This creates issues when:
- Temporary files need cleanup before merging
- Tests should run before accepting agent changes
- CI checks should validate the final result
- Custom conflict resolution is needed
- Documentation workflows generate temporary analysis files

The existing `merge:` field in MapReduce config only applies to the final merge (parent → original branch), leaving agent merges uncustomizable.

## Objective

Implement support for two distinct custom merge workflows in MapReduce:
- `agent_merge`: Customizes how map agents merge back to parent worktree
- `merge`: Customizes how parent worktree merges to original branch (already exists but not wired up)

This enables users to define validation, cleanup, and conflict resolution logic for both merge points.

## Requirements

### Functional Requirements

1. **Agent Merge Configuration**
   - Add `agent_merge` field to `MapReduceWorkflowConfig`
   - Support same syntax as existing `merge` field (array or full format)
   - Execute agent_merge workflow when merging agent → parent worktree
   - Provide agent-specific variables: `${item}`, `${worker.id}`, `${item_index}`

2. **Final Merge Configuration**
   - Wire up existing `merge` field to WorktreeManager
   - Pass merge workflow from config through executor to worktree manager
   - Execute merge workflow when merging parent → original branch
   - Provide merge-specific variables: `${merge.worktree}`, `${merge.source_branch}`, `${merge.target_branch}`

3. **Variable Interpolation**
   - Support all standard workflow variables in both merge types
   - Add agent-specific variables for agent_merge context
   - Preserve existing merge variables for final merge

4. **Error Handling**
   - Fail agent merge if agent_merge workflow fails
   - Send failed agent to DLQ if merge fails
   - Fail final merge if merge workflow fails
   - Provide clear error messages with merge context

### Non-Functional Requirements

1. **Backward Compatibility**
   - Both `agent_merge` and `merge` are optional
   - Default behavior unchanged when not specified
   - Existing workflows continue to work without modification

2. **Performance**
   - Minimal overhead when merge workflows not configured
   - Parallel agent merges remain independent
   - No significant delay added to merge operations

3. **Maintainability**
   - Reuse existing MergeWorkflow infrastructure
   - Consistent syntax across both merge types
   - Clear separation of concerns

## Acceptance Criteria

- [ ] `agent_merge` field added to `MapReduceWorkflowConfig` with optional MergeWorkflow type
- [ ] `agent_merge` supports both array and full config syntax (commands + timeout)
- [ ] Agent lifecycle manager accepts and executes agent_merge workflow
- [ ] Agent-specific variables (`${item}`, `${worker.id}`, `${item_index}`) available in agent_merge
- [ ] Existing `merge` field properly wired from config → executor → WorktreeManager
- [ ] Final merge workflow executes after reduce phase completes
- [ ] Merge variables (`${merge.worktree}`, `${merge.source_branch}`, `${merge.target_branch}`) available in merge
- [ ] Failed agent_merge sends item to DLQ
- [ ] Failed merge workflow fails the entire MapReduce job
- [ ] Comprehensive tests for both merge types
- [ ] Documentation updated in docs/workflow-syntax.md
- [ ] Example workflows demonstrate both merge types

## Technical Details

### Implementation Approach

**Phase 1: Configuration Layer**
- Add `agent_merge: Option<MergeWorkflow>` to `MapReduceWorkflowConfig`
- Reuse existing `MergeWorkflow` struct and deserialization logic
- Add tests for agent_merge parsing in mapreduce.rs

**Phase 2: Lifecycle Manager Updates**
- Update `AgentLifecycleManager` trait to accept `agent_merge` workflow
- Modify `DefaultLifecycleManager::new()` to store agent_merge config
- Update `merge_agent_to_parent()` to execute custom workflow
- Execute agent_merge in agent's worktree before git merge

**Phase 3: Executor Integration**
- Pass `merge` workflow to `WorktreeManager::with_config()`
- Pass `agent_merge` workflow to `DefaultLifecycleManager::new()`
- Update `execute_mapreduce_workflow()` in orchestrator to extract configs
- Wire configs through MapReduceExecutor initialization

**Phase 4: Variable Context**
- Add agent variables to interpolation context for agent_merge
- Preserve existing merge variables for final merge
- Ensure proper scoping of variables per merge type

**Phase 5: Error Handling**
- Wrap agent_merge execution in Result
- Convert merge failures to DLQ items with context
- Fail job on final merge workflow failure
- Include merge step details in error messages

### Architecture Changes

**Modified Components**:
- `src/config/mapreduce.rs`: Add agent_merge field
- `src/cook/execution/mapreduce/agent/lifecycle.rs`: Accept and execute agent_merge
- `src/cook/workflow/executor.rs`: Pass merge to WorktreeManager
- `src/cook/orchestrator.rs`: Extract and wire merge configs
- `src/worktree/manager.rs`: Already supports custom_merge_workflow

**No Breaking Changes**:
- Both fields optional, backward compatible
- Default behavior preserved when not configured
- Existing tests continue to pass

### Data Structures

```rust
// src/config/mapreduce.rs
pub struct MapReduceWorkflowConfig {
    // ... existing fields ...

    /// Custom merge workflow for map agents → parent worktree
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_merge: Option<MergeWorkflow>,

    /// Custom merge workflow for parent worktree → original branch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge: Option<MergeWorkflow>,
}

// src/cook/execution/mapreduce/agent/lifecycle.rs
pub struct DefaultLifecycleManager {
    worktree_manager: Arc<WorktreeManager>,
    agent_merge_workflow: Option<MergeWorkflow>,
}

impl DefaultLifecycleManager {
    pub fn new(
        worktree_manager: Arc<WorktreeManager>,
        agent_merge_workflow: Option<MergeWorkflow>,
    ) -> Self {
        Self { worktree_manager, agent_merge_workflow }
    }
}
```

### APIs and Interfaces

**New Constructor Signature**:
```rust
// Before
DefaultLifecycleManager::new(worktree_manager: Arc<WorktreeManager>)

// After
DefaultLifecycleManager::new(
    worktree_manager: Arc<WorktreeManager>,
    agent_merge_workflow: Option<MergeWorkflow>,
)
```

**WorktreeManager Usage**:
```rust
// Already exists, just needs wiring
WorktreeManager::with_config(
    repo_path: PathBuf,
    subprocess: SubprocessManager,
    verbosity: u8,
    custom_merge_workflow: Option<MergeWorkflow>,
)
```

## Dependencies

**Prerequisites**: None

**Affected Components**:
- MapReduce executor
- Agent lifecycle manager
- WorktreeManager
- Configuration parsing
- Variable interpolation engine

**External Dependencies**: None (reuses existing infrastructure)

## Testing Strategy

### Unit Tests

1. **Configuration Parsing**
   - Test `agent_merge` array syntax parsing
   - Test `agent_merge` full config syntax (with timeout)
   - Test both `agent_merge` and `merge` together
   - Test optional nature of both fields
   - Verify backward compatibility

2. **Lifecycle Manager**
   - Test agent_merge workflow execution
   - Test variable interpolation in agent context
   - Test merge failure handling
   - Test DLQ on agent merge failure

3. **Final Merge**
   - Test merge workflow execution after reduce
   - Test merge variables availability
   - Test merge failure handling
   - Test job failure on merge error

### Integration Tests

1. **End-to-End MapReduce**
   - Run MapReduce with agent_merge cleanup
   - Verify agent changes merged correctly
   - Run MapReduce with final merge validation
   - Verify final merge to original branch

2. **Error Scenarios**
   - Agent merge fails → item goes to DLQ
   - Final merge fails → job fails
   - Merge workflow times out → appropriate error
   - Invalid merge workflow → parse error

3. **Real-World Workflows**
   - workflow-syntax-drift.yml with cleanup
   - Test-before-merge in agent_merge
   - CI validation in final merge

### Performance Tests

- Measure overhead of merge workflows
- Verify parallel agent merges not blocked
- Ensure no memory leaks in merge execution

## Documentation Requirements

### Code Documentation

- Document `agent_merge` field in MapReduceWorkflowConfig
- Document DefaultLifecycleManager constructor changes
- Add examples to struct docstrings
- Update variable interpolation docs

### User Documentation

**docs/workflow-syntax.md Updates**:

```yaml
# MapReduce Workflows section

# Agent merge: Map agents → parent worktree
agent_merge:
  - shell: "cargo test ${item.path}"  # Test just this agent's changes
    on_failure:
      claude: "/fix-agent-tests"
  - shell: "git add -A && git commit -m 'Agent ${worker.id} changes' || true"

# Final merge: Parent worktree → original branch
merge:
  - shell: "rm -rf .prodigy/syntax-analysis"  # Cleanup
  - shell: "git add -A && git commit -m 'chore: cleanup' || true"
  - shell: "git fetch origin"
  - claude: "/merge-master"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

**Variable Documentation**:
- Document agent_merge variables: `${item}`, `${worker.id}`, `${item_index}`, `${item_total}`
- Document merge variables: `${merge.worktree}`, `${merge.source_branch}`, `${merge.target_branch}`, `${merge.session_id}`

### Example Workflows

Create `examples/mapreduce-with-custom-merges.yml`:

```yaml
name: example-custom-merges
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"
  max_parallel: 3

# Validate each agent's changes before merging
agent_merge:
  - shell: "cargo test --package prodigy"
    on_failure:
      claude: "/debug-test-failure ${item}"
  - shell: "cargo clippy -- -D warnings"
    on_failure:
      claude: "/fix-lint-issues ${item}"

reduce:
  - claude: "/aggregate ${map.results}"

# Validate and cleanup before final merge
merge:
  - shell: "rm -rf .prodigy/temp-analysis"
  - shell: "git add -A && git commit -m 'cleanup temp files' || true"
  - shell: "git fetch origin"
  - claude: "/merge-master"
  - shell: "cargo test --all"
  - claude: "/ci"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

## Implementation Notes

### Variable Scoping

**Agent Merge Context** (executed in agent worktree):
- Has access to: `${item}`, `${worker.id}`, `${item_index}`, `${item_total}`
- Has access to: Standard variables like `${shell.output}`, `${claude.output}`
- Does NOT have: `${merge.*}` variables (wrong context)

**Final Merge Context** (executed in parent worktree):
- Has access to: `${merge.worktree}`, `${merge.source_branch}`, `${merge.target_branch}`, `${merge.session_id}`
- Has access to: `${map.total}`, `${map.successful}`, `${map.failed}`, `${map.results}`
- Does NOT have: `${item}` variables (no longer in item context)

### Merge Execution Flow

**Agent Merge Flow**:
1. Agent completes work item successfully
2. Create branch from agent's worktree state
3. Execute `agent_merge` workflow in agent's worktree
4. If workflow succeeds → git merge to parent
5. If workflow fails → send item to DLQ
6. Cleanup agent worktree

**Final Merge Flow**:
1. Reduce phase completes
2. Execute `merge` workflow in parent worktree
3. If workflow succeeds → proceed with merge to original branch
4. If workflow fails → fail entire job with error
5. Cleanup parent worktree on success

### Error Context

Agent merge failure error:
```
Failed to merge agent for item ${item.id}
Agent: ${worker.id}
Merge workflow failed at step: ${failed_step}
Error: ${error_message}
Item sent to DLQ for retry
```

Final merge failure error:
```
MapReduce job failed during final merge
Worktree: ${merge.worktree}
Source branch: ${merge.source_branch}
Target branch: ${merge.target_branch}
Merge workflow failed at step: ${failed_step}
Error: ${error_message}
```

## Migration and Compatibility

### Breaking Changes

**None** - Both fields are optional and additive.

### Migration Path

**Existing Workflows**:
- Continue to work without modification
- Can gradually add merge workflows as needed
- No forced migration required

**Adding Merge Workflows**:
1. Add `agent_merge:` section for per-agent validation
2. Add `merge:` section for final validation/cleanup
3. Test with dry-run first
4. Deploy to production workflows

### Compatibility Guarantees

- Default merge behavior unchanged
- Optional fields backward compatible
- Existing tests unaffected
- No configuration version bump needed

## Success Metrics

- [ ] All existing MapReduce tests pass
- [ ] New tests for both merge types pass
- [ ] workflow-syntax-drift.yml uses cleanup merge successfully
- [ ] Documentation includes clear examples
- [ ] No performance regression in MapReduce execution
- [ ] Error messages provide actionable context
