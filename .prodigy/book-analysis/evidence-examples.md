# Evidence for Examples Chapter

## Source Definitions Found

### Environment Variables and Secrets
- EnvironmentConfig struct: src/cook/environment/config.rs:12-36
- EnvValue enum: src/cook/environment/config.rs:38-60
- SecretValue enum: src/cook/environment/config.rs:84-96
- EnvProfile struct: Referenced in tests/environment_workflow_test.rs:71-77

### Merge Workflows
- MergeWorkflow struct: src/config/mapreduce.rs:84-94
- Merge variables: ${merge.worktree}, ${merge.source_branch}, ${merge.target_branch}, ${merge.session_id}
- MergeWorkflow test: tests/merge_workflow_integration.rs:64-121

### Git Context Variables
- GitChangeTracker struct: src/cook/workflow/git_context.rs:86-99
- StepChanges struct: src/cook/workflow/git_context.rs:101-116
- Variable resolution: src/cook/workflow/git_context.rs:36-42
- Supported variables: files_added, files_modified, files_deleted, commits, insertions, deletions
- Format modifiers: :json, :*.rs (glob patterns)

### Timeout Configuration
- TimeoutConfig struct: src/cook/execution/mapreduce/timeout.rs:38-63
- Fields: agent_timeout_secs, command_timeouts, timeout_policy, cleanup_grace_period_secs
- Default timeout: 600 seconds (10 minutes) - src/cook/execution/mapreduce/timeout.rs:68

## Test Examples Found
- Environment profiles test: tests/environment_workflow_test.rs:63-132
- Environment inheritance test: tests/environment_workflow_test.rs:135-150
- Merge workflow end-to-end test: tests/merge_workflow_integration.rs:64-121
- Merge workflow with failures test: tests/merge_workflow_integration.rs:124-150

## Configuration Examples Found
- MapReduce environment example: workflows/mapreduce-env-example.yml:1-95
  - env block with variables: lines 7-20
  - secrets block with provider: lines 23-26
  - profiles with env vars: lines 29-40
  - merge phase with env vars: lines 83-94
- Documentation drift workflow: workflows/documentation-drift.yml:1-81
  - Timeout usage: line 48
  - max_attempts usage: lines 23, 46, 52
  - commit_required usage: lines 19, 23, 27, 51, 59

## Validation Results
✓ Environment variables go in 'env' block (not separate 'secrets' block)
✓ Secrets are part of env with 'secret: true' flag OR separate 'secrets' block
✓ Git context variables ARE implemented (files_added, files_modified, etc.)
✓ Merge workflow supports merge-specific variables
✓ Timeout config only supports agent_timeout_secs (not item/phase timeouts)
✓ max_attempts is at step level, not inside goal_seek
✗ No workflow composition examples found (feature may not be implemented)

## Discovery Notes
- Test directories: ./tests, ./workflows/tests, ./src/testing
- Example directories: ./workflows, ./examples
- Source directories: ./src/cook/environment, ./src/cook/workflow, ./src/config

## Key Findings
1. **Secrets Syntax**: Example 7 is PARTIALLY CORRECT - both formats supported:
   - Secrets in env block with secret:true (PREFERRED per drift report)
   - Separate secrets block also supported (SecretValue in mapreduce.rs:29-30)
2. **Git Context**: FULLY IMPLEMENTED - should move from "Future" to working examples
3. **Merge Workflows**: FULLY IMPLEMENTED - merge block with commands and timeout
4. **Timeout Config**: Only agent_timeout_secs is documented; item/phase timeouts commented as unimplemented
5. **Goal Seek**: max_attempts is workflow-level, not in goal_seek block
