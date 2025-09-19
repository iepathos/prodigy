---
number: 103
title: Configurable Merge Workflow and Transparent Claude Logging
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-18
---

# Specification 103: Configurable Merge Workflow and Transparent Claude Logging

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Currently, when Prodigy merges workflows/worktrees, it executes a Claude command (`claude /prodigy-merge-worktree`) directly through the WorktreeManager. This execution has two limitations:

1. **Logging Transparency**: The Claude merge command doesn't inherit the same JSON streaming visibility as other workflow commands. While regular workflow commands respect verbosity settings and stream JSON output appropriately, the merge command bypasses the standard execution pipeline.

2. **Inflexible Merge Process**: The merge workflow is hardcoded to use the default `/prodigy-merge-worktree` command with no ability to customize the merge process. Organizations may need different merge strategies such as:
   - Merging the main branch into the worktree first to ensure compatibility
   - Running CI/CD checks before merging
   - Custom conflict resolution strategies
   - Post-merge validation steps
   - Different merge strategies for different workflow types

## Objective

Enable configurable merge workflows in Prodigy while ensuring the Claude merge command has the same logging transparency and JSON streaming capabilities as other workflow commands. This will provide users with full control over their merge process and maintain consistent observability across all Claude executions.

## Requirements

### Functional Requirements

1. **Transparent Claude Streaming**
   - Ensure the Claude merge command respects the same verbosity settings as other workflow commands
   - Stream JSON output when verbosity >= 1 or when `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true`
   - Use the same streaming infrastructure (`ClaudeJsonProcessor`) as regular workflow commands
   - Maintain consistent logging behavior across all Claude executions

2. **Configurable Merge Workflow**
   - Add a new `merge` workflow block to allow custom merge workflows
   - Support all standard workflow commands (claude, shell, goal_seek, foreach) in merge blocks
   - Provide access to merge-specific variables (worktree name, target branch, source branch)
   - Fall back to default `/prodigy-merge-worktree` command when no custom merge block is defined

3. **Merge Workflow Variables**
   - Expose merge context variables:
     - `${merge.worktree}` - Name of the worktree being merged
     - `${merge.source_branch}` - Source branch (worktree branch)
     - `${merge.target_branch}` - Target branch (usually main/master)
     - `${merge.session_id}` - Session ID for correlation
   - Support all standard workflow variable interpolation

### Non-Functional Requirements

1. **Backward Compatibility**
   - Existing workflows without merge blocks must continue to work with default behavior
   - No breaking changes to current merge functionality

2. **Performance**
   - Streaming infrastructure must not significantly impact merge performance
   - Maintain efficient execution for both default and custom merge workflows

3. **Error Handling**
   - Clear error messages when merge workflows fail
   - Proper cleanup on merge failure
   - Maintain transactional integrity of merge operations

## Acceptance Criteria

- [ ] Claude merge command uses the same `ClaudeStreamingExecutor` as regular workflow commands
- [ ] JSON streaming is visible when verbosity >= 1 during merge operations
- [ ] `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true` enables streaming output during merge
- [ ] New `merge` workflow block is parsed and validated correctly
- [ ] Custom merge workflows execute successfully with proper variable interpolation
- [ ] Default `/prodigy-merge-worktree` is used when no merge block is defined
- [ ] Merge-specific variables are available and correctly interpolated
- [ ] Error handling provides clear feedback for merge workflow failures
- [ ] Documentation is updated with merge workflow examples
- [ ] Tests cover both default and custom merge scenarios
- [ ] Integration tests verify streaming output during merge operations

## Technical Details

### Implementation Approach

1. **Refactor Merge Command Execution**
   - Move from direct subprocess execution to using `ClaudeStreamingExecutor`
   - Pass verbosity and environment settings to the executor
   - Ensure event logging is properly configured for merge operations

2. **Add Merge Workflow Support**
   - Extend workflow YAML schema to include optional `merge` block
   - Implement merge workflow parser and validator
   - Create merge-specific variable context
   - Execute custom merge workflows through the standard orchestrator

