---
number: 53
title: MapReduce Worktree Management and Branch Merging
category: parallel
priority: critical
status: draft
dependencies: [49, 51]
created: 2025-08-18
---

# Specification 53: MapReduce Worktree Management and Branch Merging

**Category**: parallel
**Priority**: critical
**Status**: draft
**Dependencies**: [49 - MapReduce Parallel Execution, 51 - Command Execution Integration]

## Context

The current MapReduce implementation creates isolated git worktrees for each parallel agent but immediately cleans them up after task completion. This prevents the proper aggregation of work in the reduce phase. When using `--worktree` with MapReduce workflows, we need a hierarchical worktree structure where:

1. A parent orchestrator worktree manages the overall workflow
2. Child agent worktrees perform isolated parallel work
3. The reduce phase merges all agent branches into the parent
4. The `-y` flag auto-merges the parent back to main/master

Currently, line 491-495 in `mapreduce.rs` shows premature cleanup:
```rust
// Clean up worktree (in real implementation, might keep for reduce phase)
self.worktree_manager.cleanup_session(&worktree_name, true).await?;
```

This prevents the intended workflow where all parallel changes are aggregated into a single, reviewable set of commits.

## Objective

Implement proper worktree lifecycle management for MapReduce workflows that:
1. Maintains agent worktrees through the map phase
2. Tracks agent branches for merging in reduce phase
3. Implements intelligent merge strategies for combining parallel work
4. Handles merge conflicts gracefully
5. Integrates with the `-y` auto-merge flag for final integration
6. Provides clear visibility into the merge process

## Requirements

### Functional Requirements

1. **Worktree Lifecycle Management**
   - Create parent orchestrator worktree for MapReduce session
   - Create child worktrees for each parallel agent
   - Maintain worktrees until reduce phase completion
   - Track worktree metadata in AgentResult
   - Clean up all worktrees after successful merge

2. **Branch Management**
   - Create unique branches for each agent: `mmm-agent-{session_id}-{item_id}`
   - Track branch names in AgentResult
   - Preserve branch history for audit trail
   - Support branch naming customization

3. **Merge Strategies**
   - Sequential merge of all agent branches
   - Octopus merge for combining multiple branches at once
   - Support different merge strategies (--no-ff, --ff-only, --squash)
   - Configurable conflict resolution strategies

4. **Conflict Resolution**
   - Detect merge conflicts early
   - Support multiple resolution strategies:
     - `fail_on_conflict` (default) - Stop on any conflict
     - `ours` - Keep parent worktree version
     - `theirs` - Keep agent version
     - `union` - Combine both (for additions only)
     - `claude` - Use Claude to resolve conflicts
   - Generate conflict reports for manual review

5. **Auto-merge Integration**
   - When `-y` flag is set, merge parent worktree to main/master
   - Validate merge compatibility before attempting
   - Handle main branch protection rules
   - Support dry-run mode to preview changes

6. **Progress and Visibility**
   - Show merge progress in real-time
   - Display commit graph visualization
   - Report which files were modified by which agents
   - Generate merge summary report

### Non-Functional Requirements

1. **Performance**
   - Efficient branch switching and merging
   - Minimize disk I/O during merge operations
   - Parallel fetch of agent branches where possible

2. **Safety**
   - Never lose committed work from agents
   - Atomic merge operations (all or nothing)
   - Rollback capability on merge failure

3. **Observability**
   - Clear logging of all git operations
   - Traceable merge history
   - Debugging information for conflicts

## Acceptance Criteria

- [ ] Agent worktrees persist through map phase execution
- [ ] AgentResult includes branch name and worktree path
- [ ] Reduce phase successfully merges all agent branches
- [ ] Merge conflicts are detected and reported clearly
- [ ] Conflict resolution strategies work as configured
- [ ] Auto-merge (`-y`) integrates all changes to main/master
- [ ] Clean merge with no conflicts completes in <5s for 10 agents
- [ ] All worktrees are cleaned up after successful completion
- [ ] Failed merges preserve agent branches for manual recovery
- [ ] Integration test with 20 parallel agents succeeds
- [ ] Merge commit messages clearly indicate source branches
- [ ] Documentation includes merge strategy examples

