## Common Error Messages

This section documents specific error messages you may encounter while using Prodigy, along with their meanings and solutions.

### "checkpoint not found"

**Full message**: `Error: Checkpoint not found for session/job {id}`

**What it means**: Prodigy cannot locate checkpoint files needed to resume execution.

**Causes**:
- Session or job ID is incorrect
- Checkpoint files were deleted or moved
- Wrong repository context
- Checkpoint never created (workflow didn't reach checkpoint phase)

**Solutions**:
1. Verify the correct ID: `prodigy sessions list` or `prodigy resume-job list`
2. Check checkpoint directory: `~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/`
3. Ensure you're in the correct git repository
4. Start fresh if checkpoint is unrecoverable

### "items.json not found"

**Full message**: `Error: Input file not found: items.json`

**What it means**: The MapReduce input file specified in the workflow doesn't exist.

**Causes**:
- Setup phase failed to create the file
- Wrong file path in workflow configuration
- File created in wrong directory
- File path is relative but CWD is incorrect

**Solutions**:
1. Check setup phase output for errors
2. Verify `input:` path in workflow YAML
3. Ensure file path is correct (relative to workflow directory)
4. Run setup phase manually to debug file creation

### "command not found: claude"

**Full message**: `bash: claude: command not found`

**What it means**: The Claude Code CLI executable is not found in the system PATH.

**Causes**:
- Claude Code not installed
- Installation directory not in PATH
- Wrong executable name in workflow
- Shell environment not configured

**Solutions**:
1. Install Claude Code if not present
2. Verify installation: `which claude`
3. Add Claude Code to PATH if needed
4. Use full path in workflow: `/usr/local/bin/claude`

### "permission denied"

**Full message**: `Error: Permission denied: {path}` or `rm: cannot remove '{path}': Permission denied`

**What it means**: Insufficient permissions to access or modify a file/directory.

**Causes**:
- File/directory owned by different user
- Read-only filesystem
- Locked files or directories
- Insufficient user permissions

**Solutions**:
1. Check file ownership: `ls -l {path}`
2. Verify permissions: `ls -ld {directory}`
3. Check for locked files: `lsof {path}`
4. Run with appropriate permissions or fix ownership
5. For worktree cleanup: Use `prodigy worktree clean-orphaned`

### "timeout exceeded"

**Full message**: `Error: Operation timed out after {n} seconds`

**What it means**: A command or phase took longer than the configured timeout.

**Causes**:
- Operation genuinely slow
- Hung process or deadlock
- Insufficient timeout value
- Resource exhaustion (CPU, memory, disk I/O)

**Solutions**:
1. Increase timeout in workflow configuration
2. Check for hung processes: `ps aux | grep prodigy`
3. Optimize command performance
4. Split work into smaller chunks (use `max_items`)
5. Check system resources: `top`, `df -h`

### "Resume already in progress"

**Full message**: `Error: Resume already in progress for job {job_id}. Lock held by: PID {pid} on {hostname}`

**What it means**: Another process is currently resuming this job.

**Causes**:
- Concurrent resume attempt
- Stale lock from crashed process
- Multiple terminals running resume

**Solutions**:
1. Wait for other process to complete
2. Check if process is running: `ps aux | grep {pid}`
3. Remove stale lock if process is dead: `rm ~/.prodigy/resume_locks/{job_id}.lock`
4. Retry - stale locks are auto-detected and cleaned

See [Concurrent Resume Protection](../mapreduce/concurrent-resume-protection.md) for details.

### "JSONPath returned no results"

**Full message**: `Error: JSONPath expression '{path}' returned no results`

**What it means**: The JSONPath query didn't match any items in the input file.

**Causes**:
- Incorrect JSONPath syntax
- Wrong data structure in input file
- Empty input file
- Case-sensitive key mismatch

**Solutions**:
1. Test JSONPath with jq: `cat items.json | jq '{your_path}'`
2. Verify input file structure: `cat items.json | jq .`
3. Check for typos in key names
4. Ensure array brackets are correct: `$[*]` vs `$.items[*]`
5. Validate JSON format: `jq . items.json`

### "No commits found"

**Full message**: `Error: No commits found in worktree` or `${step.files_added} returned empty`

**What it means**: Git context variables are empty because no commits were created.

**Causes**:
- Commands didn't modify any files
- Changes not committed
- Wrong git repository context
- Worktree not initialized properly

**Solutions**:
1. Verify commands created changes: `git status`
2. Use `commit_required: true` to enforce commits
3. Check git log: `git log -1`
4. Ensure working in correct repository
5. Check if files were actually modified

### "Variable not found: {var}"

**Full message**: `Error: Variable not found: {var}` or literal `${var}` in output

**What it means**: A workflow variable reference couldn't be resolved.

**Causes**:
- Variable name typo or case mismatch
- Variable not defined in workflow
- Variable out of scope
- Syntax error in interpolation

**Solutions**:
1. Check variable spelling and case
2. Verify variable defined in `env:` or previous step
3. Use `capture_output` to capture command results
4. Check scope (step vs workflow level)
5. Verify syntax: `${var}` not `$var`

### "Invalid profile: {name}"

**Full message**: `Error: Invalid profile: {name}`

**What it means**: The specified profile doesn't exist in the workflow configuration.

**Causes**:
- Profile name typo
- Profile not defined in workflow
- Wrong flag syntax

**Solutions**:
1. Check profile name spelling
2. Verify profile exists in workflow `env:` section
3. Use correct flag: `--profile prod`
4. List available profiles in workflow YAML

### "Disk quota exceeded"

**Full message**: `Error: No space left on device` or `write: disk quota exceeded`

**What it means**: Insufficient disk space to complete operation.

**Causes**:
- Disk full
- Quota limit reached
- Large log files accumulating
- Orphaned worktrees consuming space

**Solutions**:
1. Check disk space: `df -h`
2. Clean orphaned worktrees: `prodigy worktree clean-orphaned`
3. Remove old logs: `rm ~/.prodigy/events/old-job-*/`
4. Clean Claude logs: `rm ~/.local/state/claude/logs/old-*.json`
5. Increase disk space or quota

### "Job already completed"

**Full message**: `Error: Cannot resume job {job_id}: already completed`

**What it means**: Attempting to resume a job that finished successfully.

**Causes**:
- Job actually completed
- Wrong job ID
- Attempting re-run instead of resume

**Solutions**:
1. Verify job status: `prodigy sessions list`
2. Check for correct job ID
3. Start new run instead of resume: `prodigy run workflow.yml`
4. Review job results if completion was successful

### "Concurrent modification detected"

**Full message**: `Error: Concurrent modification detected in checkpoint file`

**What it means**: Multiple processes tried to modify the same checkpoint simultaneously.

**Causes**:
- Parallel resume attempts
- File system race condition
- Stale file handle

**Solutions**:
1. Ensure only one resume process runs at a time
2. Check for concurrent resume lock
3. Wait and retry
4. Use resume lock mechanism (automatic in newer versions)