3. **Streaming Infrastructure Integration**
   - Reuse existing `ClaudeJsonProcessor` and streaming handlers
   - Ensure merge operations respect verbosity settings
   - Maintain consistent logging patterns across all Claude executions

### Architecture Changes

1. **WorktreeManager Updates**
   - Replace direct subprocess execution with `ClaudeStreamingExecutor`
   - Check for custom merge workflow before falling back to default
   - Pass appropriate configuration for streaming and logging

2. **Workflow Configuration**
   - Add `MergeWorkflow` configuration type
   - Extend `WorkflowConfig` to include optional merge block
   - Implement merge workflow validation

3. **Execution Pipeline**
   - Create `MergeWorkflowExecutor` to handle custom merge workflows
   - Integrate with existing orchestrator for command execution
   - Ensure proper variable context for merge operations

### Data Structures

```rust
pub struct MergeWorkflow {
    pub commands: Vec<Command>,
}

pub struct MergeContext {
    pub worktree: String,
    pub source_branch: String,
    pub target_branch: String,
    pub session_id: String,
}

// Extended WorkflowConfig
pub struct WorkflowConfig {
    // ... existing fields ...
    pub merge: Option<MergeWorkflow>,
}
```

### APIs and Interfaces

```yaml
# Workflow YAML with custom merge block
name: feature-workflow
commands:
  - claude: "/implement feature"
  - shell: "cargo test"

# Custom merge workflow
merge:
  - shell: "git fetch origin"
  - shell: "git merge origin/main"  # Merge main into worktree first
  - shell: "cargo test"              # Run tests
  - shell: "cargo clippy"            # Run linting
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
  - shell: "echo 'Successfully merged ${merge.worktree}'"
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - WorktreeManager (merge execution)
  - ClaudeStreamingExecutor (streaming support)
  - WorkflowConfig (schema extension)
  - Orchestrator (merge workflow execution)
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test merge workflow parsing and validation
  - Verify variable interpolation in merge context
  - Test streaming configuration for merge operations

- **Integration Tests**:
  - Test default merge behavior without custom workflow
  - Test custom merge workflow execution
  - Verify streaming output with different verbosity levels
  - Test merge failure scenarios and error handling

- **Performance Tests**:
  - Measure streaming overhead during merge operations
  - Verify no performance regression with custom merge workflows

- **User Acceptance**:
  - Manual testing of various merge scenarios
  - Verification of streaming output visibility
  - Testing of complex custom merge workflows

## Documentation Requirements

- **Code Documentation**:
  - Document merge workflow configuration options
  - Add inline documentation for merge-specific variables
  - Document streaming behavior for merge operations

- **User Documentation**:
  - Add merge workflow section to CLAUDE.md
  - Provide examples of common merge workflow patterns
  - Document merge-specific variables and their usage
  - Explain verbosity settings and streaming output control

- **Architecture Updates**:
  - Update ARCHITECTURE.md to reflect merge workflow support
  - Document the integration with streaming infrastructure
  - Add sequence diagrams for custom merge workflow execution

## Implementation Notes

1. **Phased Implementation**:
   - Phase 1: Fix Claude streaming transparency for merge operations
   - Phase 2: Add basic merge workflow configuration support
   - Phase 3: Implement full variable interpolation and advanced features

2. **Error Recovery**:
   - Ensure worktree state is properly tracked during merge failures
   - Provide rollback capabilities for failed custom merge workflows
   - Maintain clear audit trail of merge operations

3. **Future Enhancements**:
   - Support for merge hooks (pre-merge, post-merge)
   - Conditional merge strategies based on workflow metadata
   - Integration with external CI/CD systems for merge validation

## Migration and Compatibility

- No migration required for existing workflows
- Default behavior remains unchanged for workflows without merge blocks
- New functionality is purely additive
- Streaming improvements apply automatically to all Claude merge operations