## Technical Details

### Implementation Approach

1. Modify MapReduceExecutor to track worktree lifecycle
2. Enhance AgentResult to include branch information
3. Implement merge orchestration in reduce phase
4. Add conflict detection and resolution logic
5. Integrate with existing WorktreeManager

### Architecture Changes

```rust
// Enhanced AgentResult with branch tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub item_id: String,
    pub status: AgentStatus,
    pub output: Option<String>,
    pub commits: Vec<String>,
    pub duration: Duration,
    pub error: Option<String>,
    // New fields for worktree management
    pub worktree_path: Option<PathBuf>,
    pub branch_name: Option<String>,
    pub worktree_session_id: Option<String>,
    pub files_modified: Vec<PathBuf>,
}

// Merge configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConfig {
    pub strategy: MergeStrategy,
    pub conflict_resolution: ConflictResolution,
    pub merge_message_template: String,
    pub preserve_agent_branches: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStrategy {
    NoFastForward,  // --no-ff (default)
    FastForwardOnly, // --ff-only
    Squash,         // --squash
    Octopus,        // merge all at once
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictResolution {
    FailOnConflict,
    KeepOurs,
    KeepTheirs,
    Union,
    Claude { prompt: String },
}

// Worktree coordinator for managing hierarchy
pub struct WorktreeCoordinator {
    parent_session: WorktreeSession,
    agent_sessions: Vec<WorktreeSession>,
    merge_config: MergeConfig,
}

impl WorktreeCoordinator {
    pub async fn merge_agent_branches(&self) -> Result<MergeResult>;
    pub async fn merge_to_main(&self, auto_confirm: bool) -> Result<()>;
    pub async fn cleanup_all(&self, force: bool) -> Result<()>;
}
```

### Data Flow

```
1. MAP PHASE:
   a. Create parent worktree with branch: mmm-session-{id}
   b. For each agent:
      - Create child worktree: mmm-session-{id}-{item_id}
      - Create branch: mmm-agent-{session_id}-{item_id}
      - Execute commands and commit
      - Store branch info in AgentResult
      - Keep worktree alive

2. REDUCE PHASE:
   a. Switch parent worktree to integration branch
   b. For each successful agent:
      - Fetch agent branch
      - Attempt merge with configured strategy
      - Handle conflicts if any
   c. Create merge commit with summary
   d. Execute reduce commands
   e. Finalize integration branch

3. AUTO-MERGE (if -y):
   a. Switch to main/master
   b. Merge integration branch
   c. Push if configured
   d. Cleanup all worktrees
```

### APIs and Interfaces

```yaml
# Enhanced MapReduce workflow configuration
name: parallel-debt-elimination
mode: mapreduce

map:
  # ... existing configuration ...
  
  # Worktree management
  worktree_config:
    preserve_branches: false  # Keep agent branches after merge
    branch_prefix: "fix"      # Creates fix-{item_id} branches
    
reduce:
  # ... existing commands ...
  
  # Merge configuration
  merge_config:
    strategy: "no-ff"        # Preserve commit history
    conflict_resolution: "fail_on_conflict"
    merge_message: "feat: merge ${map.successful} parallel improvements"
    
    # Optional Claude-assisted conflict resolution
    on_conflict:
      claude: "/resolve-merge-conflict --file ${conflict.file} --ours ${conflict.ours} --theirs ${conflict.theirs}"
      
  # Post-merge validation
  validate:
    - shell: "cargo test --all"
    - shell: "cargo clippy -- -D warnings"
```

