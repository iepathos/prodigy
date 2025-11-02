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

Prodigy supports several types of commands in workflows. **Each command step must specify exactly one command type** - they are mutually exclusive.

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

- **`goal_seek:`** - Goal-seeking operations with validation (see [Advanced Features](advanced.md))
- **`foreach:`** - Iterate over lists with nested commands (see [Advanced Features](advanced.md))
- **`validate:`** - Validates that implementation matches spec/plan requirements (see [Commands](commands.md))
- **`write_file:`** - Write content to files with format validation (see [Commands](commands.md))
- **`analyze:`** - Run analysis handlers for coverage, complexity metrics, etc. (see [Commands](commands.md))

**Deprecated:**
- **`test:`** - Deprecated in favor of `shell:` with `on_failure:` handlers

For detailed information on each command type and their fields, see the [Command Types chapter](commands.md).

## Command-Level Options

All command types support additional fields for advanced control:

### Basic Options

```yaml
- shell: "cargo test"
  id: "run-tests"              # Step identifier for output referencing
  commit_required: true        # Expect git commit after this step
  timeout: 300                 # Timeout in seconds
  cwd: "./subproject"          # Set working directory (alias: working_dir)
  output_file: "test-results.txt"  # Save output to file
```

**Working Directory Control:**
- `cwd` (or `working_dir`) - Changes working directory for the command
- Supports variable interpolation: `cwd: "${PROJECT_DIR}/subdir"`
- Relative paths are relative to workflow file location

### Conditional Execution

Run commands based on conditions:

```yaml
- shell: "deploy.sh"
  when: "${branch} == 'main'"  # Only run on main branch
```

### Error Handling

Handle failures gracefully:

```yaml
- shell: "risky-command"
  on_failure:
    shell: "cleanup.sh"        # Run on failure
  on_success:
    shell: "notify.sh"         # Run on success
```

### Output Capture

Capture command output to variables for use in subsequent commands:

```yaml
- shell: "git rev-parse HEAD"
  id: "get-commit"
  capture_output: "commit_hash"  # Variable name to capture output
  capture_format: "string"       # Format type (see below)
  capture_streams: "stdout"      # Which streams to capture (default)
```

**Capture formats:**
- `string` - Raw output as string (default)
- `json` - Parse output as JSON object
- `lines` - Split output into array of lines
- `number` - Parse output as number
- `boolean` - Parse output as true/false

**Stream control:**
- `capture_streams` - Controls which output streams to capture:
  - `stdout` - Capture standard output only (default)
  - `stderr` - Capture error output only
  - `both` - Capture both stdout and stderr combined

```yaml
# Example: Capture stderr separately for error analysis
- shell: "cargo build 2>&1"
  capture_output: "build_output"
  capture_streams: "both"
```

**Output capture with metadata:**

When using `capture_output` with a variable name, the command output is stored in that variable, and additional metadata is automatically captured as sub-fields:

- `${varname}` - The captured output (formatted according to `capture_format`)
- `${varname.exit_code}` - The command's exit code (0 for success)
- `${varname.success}` - Boolean indicating command success (true/false)
- `${varname.duration}` - Command execution duration in seconds
- `${varname.stderr}` - Standard error output (when using `capture_streams: "both"`)

Example:
```yaml
- shell: "cargo test"
  capture_output: "test_results"
  capture_format: "string"

- shell: "echo 'Exit code: ${test_results.exit_code}'"
- shell: "echo 'Success: ${test_results.success}'"
- shell: "echo 'Duration: ${test_results.duration}s'"
```

For comprehensive coverage of these options, see:
- [Advanced Features](advanced.md) - Conditional execution, output capture, timeouts
- [Error Handling](error-handling.md) - on_failure and on_success handlers
- [Variables](variables.md) - Variable interpolation and capture formats

## Environment Configuration

Environment variables can be configured at multiple levels:

### Global Environment Variables

**Basic (Static) Variables:**

```yaml
env:
  NODE_ENV: production
  DATABASE_URL: postgres://localhost/mydb
```

**Advanced Variable Types:**

Prodigy supports three types of environment variable values:

