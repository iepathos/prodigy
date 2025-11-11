# Variables and Interpolation

Prodigy provides a powerful variable system that enables dynamic workflows with captured outputs, nested field access, and flexible interpolation.

## Overview

Variables in Prodigy allow you to:
- Capture command outputs in multiple formats (JSON, text, lines, numbers)
- Reference work items in MapReduce workflows
- Access nested fields with dot notation
- Provide default values for missing variables
- Use both `${var}` and `$var` syntax

## Variable Categories

### Standard Variables

Available in all execution modes:

- `${item}` - Current work item in foreach/MapReduce
- `${item_index}` - Zero-based index of current item
- `${item_total}` - Total number of items being processed
- `${workflow.name}` - Workflow identifier
- `${workflow.id}` - Unique workflow execution ID
- `${workflow.iteration}` - Current iteration number

### MapReduce Variables

Available in MapReduce workflows:

- `${map.results}` - Aggregated results from all map agents
- `${map.successful}` - Count of successfully processed items
- `${map.failed}` - Count of failed items
- `${map.total}` - Total item count
- `${map.key}` - Key for grouping map outputs
- `${worker.id}` - Identifier for parallel worker

### Merge Variables

Available in merge workflows:

- `${merge.worktree}` - Name of worktree being merged
- `${merge.source_branch}` - Source branch for merge
- `${merge.target_branch}` - Target branch for merge
- `${merge.session_id}` - Session ID for correlation

## Output Capture

Capture command outputs for use in subsequent steps:

```yaml
# Capture as string (default)
- shell: "git rev-parse HEAD"
  capture_output: commit_sha

# Capture as JSON
- shell: "cat items.json"
  capture_output: items
  capture_format: json

# Capture as lines array
- shell: "find src -name '*.rs'"
  capture_output: rust_files
  capture_format: lines

# Capture as number
- shell: "wc -l < file.txt"
  capture_output: line_count
  capture_format: number
```

### Capture Formats

- **string** - Raw text output (default)
- **json** - Parse JSON and access fields with dot notation
- **lines** - Split output into array of lines
- **number** - Parse numeric value
- **boolean** - Parse true/false value

### Capture Metadata

Additional metadata available for captured outputs:

- `${var.exit_code}` - Command exit status
- `${var.success}` - Boolean success flag
- `${var.duration}` - Execution time
- `${var.stderr}` - Error output

## Interpolation Syntax

### Basic Interpolation

```yaml
# Both syntaxes work
- shell: "echo $VARIABLE"
- shell: "echo ${VARIABLE}"
```

### Nested Field Access

```yaml
# Access nested JSON fields
- shell: "echo ${item.metadata.priority}"
- claude: "/process ${user.config.api_url}"
```

### Default Values

```yaml
# Provide default if variable missing
- shell: "echo ${PORT|default:8080}"
- claude: "/deploy ${environment|default:dev}"
```

### Array Access

```yaml
# Access array elements
- shell: "echo ${items[0]}"
- shell: "echo ${items[-1]}"  # Last element
```

## Variable Aliases

Prodigy supports aliases for backward compatibility:

- `$item` → `${item}`
- `$workflow_name` → `${workflow.name}`
- Legacy snake_case variables map to dot notation

## Examples

### Capturing and Using JSON Output

```yaml
- shell: "jq -c '{name, version}' package.json"
  capture_output: pkg
  capture_format: json

- shell: "echo Building ${pkg.name} version ${pkg.version}"
```

### Conditional Variable Usage

```yaml
- shell: "npm run build"
  when: "${environment} == 'production'"
  capture_output: build_output

- claude: "/analyze-build ${build_output}"
  when: "${build_output.success} == true"
```

### MapReduce Variable Flow

```yaml
mode: mapreduce

map:
  agent_template:
    - claude: "/process ${item.path}"
      capture_output: result

reduce:
  - shell: "echo Processing ${map.total} files"
  - shell: "echo Successful: ${map.successful}"
  - claude: "/summarize ${map.results}"
```

## See Also

- [Command Types](command-types.md) - Commands that can capture output
- [Environment Variables](environment.md) - Environment-specific configuration
- [MapReduce Workflows](../mapreduce/index.md) - Using variables in parallel workflows
- [Conditional Execution](conditional-execution.md) - Conditional logic with variables
