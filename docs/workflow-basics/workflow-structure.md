# Workflow Structure

Prodigy workflows are defined in YAML files and support multiple formats, from simple command arrays to complex configurations with environment variables, secrets, and custom merge workflows.

## Overview

A Prodigy workflow file specifies a sequence of commands to execute. Commands run sequentially, and each command can access outputs from previous commands. Workflows can range from simple task automation to complex multi-step pipelines with conditional execution and parallel processing.

## Workflow Formats

Prodigy supports three workflow formats, automatically detecting which format you're using based on the YAML structure.

### Simple Array Format

The most common format for basic workflows - a direct array of commands:

```yaml
# Source: workflows/debug.yml
- shell: "just test"
  on_failure:
    claude: "/prodigy-debug-test-failure --output ${shell.output}"
    max_attempts: 3
    fail_workflow: true
    commit_required: true

- claude: "/prodigy-lint"
```

**Use this format when:**
- You have a straightforward sequence of commands
- You don't need environment variables or secrets
- You don't need custom merge workflows

### Full Configuration Format

The complete format with all available fields:

```yaml
# Source: src/config/workflow.rs:12-39
name: my-workflow

commands:
  - shell: "cargo build"
  - claude: "/prodigy-test"

env:
  PROJECT_NAME: "prodigy"
  VERSION: "1.0.0"

secrets:
  API_KEY:
    secret: true
    value: "sk-abc123"

env_files:
  - .env
  - .env.local

profiles:
  dev:
    env:
      DATABASE_URL: "postgres://localhost/dev"
  prod:
    env:
      DATABASE_URL: "postgres://prod-server/db"

merge:
  commands:
    - shell: "cargo test"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
  timeout: 600
```

**Use this format when:**
- You need environment variables or secrets
- You want different configurations for different profiles
- You need a custom merge workflow for worktree integration
- You want to name your workflow

### Legacy Commands Field Format

An older format still supported for backward compatibility:

```yaml
name: my-workflow
commands:
  - shell: "cargo build"
  - claude: "/prodigy-test"
```

!!! note
    This format is equivalent to the full configuration format but without support for `env`, `secrets`, `profiles`, or `merge` fields. Use the full configuration format for new workflows.

## Available Fields

### Top-Level Fields

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `name` | No | String | Workflow name (defaults to "default") |
| `commands` | Yes* | Array | List of commands to execute sequentially |
| `env` | No | Object | Global environment variables for all commands |
| `secrets` | No | Object | Secret environment variables (masked in logs) |
| `env_files` | No | Array | Environment files to load (.env format) |
| `profiles` | No | Object | Environment profiles for different contexts (dev, prod, etc.) |
| `merge` | No | Object | Custom merge workflow for worktree integration |

*In simple array format, the entire file is the `commands` array.

### Command Types

Each command in the `commands` array must specify one of the following command types:

| Command Type | Description | Example |
|--------------|-------------|---------|
| `claude` | Execute a Claude command via Claude Code CLI | `claude: "/prodigy-lint"` |
| `shell` | Run a shell command | `shell: "cargo test"` |
| `goal_seek` | Run goal-seeking operations with validation | `goal_seek: {...}` |
| `foreach` | Iterate over lists with nested commands | `foreach: {...}` |
| `write_file` | Write content to a file | `write_file: {...}` |
| `analyze` | Run analysis commands | `analyze: {...}` |

See the [Command Types](../reference/command-types.md) reference for detailed documentation of each command type.

### Command Execution Options

All command types support these optional fields:

| Field | Type | Description |
|-------|------|-------------|
| `id` | String | Command ID for referencing outputs |
| `commit_required` | Boolean | Whether this command is expected to create git commits |
| `capture_output` | Boolean/String | Capture command output (bool for backward compat, string for variable name) |
| `on_failure` | Object | Commands to run if this command fails |
| `on_success` | Object | Commands to run if this command succeeds |
| `timeout` | Number | Timeout in seconds for command execution |
| `when` | String | Conditional expression for command execution |
| `validate` | Object | Validation configuration for checking implementation completeness |

