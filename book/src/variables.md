# Variable Interpolation

## Available Variables

### Standard Variables
- `${workflow.name}` - Workflow name
- `${workflow.id}` - Workflow unique identifier
- `${workflow.iteration}` - Current iteration number
- `${step.name}` - Current step name
- `${step.index}` - Current step index
- `${step.files_changed}` - Files changed in current step
- `${workflow.files_changed}` - All files changed in workflow

### Output Variables
- `${shell.output}` - Output from last shell command
- `${claude.output}` - Output from last Claude command
- `${last.output}` - Output from last executed command (any type)
- `${last.exit_code}` - Exit code from last command
- `${handler.output}` - Output from handler command
- `${test.output}` - Output from test command
- `${goal_seek.output}` - Output from goal-seeking command

### MapReduce Variables
- `${item}` - Current work item in map phase
- `${item.value}` - Value of current item (for simple items)
- `${item.path}` - Path field of current item
- `${item.name}` - Name field of current item
- `${item.*}` - Access any item field using wildcard pattern (e.g., `${item.id}`, `${item.priority}`)
- `${item_index}` - Index of current item in the list
- `${item_total}` - Total number of items being processed
- `${map.key}` - Current map key
- `${map.total}` - Total items processed
- `${map.successful}` - Successfully processed items
- `${map.failed}` - Failed items
- `${map.results}` - Aggregated results
- `${worker.id}` - ID of the current worker agent

### Merge Variables
- `${merge.worktree}` - Worktree name
- `${merge.source_branch}` - Source branch
- `${merge.target_branch}` - Target branch
- `${merge.session_id}` - Session ID

### Validation Variables
- `${validation.completion}` - Completion percentage
- `${validation.completion_percentage}` - Completion percentage (numeric)
- `${validation.implemented}` - List of implemented features
- `${validation.missing}` - Missing requirements
- `${validation.gaps}` - Gap details
- `${validation.status}` - Status (complete/incomplete/failed)

### Git Context Variables
- `${step.commits}` - Commits in current step
- `${workflow.commits}` - All workflow commits

### Legacy Variable Aliases

These legacy aliases are supported for backward compatibility but should be replaced with modern equivalents:

- `$ARG` / `$ARGUMENT` - Legacy aliases for `${item.value}` (available in WithArguments mode)
- `$FILE` / `$FILE_PATH` - Legacy aliases for `${item.path}` (available in WithFilePattern mode)

**Note:** Use the modern `${item.*}` syntax in new workflows instead of legacy aliases.

---

## Custom Variable Capture

```yaml
# Capture to custom variable
- shell: "ls -la | wc -l"
  capture: "file_count"
  capture_format: number  # number, string, json, lines, boolean

# Use in next command
- shell: "echo 'Found ${file_count} files'"

# Capture specific streams
- shell: "cargo test 2>&1"
  capture: "test_results"
  capture_streams:
    stdout: true
    stderr: true
    exit_code: true
    success: true
    duration: true  # Capture execution duration

# Access captured data
- shell: "echo 'Exit code: ${test_results.exit_code}'"
- shell: "echo 'Success: ${test_results.success}'"
- shell: "echo 'Duration: ${test_results.duration}s'"
```
