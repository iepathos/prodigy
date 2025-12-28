# Variable Interpolation

## Overview

Prodigy provides two complementary variable systems:

1. **Built-in Variables**: Automatically available based on workflow context (workflow state, step info, work items, etc.)
2. **Custom Captured Variables**: User-defined variables created via the `capture:` field in commands

Both systems support two syntax options:

- **`${variable.name}`** - Recommended for all variable references, especially dotted paths
- **`$VAR`** - Simple unbraced syntax for basic variable names (no dots)

!!! tip "Syntax Recommendation"
    Use `${variable.name}` syntax for consistency and clarity. Unbraced `$VAR` syntax works only for simple names without dots—it cannot be used for paths like `map.results`.

## Variable Availability by Phase

Understanding which variables are available in each workflow phase prevents interpolation errors:

| Variable Category | Setup | Map | Reduce | Merge |
|------------------|:-----:|:---:|:------:|:-----:|
| **Standard Variables** (`last.output`, `shell.output`, etc.) | ✓ | ✓ | ✓ | ✓ |
| **Output Variables** (captured via `capture:`) | ✓ | ✓ | ✓ | ✓ |
| **Computed Variables** (`env.*`, `file:`, `cmd:`, `date:`, `uuid`) | ✓ | ✓ | ✓ | ✓ |
| **Git Context Variables** (`step.files_*`, `step.commits`, etc.) | ✓ | ✓ | ✓ | ✓ |
| **Item Variables** (`${item}`, `${item.field}`) | ✗ | ✓ | ✗ | ✗ |
| **Worker Variables** (`${worker.id}`) | ✗ | ✓ | ✗ | ✗ |
| **Map Aggregation** (`map.total`, `map.successful`, `map.failed`, `map.success_rate`, `map.results`, `map.results_json`) | ✗ | ✗ | ✓ | ✗ |
| **Merge Variables** (`merge.worktree`, `merge.source_branch`, etc.) | ✗ | ✗ | ✗ | ✓ |

!!! warning "Phase-Specific Variables"
    Using variables outside their designated phase will result in interpolation errors or empty values. For example:

    - `${item}` in reduce phase → Error
    - `${map.results}` in map phase → Error
    - `${merge.worktree}` in setup phase → Error

!!! note "Accessing Item Data in Reduce Phase"
    Individual `${item.*}` variables are not available in reduce phase, but you can access all item data through `${map.results}`, which contains the aggregated results from all map agents. This enables processing item-level information during aggregation.

## Quick Example

```yaml title="Variable usage across phases"
# Source: Example workflow demonstrating variable scoping
name: example-workflow
mode: mapreduce

setup:
  # Standard and computed variables available
  - shell: "echo 'Build: ${cmd:git rev-parse --short HEAD}'"

map:
  input: "items.json"
  agent_template:
    # Item variables available in map phase
    - shell: "echo 'Processing ${item.name} by worker ${worker.id}'"

reduce:
  # Map aggregation variables available in reduce phase
  - shell: "echo 'Processed ${map.successful}/${map.total} items'"
  - shell: "echo 'Success rate: ${map.success_rate}%'"
```

## Additional Topics

Explore specific variable types and patterns:

- **[Available Variables](available-variables.md)** — Complete reference of all built-in, computed, and phase-specific variables with examples
- **[Custom Variable Capture](custom-variable-capture.md)** — Define and use custom variables via the `capture:` field
- **[Troubleshooting Variable Interpolation](troubleshooting-variable-interpolation.md)** — Common errors, debugging techniques, and solutions
