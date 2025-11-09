# Troubleshooting

This chapter provides comprehensive guidance for diagnosing and resolving common issues with Prodigy workflows. Whether you're experiencing MapReduce failures, checkpoint issues, or variable interpolation problems, you'll find practical solutions here.

## Common Issues

### Variables not interpolating

**Symptoms:** Literal `${var}` appears in output instead of value

**Causes:**
- Variable name typo or case mismatch
- Variable not in scope
- Incorrect syntax
- Variable not captured

**Solutions:**
- Check variable name spelling and case sensitivity
- Verify variable is available in current scope (step vs workflow)
- Ensure proper syntax: `${var}` not `$var` for complex expressions
- Verify capture_output command succeeded
- Check variable was set before use (e.g., in previous step)

### MapReduce items not found

**Symptoms:** No items to process, empty JSONPath result, or "items.json not found"

**Causes:**
- Input file doesn't exist
- Incorrect JSONPath
- Setup phase failed
- Wrong file format

**Solutions:**
- Verify input file exists with correct path
- Test JSONPath expression with jsonpath-cli or jq
- Check json_path field syntax (default: `$[*]`)
- Ensure setup phase generated the input file successfully
- Validate JSON format with jq or json validator

### Timeout errors

**Symptoms:** Commands or phases timing out before completion

**Causes:**
- Operation too slow
- Insufficient timeout
- Hung processes
- Deadlock

**Solutions:**
- Increase timeout value for long operations
- Optimize command execution for better performance
- Split work into smaller chunks (use max_items, offset)
- Check for hung processes with ps or top
- Look for deadlocks in concurrent operations
- Use agent_timeout_secs for MapReduce agents

### Checkpoint resume not working

**Symptoms:** Resume starts from beginning, fails to load state, or "checkpoint not found"

**Causes:**
- Checkpoint files missing
- Wrong session/job ID
- Workflow changed
- Concurrent resume

**Solutions:**
- Verify checkpoint files exist in `~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/`
- Check session/job ID is correct with `prodigy sessions list`
- Ensure workflow file hasn't changed significantly
- Check for concurrent resume lock in `~/.prodigy/resume_locks/`
- Review checkpoint file contents for corruption

See [MapReduce Checkpoint and Resume](../mapreduce/checkpoint-and-resume.md) for detailed information.

### DLQ items not retrying or re-failing

**Symptoms:** Retry command fails, items immediately fail again, or no progress

**Causes:**
- Systematic error not transient
- DLQ file corrupted
- Underlying issue not fixed

**Solutions:**
- Check DLQ file format and contents with `prodigy dlq show <job_id>`
- Verify error was transient not systematic (e.g., rate limit vs bug)
- Fix underlying issue before retry (e.g., API credentials, file permissions)
- Increase max-parallel for retry if parallelism helps
- Check json_log_location in DLQ for detailed error info

See [Dead Letter Queue](../mapreduce/dead-letter-queue.md) for complete DLQ management details.

### Worktree cleanup failures

**Symptoms:** Orphaned worktrees after failures, "permission denied" on cleanup

**Causes:**
- Locked files
- Running processes
- Permission issues
- Disk full

**Solutions:**
- Use `prodigy worktree clean-orphaned <job_id>` for automatic cleanup
- Check for locked files with lsof or similar tools
- Verify no running processes using worktree with ps
- Check disk space with `df -h`
- Verify file permissions on worktree directory
- Manual cleanup if necessary: `rm -rf ~/.prodigy/worktrees/<path>`

See [Cleanup Failure Handling](../mapreduce/cleanup-failure-handling.md) for detailed cleanup guidance.

### Environment variables not resolved

**Symptoms:** Literal `${VAR}` or `$VAR` appears in commands instead of value

**Causes:**
- Variable not defined
- Wrong profile
- Scope issue
- Syntax error

**Solutions:**
- Check variable defined in env, secrets, or profiles section
- Verify correct profile activated with --profile flag
- Use proper syntax: `${VAR}` for workflow vars, `$VAR` may work for shell
- Check variable scope (global vs step-level)
- Ensure env_files loaded correctly

See [Environment Variables](../advanced-features/environment-variables.md) for variable configuration details.

### Git context variables empty

**Symptoms:** `${step.files_added}` returns empty string or undefined

**Causes:**
- No commits created
- Git repo not initialized
- Step not completed
- Wrong format

**Solutions:**
- Ensure commands created commits (use `commit_required: true`)
- Check git repository is initialized in working directory
- Verify step completed before accessing variables
- Use appropriate format modifier (e.g., :json, :newline)
- Check git status to verify changes exist

See [Git Context Variables](../advanced-features/git-context-variables.md) for available git variables.

### Claude command fails with "command not found"

**Symptoms:** Shell error about claude command not existing

**Causes:**
- Claude Code not installed
- Not in PATH
- Wrong executable name

**Solutions:**
- Install Claude Code CLI if not present
- Verify claude is in PATH with `which claude`
- Check command name matches Claude Code CLI (not "claude-code")
- Use full path if necessary: `/path/to/claude`

## Debug Tips

### Use verbose mode for execution details

```bash
prodigy run workflow.yml -v
```

**Shows:** Claude streaming output, tool invocations, and execution timeline

**Use when:** Understanding what Claude is doing, debugging tool calls

### Check Claude JSON logs for full interaction

```bash
prodigy logs --latest --summary
```

**Shows:** Full Claude interaction including messages, tools, token usage, errors

**Use when:** Claude command failed, understanding why Claude made certain decisions

See [Claude JSON Logs](../observability/claude-json-logs.md) for detailed log analysis.

### Inspect event logs for execution timeline

```bash
prodigy events list <job_id>
```

**Shows:** Detailed execution timeline, agent starts/completions, durations

**Use when:** Understanding workflow execution flow, finding bottlenecks

### Review DLQ for failed item details

```bash
prodigy dlq show <job_id>
```

**Shows:** Failed items with full error details, retry history, json_log_location

**Use when:** MapReduce items failing, understanding failure patterns

### Check checkpoint state for resume issues

**Location:** `~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/`

**Shows:** Saved execution state, completed items, variables, phase progress

**Use when:** Resume not working, understanding saved state

### Examine worktree git log for commits

```bash
cd ~/.prodigy/worktrees/{repo}/{session}/ && git log
```

**Shows:** All commits created during workflow execution with full details

**Use when:** Understanding what changed, verifying commits created

### Tail Claude JSON log in real-time

```bash
prodigy logs --latest --tail
```

**Shows:** Live streaming of Claude JSON log as it's being written

**Use when:** Watching long-running Claude command, debugging in real-time

## Additional Topics

For more specific troubleshooting guidance, see:
- [FAQ](faq.md) - Frequently asked questions
- [Common Error Messages](common-error-messages.md) - Specific error messages explained
- [Best Practices for Debugging](best-practices-for-debugging.md) - Proven debugging strategies
