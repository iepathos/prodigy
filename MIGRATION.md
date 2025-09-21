# Prodigy Migration Guide

## Version 0.2.0 Breaking Changes

This guide helps you migrate from Prodigy v0.1.x to v0.2.0, which removes several deprecated features to streamline the tool.

## Removed Commands

### 1. `cook` Command → Use `run` Instead

The `cook` command has been removed. Use `prodigy run` instead:

```bash
# Old (no longer works)
prodigy cook workflow.yml

# New
prodigy run workflow.yml
```

All flags and options remain the same - simply replace `cook` with `run`.

### 2. `improve` Alias → No Longer Supported

The `improve` alias for the `cook` command has been removed:

```bash
# Old (no longer works)
prodigy improve workflow.yml

# New
prodigy run workflow.yml
```

### 3. `dlq reprocess` → Use `dlq retry`

The `dlq reprocess` subcommand has been replaced with `dlq retry`:

```bash
# Old (no longer works)
prodigy dlq reprocess <item-ids> --job-id <job-id>

# New
prodigy dlq retry <workflow-id> --filter <expression>
```

Note: The new `dlq retry` command has different syntax and options. See `prodigy dlq retry --help` for details.

## Removed YAML Parameters

### MapReduce Workflow Parameters

The following parameters are no longer supported in MapReduce workflows:

#### 1. `timeout_per_agent`
```yaml
# Old (no longer works)
map:
  timeout_per_agent: 600
  # or
  timeout_per_agent: "10m"

# New - timeouts are handled automatically
map:
  # Remove timeout_per_agent entirely
```

#### 2. `retry_on_failure`
```yaml
# Old (no longer works)
map:
  retry_on_failure: 3

# New - retries are configured globally or per-step
map:
  # Remove retry_on_failure entirely
```

### Error Handler Parameters

The following parameters in `on_failure` handlers are no longer supported:

#### 1. `max_attempts`
```yaml
# Old (no longer works)
on_failure:
  max_attempts: 3
  commands:
    - shell: "cleanup.sh"

# New - simplified syntax
on_failure:
  - shell: "cleanup.sh"
```

#### 2. `fail_workflow`
```yaml
# Old (no longer works)
on_failure:
  fail_workflow: false
  commands:
    - shell: "recover.sh"

# New - workflows continue by default unless handler fails
on_failure:
  - shell: "recover.sh"
```

## Migration Examples

### Example 1: Simple Workflow
```yaml
# Old
name: my-workflow
steps:
  - claude: "/implement feature"
    commit_required: true  # No longer needed
  - shell: "cargo test"
    on_failure:
      max_attempts: 2      # Remove this
      commands:
        - shell: "cargo clean"

# New
name: my-workflow
steps:
  - claude: "/implement feature"
  - shell: "cargo test"
    on_failure:
      - shell: "cargo clean"
```

### Example 2: MapReduce Workflow
```yaml
# Old
name: parallel-processing
mode: mapreduce

map:
  input: items.json
  json_path: "$[*]"
  max_parallel: 10
  timeout_per_agent: 600    # Remove
  retry_on_failure: 2        # Remove
  agent_template:
    commands:  # Old nested syntax
      - claude: "/process ${item}"

# New
name: parallel-processing
mode: mapreduce

map:
  input: items.json
  json_path: "$[*]"
  max_parallel: 10
  agent_template:  # New direct syntax
    - claude: "/process ${item}"
```

### Example 3: Error Handling
```yaml
# Old
steps:
  - shell: "deploy.sh"
    on_failure:
      max_attempts: 3        # Remove
      fail_workflow: false   # Remove
      commands:
        - shell: "rollback.sh"
        - claude: "/diagnose"

# New
steps:
  - shell: "deploy.sh"
    on_failure:
      - shell: "rollback.sh"
      - claude: "/diagnose"
```

## Command Equivalents

| Old Command | New Command | Notes |
|------------|-------------|-------|
| `prodigy cook workflow.yml` | `prodigy run workflow.yml` | Direct replacement |
| `prodigy improve workflow.yml` | `prodigy run workflow.yml` | Alias removed |
| `prodigy dlq reprocess` | `prodigy dlq retry` | Different syntax |

## Getting Help

If you encounter issues during migration:

1. Run `prodigy validate <workflow.yml>` to check for deprecated syntax
2. Check the updated documentation with `prodigy --help`
3. Review examples in the `workflows/` directory
4. Report issues at https://github.com/iepathos/prodigy/issues

## Automated Migration

For simple cases, you can use the migration tool:

```bash
prodigy migrate-yaml <workflow.yml>
```

This will attempt to automatically update deprecated syntax, but manual review is recommended.