```rust
// Integration with MapReduceExecutor
impl MapReduceExecutor {
    async fn execute_agent_commands(
        &self,
        item_id: &str,
        item: &Value,
        template_steps: &[WorkflowStep],
        env: &ExecutionEnvironment,
    ) -> Result<AgentResult> {
        // Create worktree but don't cleanup
        let worktree_session = self.worktree_manager.create_session().await?;
        let branch_name = format!("mmm-agent-{}-{}", env.session_id, item_id);
        
        // Create and checkout branch
        self.create_agent_branch(&worktree_session, &branch_name).await?;
        
        // Execute commands...
        
        // Return result with worktree info
        Ok(AgentResult {
            worktree_path: Some(worktree_session.path),
            branch_name: Some(branch_name),
            worktree_session_id: Some(worktree_session.id),
            // ... other fields
        })
    }
    
    async fn execute_reduce_phase(
        &self,
        reduce_phase: &ReducePhase,
        map_results: &[AgentResult],
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        // Create worktree coordinator
        let coordinator = WorktreeCoordinator::new(
            env.worktree_session.clone(),
            map_results.iter().filter_map(|r| r.worktree_session_id.clone()).collect(),
            self.merge_config.clone(),
        );
        
        // Merge all agent branches
        let merge_result = coordinator.merge_agent_branches().await?;
        
        // Execute reduce commands with merge context
        // ...
        
        // Auto-merge to main if requested
        if env.auto_merge {
            coordinator.merge_to_main(true).await?;
        }
        
        // Cleanup all worktrees
        coordinator.cleanup_all(false).await?;
        
        Ok(())
    }
}
```

## Dependencies

- **Prerequisites**: 
  - Spec 49 (MapReduce base implementation)
  - Spec 51 (Command execution integration)
- **Affected Components**:
  - `src/cook/execution/mapreduce.rs` - Main changes
  - `src/worktree/manager.rs` - Enhanced lifecycle management
  - `src/git/` - Additional git operations for merging
- **External Dependencies**: None (uses git CLI)

## Testing Strategy

- **Unit Tests**:
  - Worktree lifecycle tracking
  - Branch name generation
  - Merge strategy selection
  - Conflict detection logic

- **Integration Tests**:
  - 10 parallel agents with clean merge
  - 5 agents with deliberate conflicts
  - Auto-merge to main branch
  - Cleanup verification
  - Recovery from failed merge

- **Performance Tests**:
  - Merge performance with 50+ branches
  - Large file merge scenarios
  - Conflict resolution performance

- **Chaos Tests**:
  - Kill agent during execution
  - Network failure during merge
  - Disk full during worktree creation

## Documentation Requirements

- **Code Documentation**:
  - Worktree hierarchy explanation
  - Merge strategy details
  - Conflict resolution examples

- **User Documentation**:
  - MapReduce with worktrees guide
  - Merge strategy selection
  - Troubleshooting merge conflicts
  - Recovery procedures

- **Architecture Updates**:
  - Worktree hierarchy diagram
  - Merge flow visualization
  - State management documentation

## Implementation Notes

### Phase 1: Worktree Persistence (Day 1-2)
- Modify agent cleanup behavior
- Add branch tracking to AgentResult
- Implement worktree coordinator

### Phase 2: Merge Implementation (Day 3-4)
- Basic sequential merge
- Conflict detection
- Merge strategy support

### Phase 3: Conflict Resolution (Day 5)
- Resolution strategies
- Claude integration for conflicts
- Conflict reporting

### Phase 4: Auto-merge Integration (Day 6)
- Integration with `-y` flag
- Main branch merge
- Final cleanup

### Key Considerations

1. **Git Operations**: Use git plumbing commands for reliability
2. **Branch Naming**: Ensure unique, traceable branch names
3. **Atomic Operations**: All-or-nothing merge behavior
4. **Error Recovery**: Preserve branches on failure for manual recovery
5. **Performance**: Consider shallow clones for large repos

## Migration and Compatibility

- **Breaking Changes**: None - enhances existing behavior
- **Migration Path**: Existing MapReduce workflows gain merge capability automatically
- **Compatibility**: Works with all existing worktree features
- **Rollback**: Can disable with configuration flag if needed

## Success Metrics

- Zero data loss from parallel agent work
- 95% of merges complete without conflicts
- <10s merge time for 20 parallel agents
- Clear merge history in git log
- Successful integration with CI/CD pipelines