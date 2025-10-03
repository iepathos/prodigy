# Workflow Basics

This chapter covers the fundamentals of creating Prodigy workflows. You'll learn about workflow structure, basic commands, and configuration options.

## Overview

Prodigy workflows are YAML files that define a sequence of commands to execute. They can be as simple as a list of shell commands or as complex as parallel MapReduce jobs.

**Two Main Workflow Types:**
- **Standard Workflows**: Sequential command execution (covered here)
- **MapReduce Workflows**: Parallel processing with map/reduce phases (see [MapReduce chapter](mapreduce.md))

## Simple Workflows

The simplest workflow is just an array of commands:

```yaml
# Simple array format - just list your commands
- shell: "echo 'Starting workflow...'"
- claude: "/prodigy-analyze"
- shell: "cargo test"
```

This executes each command sequentially. No additional configuration needed.

## Full Workflow Structure

For more complex workflows, use the full format with explicit configuration:

```yaml
# Full format with environment and merge configuration
commands:
  - shell: "cargo build"
  - claude: "/prodigy-test"

# Global environment variables (available to all commands)
env:
  NODE_ENV: production
  API_URL: https://api.example.com

# Secret environment variables (masked in logs)
secrets:
  API_KEY: "${env:SECRET_API_KEY}"

# Environment files to load (.env format)
env_files:
  - .env.production

# Environment profiles (switch contexts easily)
profiles:
  development:
    NODE_ENV: development
    DEBUG: "true"

# Custom merge workflow (for worktree integration)
merge:
  - shell: "git fetch origin"
  - claude: "/merge-worktree ${merge.source_branch}"
  timeout: 600  # Optional timeout in seconds
```

## Available Fields

Standard workflows support these top-level fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `commands` | Array | Yes* | List of commands to execute sequentially |
| `env` | Map | No | Global environment variables |
| `secrets` | Map | No | Secret environment variables (masked in logs) |
| `env_files` | Array | No | Paths to .env files to load |
| `profiles` | Map | No | Named environment profiles |
| `merge` | Object | No | Custom merge workflow for worktree integration |

**Note:** `commands` is only required in the full format. Simple array format doesn't use the `commands` key.

## Command Types

Prodigy supports several types of commands in workflows:

### Core Commands

**`shell:`** - Execute shell commands
```yaml
- shell: "cargo build --release"
- shell: "npm install"
```

**`claude:`** - Invoke Claude Code commands
```yaml
- claude: "/prodigy-lint"
- claude: "/analyze codebase"
```

### Advanced Commands

- **`goal_seek:`** - Goal-seeking operations with validation (see [Goal Seeking chapter](goal-seeking.md))
- **`foreach:`** - Iterate over lists with nested commands (see [Loops chapter](loops.md))
- **`validate:`** - Validation steps (see [Validation chapter](validation.md))
- **`analyze:`** - Analysis operations (see [Analysis chapter](analysis.md))

For detailed information on each command type and advanced features like conditional execution, error handling, and output capture, see the [Command Reference chapter](command-reference.md).

## Environment Configuration

Environment variables can be configured at multiple levels:

### Global Environment Variables

```yaml
env:
  NODE_ENV: production
  DATABASE_URL: postgres://localhost/mydb
```

### Secret Variables

Secret variables are masked in logs for security:

```yaml
secrets:
  API_KEY: "${env:SECRET_API_KEY}"
  DB_PASSWORD: "${env:DATABASE_PASSWORD}"
```

### Environment Files

Load variables from .env files:

```yaml
env_files:
  - .env
  - .env.production
```

### Environment Profiles

Switch between different environment contexts:

```yaml
profiles:
  development:
    NODE_ENV: development
    DEBUG: "true"
    API_URL: http://localhost:3000

  production:
    NODE_ENV: production
    DEBUG: "false"
    API_URL: https://api.example.com
```

Activate a profile with: `prodigy run --profile development`

For more details, see the [Environment Variables chapter](environment.md).

## Merge Workflows

Merge workflows execute when merging worktree changes back to the main branch. This allows custom validation and conflict resolution:

```yaml
merge:
  - shell: "git fetch origin"
  - shell: "git merge origin/main"
  - shell: "cargo test"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
  timeout: 600
```

**Available merge variables:**
- `${merge.worktree}` - Worktree name
- `${merge.source_branch}` - Source branch (worktree branch)
- `${merge.target_branch}` - Target branch (usually main/master)
- `${merge.session_id}` - Session ID

## Complete Example

Here's a complete workflow combining multiple features:

```yaml
# Environment configuration
env:
  RUST_BACKTRACE: 1

env_files:
  - .env

profiles:
  ci:
    CI: "true"
    VERBOSE: "true"

# Workflow commands
commands:
  - shell: "cargo fmt --check"
  - shell: "cargo clippy -- -D warnings"
  - shell: "cargo test --all"
  - claude: "/prodigy-lint"

# Custom merge workflow
merge:
  - shell: "cargo test"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
  timeout: 300
```

## Next Steps

Now that you understand basic workflows, explore these topics:

- **[Command Reference](command-reference.md)** - Detailed guide to all command types and options
- **[Environment Variables](environment.md)** - Advanced environment configuration
- **[Error Handling](error-handling.md)** - Handle failures gracefully
- **[MapReduce Workflows](mapreduce.md)** - Parallel processing for large-scale tasks
- **[Conditional Execution](conditionals.md)** - Run commands based on conditions
- **[Output Capture](output-capture.md)** - Capture and use command outputs