1. **Static** - Plain string values (shown above)
2. **Dynamic** - Execute commands to compute values (with caching support)
3. **Conditional** - Use expressions to compute values based on conditions

**Dynamic Variables:**

Execute shell commands to generate values:

```yaml
env:
  GIT_COMMIT:
    command: "git rev-parse HEAD"
    cache: true  # Cache result for performance
  BUILD_TIME:
    command: "date -u +%Y-%m-%dT%H:%M:%SZ"
    cache: false  # Recompute each time
```

**Conditional Variables:**

Use expressions to set values based on conditions:

```yaml
env:
  LOG_LEVEL:
    condition: "${CI} == 'true'"
    if_true: "debug"
    if_false: "info"
  API_URL:
    condition: "${NODE_ENV} == 'production'"
    if_true: "https://api.example.com"
    if_false: "http://localhost:3000"
```

### Secret Variables

Secret variables are masked in logs for security. Prodigy supports multiple secret providers:

**Environment Variable Provider:**

```yaml
secrets:
  API_KEY: "${env:SECRET_API_KEY}"
  DB_PASSWORD: "${env:DATABASE_PASSWORD}"
```

**File Provider:**

Read secrets from files:

```yaml
secrets:
  SSH_KEY: "${file:/path/to/ssh/key}"
  TLS_CERT: "${file:~/.config/certs/client.pem}"
```

**Vault Provider:**

Retrieve secrets from HashiCorp Vault:

```yaml
secrets:
  DATABASE_PASSWORD: "${vault:secret/data/myapp/db}"
  API_TOKEN: "${vault:auth/token/myapp}"
```

**AWS Secrets Manager Provider:**

Fetch secrets from AWS:

```yaml
secrets:
  RDS_PASSWORD: "${aws:prod/database/password}"
  API_KEY: "${aws:prod/api/key}"
```

**Custom Provider:**

Use custom secret providers:

```yaml
secrets:
  CUSTOM_SECRET: "${custom:my-provider:secret-name}"
```

All secret values are masked in logs, error messages, and output regardless of provider type.

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

### Step-Level Environment Overrides

Individual commands can override or add environment variables:

```yaml
- shell: "npm test"
  env:
    NODE_ENV: test
    DEBUG: "true"

- shell: "cargo build"
  env:
    RUST_BACKTRACE: full
```

Step-level environment variables override global variables for that command only.

**Clearing Parent Environment:**

Use `clear_env` to start with a clean environment (no inherited global variables):

```yaml
- shell: "isolated-command"
  env:
    ONLY_VAR: "value"
    clear_env: true  # Start fresh, ignore global env
```

This is useful for commands that need complete isolation from the global environment. When `clear_env: true`, only the step-level environment variables are available to the command. Default is `false` (inherit global environment).

For more details, see the [Environment Variables chapter](environment.md).

## Merge Workflows

Merge workflows execute when merging worktree changes back to the main branch. This feature enables custom validation, testing, and conflict resolution before integrating changes.

**When to use merge workflows:**
- Run tests before merging
- Validate code quality
- Handle merge conflicts automatically
- Sync with upstream changes

```yaml
merge:
  - shell: "git fetch origin"
  - shell: "git merge origin/main"
  - shell: "cargo test"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
  timeout: 600  # Optional: timeout for entire merge phase (seconds)
```

**Available merge variables:**
- `${merge.worktree}` - Worktree name (e.g., "prodigy-session-abc123")
- `${merge.source_branch}` - Source branch (worktree branch)
- `${merge.target_branch}` - Target branch (usually main/master)
- `${merge.session_id}` - Session ID for correlation

These variables are only available within the merge workflow context.

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
```

## Next Steps

Now that you understand basic workflows, explore these topics:

- **[Command Types](commands.md)** - Detailed guide to all command types and options
- **[Advanced Features](advanced.md)** - Conditional execution, output capture, goal seeking, and more
- **[Environment Variables](environment.md)** - Advanced environment configuration
- **[Error Handling](error-handling.md)** - Handle failures gracefully
- **[MapReduce Workflows](mapreduce.md)** - Parallel processing for large-scale tasks
- **[Variables](variables.md)** - Variable interpolation and usage
