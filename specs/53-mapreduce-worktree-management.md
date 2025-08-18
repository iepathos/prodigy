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

The current MapReduce implementation creates isolated git worktrees for each parallel agent but immediately cleans them up after task completion. This prevents the proper aggregation of work. When using `--worktree` with MapReduce workflows, we need a two-phase merge strategy:

**Phase 1: Progressive Agent Merging**
- Each agent completes work in its own worktree/branch
- As agents complete successfully, immediately merge to parent worktree
- Parent worktree accumulates all changes progressively
- Agent worktrees are cleaned up after successful merge

**Phase 2: Final Integration**
- After all agents complete and merge to parent
- Execute reduce phase commands in parent worktree
- If `-y` flag is set, merge parent worktree to main/master
- Single merge commit to main with all accumulated changes

Currently, line 491-495 in `mapreduce.rs` shows premature cleanup:
```rust
// Clean up worktree (in real implementation, might keep for reduce phase)
self.worktree_manager.cleanup_session(&worktree_name, true).await?;
```

This prevents the intended workflow where all parallel changes are aggregated into a single, reviewable set of commits.

## Objective

Implement a two-phase merge strategy for MapReduce workflows that:
1. Progressively merges successful agents to parent worktree using `/mmm-merge-worktree`
2. Cleans up agent worktrees immediately after successful merge
3. Accumulates all changes in parent worktree branch
4. Handles merge conflicts gracefully during agent merging
5. Provides single integration point to main/master with `-y` flag
6. Maintains clear git history and rollback capability

## Requirements

### Functional Requirements

1. **Worktree Lifecycle Management**
   - Create parent orchestrator worktree for MapReduce session
   - Create child worktrees for each parallel agent
   - Maintain agent worktrees only until successful merge to parent
   - Track worktree metadata in AgentResult for merge operation
   - Clean up agent worktrees progressively as they complete
   - Clean up parent worktree after final merge or on failure

2. **Branch Management**
   - Create unique branches for each agent: `mmm-agent-{session_id}-{item_id}`
   - Track branch names in AgentResult
   - Preserve branch history for audit trail
   - Support branch naming customization

3. **Two-Phase Merge Strategy**
   - **Phase 1**: Progressive merge of each agent to parent as they complete
   - Use existing `/mmm-merge-worktree` command for agent→parent merges
   - Validate parent state after each agent merge (run tests)
   - **Phase 2**: Single merge from parent to main/master
   - Always use --no-ff to preserve full history
   - Support for PR creation when `-y` not specified

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

- [ ] Agent worktrees persist only until merge to parent
- [ ] AgentResult includes branch name for merge operation
- [ ] Each successful agent merges to parent via `/mmm-merge-worktree`
- [ ] Parent worktree accumulates all agent changes progressively
- [ ] Agent worktrees cleaned up immediately after successful merge
- [ ] Merge conflicts during agent→parent are handled by mmm-merge-worktree
- [ ] Reduce phase runs in parent worktree with all accumulated changes
- [ ] Auto-merge (`-y`) creates single merge commit to main/master
- [ ] Without `-y`, parent branch ready for PR creation
- [ ] Failed agent merges don't block other agents
- [ ] Integration test with 20 parallel agents succeeds
- [ ] Final merge to main shows all agent commits in history
- [ ] Documentation explains two-phase merge strategy

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
   b. For each agent (parallel):
      - Create child worktree: mmm-session-{id}-{item_id}
      - Create branch: mmm-agent-{session_id}-{item_id}
      - Execute commands and commit
      - On success:
        * Switch to parent worktree
        * Execute: /mmm-merge-worktree mmm-agent-{session_id}-{item_id}
        * Validate merge (run tests in parent)
        * Clean up agent worktree
      - On failure:
        * Mark agent as failed
        * Keep worktree for debugging (optional)

2. REDUCE PHASE:
   a. All successful agents already merged to parent
   b. Execute reduce commands in parent worktree
   c. Parent branch now contains all changes
   d. Create summary commit if needed

