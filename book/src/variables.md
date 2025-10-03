# Variable Interpolation

## Overview

Prodigy provides two complementary variable systems:

1. **Built-in Variables**: Automatically available based on workflow context (workflow state, step info, work items, etc.)
2. **Custom Captured Variables**: User-defined variables created via the `capture:` field in commands

Both systems use the same `${variable.name}` interpolation syntax and can be freely mixed in your workflows.

## Variable Availability by Phase

| Variable Category | Setup | Map | Reduce | Merge |
|------------------|-------|-----|--------|-------|
| Standard Variables | ✓ | ✓ | ✓ | ✓ |
| Output Variables | ✓ | ✓ | ✓ | ✓ |
| Item Variables (`${item.*}`) | ✗ | ✓ | ✗ | ✗ |
| Map Aggregation (`${map.total}`, etc.) | ✗ | ✗ | ✓ | ✗ |
| Merge Variables | ✗ | ✗ | ✗ | ✓ |
| Custom Captured Variables | ✓ | ✓ | ✓ | ✓ |

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

**Primary Output Variables:**
- `${shell.output}` - Output (stdout) from last shell command
- `${claude.output}` - Output from last Claude command
- `${last.output}` - Output from last executed command (any type)
- `${last.exit_code}` - Exit code from last command

**Note**: `${shell.output}` is the correct variable name for shell command output. The code uses `shell.output`, not `shell.stdout`.

**Legacy/Specialized Output Variables:**
- `${handler.output}` - Output from handler command (used in error handling)
- `${test.output}` - Output from test command (used in validation)
- `${goal_seek.output}` - Output from goal-seeking command

**Best Practice**: For most workflows, use custom capture variables (via `capture:` field) instead of relying on these automatic output variables. This provides explicit naming and better readability.

### MapReduce Variables

**Map Phase Variables** (available in `agent_template:` commands):
- `${item}` - Current work item in map phase (scope: map phase only)
- `${item.value}` - Value of current item (for simple items)
- `${item.path}` - Path field of current item
- `${item.name}` - Name field of current item
- `${item.*}` - Access any item field using wildcard pattern (e.g., `${item.id}`, `${item.priority}`)
- `${item_index}` - Index of current item in the list
- `${item_total}` - Total number of items being processed
- `${map.key}` - Current map key
- `${worker.id}` - ID of the current worker agent

**Reduce Phase Variables** (available in `reduce:` commands):
- `${map.total}` - Total items processed across all map agents
- `${map.successful}` - Number of successfully processed items
- `${map.failed}` - Number of failed items
- `${map.results}` - Aggregated results from all map agents (JSON array)

**Note**: `${item}` and related item variables are only available within the map phase. The aggregation variables (`${map.total}`, `${map.successful}`, `${map.failed}`, `${map.results}`) are only available in the reduce phase.

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
- `${step.commits}` - Commits in current step (array of commit objects)
- `${workflow.commits}` - All workflow commits (array of commit objects)

**Note**: These are arrays of commit data. Use in foreach loops or access individual commits with array indexing. Each commit object contains fields like hash, message, timestamp, etc.

### Legacy Variable Aliases

These legacy aliases are supported for backward compatibility but should be replaced with modern equivalents:

- `$ARG` / `$ARGUMENT` - Legacy aliases for `${item.value}` (available in WithArguments mode)
- `$FILE` / `$FILE_PATH` - Legacy aliases for `${item.path}` (available in WithFilePattern mode)

**Note:** Use the modern `${item.*}` syntax in new workflows instead of legacy aliases.

---

## Custom Variable Capture

Custom capture variables allow you to save command output with explicit names for later use. This is the recommended approach for most workflows instead of relying on automatic output variables.

### Basic Capture Examples

```yaml
# Capture to custom variable
- shell: "ls -la | wc -l"
  capture: "file_count"
  capture_format: number  # Default: string

# Use in next command
- shell: "echo 'Found ${file_count} files'"
```

### Capture Formats

The `capture_format` field determines how output is parsed and stored:

```yaml
# String format (default) - stores raw output
- shell: "echo 'Hello World'"
  capture: "greeting"
  capture_format: string
# Access: ${greeting} → "Hello World"

# Number format - parses numeric output
- shell: "echo 42"
  capture: "answer"
  capture_format: number
# Access: ${answer} → 42 (as number, not string)

# Boolean format - converts to true/false
- shell: "[ -f README.md ] && echo true || echo false"
  capture: "has_readme"
  capture_format: boolean
# Access: ${has_readme} → true or false

# JSON format - parses JSON output
- shell: "echo '{\"name\": \"project\", \"version\": \"1.0\"}'"
  capture: "package_info"
  capture_format: json
# Access nested fields: ${package_info.name} → "project"
# Access nested fields: ${package_info.version} → "1.0"

# Lines format - splits into array by newlines
- shell: "ls *.md"
  capture: "markdown_files"
  capture_format: lines
# Access: ${markdown_files} → array of filenames
```

### Capture Streams

Control which output streams to capture (useful for detailed command analysis):

```yaml
# Capture specific streams
- shell: "cargo test 2>&1"
  capture: "test_results"
  capture_streams:
    stdout: true      # Default: true
    stderr: true      # Default: false
    exit_code: true   # Default: true
    success: true     # Default: true
    duration: true    # Default: true

# Access captured stream data
- shell: "echo 'Exit code: ${test_results.exit_code}'"
- shell: "echo 'Success: ${test_results.success}'"
- shell: "echo 'Duration: ${test_results.duration}s'"
```

**Default Behavior**: By default, `stdout`, `exit_code`, `success`, and `duration` are captured (all `true`). Set `stderr: true` to also capture error output.

### Nested JSON Field Access

For JSON-formatted captures, use dot notation to access nested fields:

```yaml
# Example: API response with nested data
- shell: "curl -s https://api.example.com/user/123"
  capture: "user"
  capture_format: json

# Access nested fields with dot notation
- shell: "echo 'Name: ${user.profile.name}'"
- shell: "echo 'Email: ${user.contact.email}'"
- shell: "echo 'City: ${user.address.city}'"
```

### Variable Scope and Precedence

Variables follow a parent/child scope hierarchy:

1. **Local Scope**: Variables defined in the current command block
2. **Parent Scope**: Variables from enclosing blocks (foreach, map phase, etc.)
3. **Built-in Variables**: Standard workflow context variables

**Precedence**: Local variables override parent scope variables, which override built-in variables.

```yaml
# Parent scope
- shell: "echo 'outer'"
  capture: "message"

# Child scope (foreach creates new scope)
- foreach:
    items: [1, 2, 3]
    commands:
      # This creates a local 'message' that shadows parent
      - shell: "echo 'inner-${item}'"
        capture: "message"
      - shell: "echo ${message}"  # Uses local 'message'

# After foreach, parent 'message' is still accessible
- shell: "echo ${message}"  # Uses parent 'message' → "outer"
```