See [Error Handling](error-handling.md) and [Conditional Execution](conditional-execution.md) for details.

## Command Execution Order

Commands in the workflow execute **sequentially** in the order they appear in the array:

1. Command 1 executes
2. Command 1 completes (success or failure)
3. Command 2 executes (can access outputs from Command 1)
4. Command 2 completes
5. And so on...

**State flows between commands:**

```yaml
# Source: workflows/complex-build-pipeline.yml
- shell: "cargo clippy -- -D warnings"
  capture_output: "clippy_warnings"
  on_failure:
    claude: "/prodigy-fix-clippy-warnings '${clippy_warnings}'"

- shell: "cargo bench"
  timeout: 600
  capture_output: "benchmark_results"
```

In this example:
- The first command captures output to `clippy_warnings`
- If it fails, the `on_failure` command can access `${clippy_warnings}`
- The second command captures output to `benchmark_results`
- Each command can access outputs from previous commands

## Format Comparison

| Feature | Simple Array | Legacy Commands | Full Configuration |
|---------|--------------|-----------------|-------------------|
| Command execution | ✅ | ✅ | ✅ |
| Workflow name | ❌ | ✅ | ✅ |
| Environment variables | ❌ | ❌ | ✅ |
| Secrets | ❌ | ❌ | ✅ |
| Profiles | ❌ | ❌ | ✅ |
| Custom merge | ❌ | ❌ | ✅ |
| Source reference | src/config/workflow.rs:78-86 | src/config/workflow.rs:104-113 | src/config/workflow.rs:87-103 |

## Examples

### Basic Workflow

```yaml
# Simple array format - most common
- shell: "cargo build"
- shell: "cargo test"
- claude: "/prodigy-lint"
```

### Workflow with Environment Variables

```yaml
# Full configuration format
name: build-and-deploy

env:
  PROJECT_NAME: "prodigy"
  BUILD_TARGET: "release"

commands:
  - shell: "cargo build --release"
  - shell: "echo Building ${PROJECT_NAME} in ${BUILD_TARGET} mode"
  - claude: "/prodigy-test --project ${PROJECT_NAME}"
```

### Workflow with Error Handling

```yaml
# Source: workflows/debug.yml
- shell: "just test"
  on_failure:
    claude: "/prodigy-debug-test-failure --output ${shell.output}"
    max_attempts: 3
    fail_workflow: true
    commit_required: true

- claude: "/prodigy-lint"
```

### Workflow with Conditional Execution

```yaml
# Source: workflows/complex-build-pipeline.yml
- shell: "cargo check"
  on_success:
    shell: "cargo build --release"
    on_success:
      shell: "cargo test --release"
      on_failure:
        claude: "/prodigy-debug-and-fix '${shell.output}'"
```

## Common Structure Mistakes

!!! warning "Empty Workflow"
    ```yaml
    # ❌ Wrong - empty workflow
    commands: []
    ```
    ```yaml
    # ✅ Correct - at least one command
    commands:
      - shell: "echo Hello"
    ```

!!! warning "Missing Command Type"
    ```yaml
    # ❌ Wrong - no command type specified
    - "cargo test"
    ```
    ```yaml
    # ✅ Correct - explicit command type
    - shell: "cargo test"
    ```

!!! warning "Mixed Formats"
    ```yaml
    # ❌ Wrong - mixing simple array with top-level fields
    name: my-workflow
    - shell: "cargo build"
    ```
    ```yaml
    # ✅ Correct - use full configuration format
    name: my-workflow
    commands:
      - shell: "cargo build"
    ```

## Next Steps

- [Variables](variables.md) - Learn about variable interpolation and substitution
- [Environment](environment.md) - Deep dive into environment variables and profiles
- [Error Handling](error-handling.md) - Detailed error handling patterns
- [Conditional Execution](conditional-execution.md) - Advanced conditional logic
- [Command Types Reference](../reference/command-types.md) - Complete command type documentation