3. FINAL MERGE:
   a. If -y flag:
      - Switch to main/master
      - Execute: /mmm-merge-worktree mmm-session-{id}
      - Clean up parent worktree
   b. Without -y:
      - Report parent branch ready for review
      - Suggest PR creation command
      - Keep parent worktree for manual inspection
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
        // Create worktree for agent
        let worktree_session = self.worktree_manager.create_session().await?;
        let branch_name = format!("mmm-agent-{}-{}", env.session_id, item_id);
        
        // Create and checkout branch
        self.create_agent_branch(&worktree_session, &branch_name).await?;
        
        // Execute commands...
        let result = self.run_agent_commands(template_steps).await?;
        
        // If successful, merge to parent immediately
        if result.success {
            self.merge_agent_to_parent(&branch_name, &env.parent_worktree).await?;
            self.worktree_manager.cleanup_session(&worktree_session.id, true).await?;
        }
        
        // Return result
        Ok(AgentResult {
            branch_name: Some(branch_name),
            merged_to_parent: result.success,
            // ... other fields
        })
    }
    
    async fn merge_agent_to_parent(
        &self,
        agent_branch: &str,
        parent_worktree: &WorktreeSession,
    ) -> Result<()> {
        // Switch to parent worktree
        self.switch_to_worktree(parent_worktree).await?;
        
        // Use mmm-merge-worktree command
        let output = self.execute_command(
            "/mmm-merge-worktree",
            agent_branch,
        ).await?;
        
        if !output.success {
            return Err(anyhow!("Failed to merge agent: {}", output.error));
        }
        
        // Validate parent state (optional)
        self.validate_parent_state().await?;
        
        Ok(())
    }
    
    async fn execute_reduce_phase(
        &self,
        reduce_phase: &ReducePhase,
        map_results: &[AgentResult],
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        // All agents already merged to parent progressively
        let successful_count = map_results.iter()
            .filter(|r| r.merged_to_parent)
            .count();
        
        self.user_interaction.display_info(&format!(
            "All {} successful agents merged to parent worktree",
            successful_count
        ));
        
        // Execute reduce commands in parent worktree
        self.switch_to_worktree(&env.parent_worktree).await?;
        for command in &reduce_phase.commands {
            self.execute_command(command).await?;
        }
        
        // Final merge to main if -y flag set
        if env.auto_merge {
            self.merge_parent_to_main(&env.parent_worktree).await?;
            self.worktree_manager.cleanup_session(
                &env.parent_worktree.id,
                true
            ).await?;
        } else {
            self.user_interaction.display_info(&format!(
                "Parent worktree ready for review: {}\n",
                "To create PR: git push origin {} && gh pr create",
                env.parent_worktree.branch_name
            ));
        }
        
        Ok(())
    }
    
    async fn merge_parent_to_main(
        &self,
        parent_worktree: &WorktreeSession,
    ) -> Result<()> {
        // Switch to main/master
        self.switch_to_main_branch().await?;
        
        // Use mmm-merge-worktree for final merge
        let output = self.execute_command(
            "/mmm-merge-worktree",
            &parent_worktree.branch_name,
        ).await?;
        
        if !output.success {
            return Err(anyhow!("Failed to merge to main: {}", output.error));
        }
        
        self.user_interaction.display_success(
            "Successfully merged all changes to main/master"
        );
        
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

### Phase 1: Progressive Agent Merging (Day 1-2)
- Implement merge_agent_to_parent using /mmm-merge-worktree
- Add progressive cleanup of agent worktrees
- Track merge status in AgentResult

### Phase 2: Parent Worktree Management (Day 3)
- Ensure parent worktree accumulates all changes
- Validate parent state after each merge
- Handle failed agent merges gracefully

### Phase 3: Final Integration (Day 4)
- Implement merge_parent_to_main for -y flag
- Support PR creation workflow without -y
- Clean up parent worktree after final merge

### Phase 4: Testing & Polish (Day 5)
- Test with 20+ parallel agents
- Verify git history preservation
- Document two-phase merge strategy

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