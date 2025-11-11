# Variables and Interpolation

Prodigy provides a powerful variable system for dynamic workflow execution. Variables enable you to pass data between commands, capture outputs, and parameterize workflows for different contexts.

## Overview

The variable system supports:
- **Standard variables** - Available in all workflow types (item, workflow.name, etc.)
- **MapReduce variables** - Specific to parallel processing (map.results, worker.id, etc.)
- **Capture variables** - Command outputs stored as variables
- **Environment variables** - System and workflow-defined environment values

## Variable Interpolation

Variables are referenced using `${variable.name}` or `$VARIABLE` syntax:

```yaml
- shell: "echo Processing ${item.name}"
- shell: "cargo test --package $PROJECT_NAME"
```

### Nested Field Access

Access nested fields using dot notation:

```yaml
- shell: "echo ${item.metadata.priority}"
- claude: "/analyze ${workflow.config.target}"
```

### Default Values

Provide fallback values when variables may not exist:

```yaml
- shell: "echo ${item.description|default:No description}"
```

## Standard Variables

Available in all workflow executions:

- `${item}` - Current work item being processed
- `${item_index}` - Zero-based index of current item
- `${item_total}` - Total number of items to process
- `${workflow.name}` - Workflow identifier
- `${workflow.id}` - Unique workflow execution ID
- `${workflow.iteration}` - Current iteration number

## MapReduce Variables

Specific to MapReduce workflows:

- `${map.results}` - Aggregated results from all map agents
- `${map.key}` - Key for map output organization
- `${worker.id}` - Identifier for parallel worker
- `${merge.worktree}` - Worktree being merged
- `${merge.source_branch}` - Source branch for merge operation
- `${merge.target_branch}` - Target branch for merge operation

## Output Capture

Capture command outputs as variables for use in subsequent steps:

```yaml
- shell: "git rev-parse HEAD"
  capture: commit_sha

- shell: "echo Current commit: ${commit_sha}"
```

### Capture Formats

- `string` - Raw text output
- `json` - Parsed JSON structure
- `lines` - Array of output lines
- `number` - Numeric value
- `boolean` - Boolean flag

### Capture Metadata

Along with the output value, these metadata fields are available:

- `${variable.exit_code}` - Command exit status
- `${variable.success}` - Boolean success flag
- `${variable.duration}` - Execution time
- `${variable.stderr}` - Error output stream

## See Also

- [Environment Variables](environment.md) - Environment variable configuration
- [Command Types](command-types.md) - Commands that support variable interpolation
- [MapReduce Overview](../mapreduce/overview.md) - MapReduce-specific variables
