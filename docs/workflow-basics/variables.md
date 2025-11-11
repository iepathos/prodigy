# Variables and Interpolation

Prodigy provides a powerful variable system for dynamic workflow execution. Variables enable you to pass data between commands, capture outputs, and parameterize workflows for different contexts.

## Overview

The variable system supports:
- **Standard variables** - Available in all workflow types (item, workflow.name, etc.)
- **MapReduce variables** - Specific to parallel processing (map.results, worker.id, etc.)
- **Capture variables** - Command outputs stored as variables
- **Environment variables** - System and workflow-defined environment values

## Variable Interpolation

Variables are referenced using two syntax forms:

- `${variable.name}` - Braced syntax (recommended for complex paths, nested fields, and default values)
- `$VARIABLE` - Unbraced syntax (convenient for simple environment variables)

```yaml
# Source: Common workflow patterns
- shell: "echo Processing ${item.name}"
- shell: "cargo test --package $PROJECT_NAME"
```

!!! tip "Choosing the Right Syntax"
    Use `${...}` syntax when accessing nested fields, arrays, or providing default values. Use `$VAR` for simple environment variable references.

### Nested Field Access

Access nested fields using dot notation and array indexing with brackets:

```yaml
# Source: src/cook/execution/interpolation.rs:480-508
# Dot notation for object properties
- shell: "echo ${item.metadata.priority}"
- claude: "/analyze ${workflow.config.target}"

# Array indexing with brackets
- shell: "echo First item: ${items[0].name}"
- shell: "echo Result: ${data.results[0]}"
```

The interpolation engine supports:
- **Dot notation**: `${object.property}` for accessing object fields
- **Array indexing**: `${array[0]}` or `${object.items[0].field}` for array elements

### Default Values

Provide fallback values when variables may not exist using bash-style `:-` syntax:

```yaml
# Source: src/cook/execution/interpolation.rs:277
- shell: "echo ${item.description:-No description}"
- shell: "timeout ${timeout:-600}"
```

The syntax `${variable:-default}` returns the default value if the variable is undefined or null.

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

## Advanced Features

### Alias Resolution

The interpolation engine supports alias resolution for backwards compatibility. This allows older variable names to resolve to their current equivalents, ensuring workflow compatibility across versions.

!!! note
    Alias resolution is handled automatically by the interpolation engine and requires no special syntax.

## See Also

- [Environment Variables](environment.md) - Environment variable configuration
- [Workflow Structure](workflow-structure.md) - Commands that support variable interpolation
- [Work Distribution](../mapreduce/work-distribution.md) - MapReduce-specific variables
