# Standard and Computed Variables

This section covers core output variables, workflow context, and computed variables that provide dynamic data access.

## Standard Variables

These variables capture output from the most recently executed command:

| Variable | Description | Example |
|----------|-------------|---------|
| `${last.output}` | Output from the last command of any type (shell, claude, handler) | `echo ${last.output}` |
| `${last.exit_code}` | Exit code from the last command | `if [ ${last.exit_code} -eq 0 ]` |
| `${shell.output}` | Output from the last shell command specifically | `echo ${shell.output}` |
| `${claude.output}` | Output from the last Claude command specifically | `echo ${claude.output}` |

!!! tip "Choosing Output Variables"
    - Use `${last.output}` when you need output from any command type - it's always set after every command
    - Use `${shell.output}` when you specifically need the shell command output, even if Claude commands ran after
    - Use `${claude.output}` when you specifically need the Claude response, even if shell commands ran after

### Output Variable Behavior

The type-specific variables (`shell.output`, `claude.output`) are only updated when their respective command types execute:

```yaml
# Source: Example workflow demonstrating output variable scoping

# shell.output is set here
- shell: "cargo test --lib"
- shell: "echo 'Test output: ${shell.output}'"

# claude.output is set here, shell.output still has test output
- claude: "/analyze-code"
- shell: "echo 'Claude analysis: ${claude.output}'"
- shell: "echo 'Original test output still available: ${shell.output}'"
```

!!! note "last.output Updates on Every Command"
    The `${last.output}` variable is updated after every command execution, regardless of type. Use the type-specific variables when you need to preserve output across different command types.

## Computed Variables

Computed variables are dynamically evaluated at runtime, providing access to external data sources and generated values. These variables are prefixed with specific identifiers that trigger their evaluation.

| Variable Type | Syntax | Description | Cached | Example |
|---------------|--------|-------------|--------|---------|
| **Environment** | `${env.VAR_NAME}` | Read environment variable | Yes | `${env.HOME}`, `${env.PATH}` |
| **File Content** | `${file:path/to/file}` | Read file contents | Yes | `${file:config.txt}`, `${file:data.json}` |
| **Command Output** | `${cmd:shell-command}` | Execute command and capture output | Yes | `${cmd:git rev-parse HEAD}`, `${cmd:date +%Y}` |
| **JSON Path** | `${json:path:from:source_var}` | Extract from JSON using JSONPath | No | `${json:$.items[0].name:from:data}` |
| **Date Format** | `${date:format}` | Current date/time with format | No | `${date:%Y-%m-%d}`, `${date:%H:%M:%S}` |
| **UUID** | `${uuid}` | Generate random UUID v4 | No | `${uuid}` (always unique) |

**Available in:** All phases (setup, map, reduce, merge)

### Environment Variables (`env.*`)

Access environment variables at runtime. Useful for reading system configuration or secrets.

<!-- Source: src/cook/execution/variables.rs - EnvVariable struct at lines 169-196 -->

**Examples:**
```yaml
# Read user's home directory
- shell: "echo 'Home: ${env.HOME}'"

# Use CI environment variables
- shell: "echo 'Running in ${env.CI_PROVIDER:-local}'"

# Access secrets from environment
- shell: "curl -H 'Authorization: Bearer ${env.API_TOKEN}' https://api.example.com"
```

!!! info "Caching Behavior"
    Environment variable reads are cached for performance using an LRU cache (100 entries maximum). This means the value is read once and reused within the same workflow execution.

### File Content (`file:`)

Read file contents directly into variables. Useful for configuration, templates, or data files.

<!-- Source: src/cook/execution/variables.rs - FileVariable struct at lines 198-225 -->

**Examples:**
```yaml
# Read version from file
- shell: "echo 'Version: ${file:VERSION}'"

# Use file content in command
- shell: "git commit -m '${file:.commit-message.txt}'"

# Read JSON configuration
- shell: "echo '${file:config.json}' | jq '.database.host'"
```

!!! info "Caching Behavior"
    File reads are cached because file I/O is expensive. The content is read once and reused within the same workflow execution.

!!! note "Path Resolution"
    File paths are relative to the workflow execution directory.

### Command Output (`cmd:`)

Execute shell commands and capture their output as variable values. Powerful for dynamic configuration.

<!-- Source: src/cook/execution/variables.rs - CommandVariable struct at lines 227-265 -->

**Examples:**
```yaml
# Get current git commit
- shell: "echo 'Building from ${cmd:git rev-parse --short HEAD}'"

# Use command output in logic
- shell: "if [ '${cmd:uname}' = 'Darwin' ]; then echo 'macOS'; fi"

# Capture timestamp
- shell: "echo 'Build started at ${cmd:date +%Y-%m-%d_%H-%M-%S}'"

# Dynamic configuration
- shell: "cargo build --jobs ${cmd:nproc}"
```

!!! info "Caching Behavior"
    Command execution results are cached because shell execution is expensive. The command runs once and the result is reused within the same workflow execution.

!!! warning "Security Consideration"
    Be cautious with `cmd:` variables in untrusted workflows - they execute arbitrary shell commands. Only use in workflows you control and trust.

### JSON Path Extraction (`json:`)

Extract values from JSON data using JSONPath syntax. Useful for processing complex JSON structures.

<!-- Source: src/cook/execution/variables.rs - JsonPathVariable struct at lines 362-391 -->

**Syntax:** `${json:path:from:source_variable}`

**Examples:**
```yaml
# Extract from captured variable
- shell: "curl https://api.example.com/data"
  capture_output: "api_response"
- shell: "echo 'ID: ${json:id:from:api_response}'"

# Extract array element (using dot notation for indices)
- shell: "echo 'First item: ${json:items.0.name:from:api_response}'"

# Extract nested field
- shell: "echo 'Author: ${json:metadata.author:from:config}'"

# Bracket notation for array access
- shell: "echo 'Second item: ${json:items[1].name:from:api_response}'"
```

!!! note "Path Syntax"
    Prodigy uses simple dot-notation path syntax (not full JSONPath with `$`). Use `field.nested.value` for objects and `items.0` or `items[0]` for arrays.

!!! info "Caching"
    JSON path extraction is **not cached** because parsing is fast and results may change if the source variable is updated.

### Date Formatting (`date:`)

Generate current date/time with custom formatting using chrono format specifiers.

<!-- Source: src/cook/execution/variables.rs - DateVariable struct at lines 287-314 -->

**Syntax:** `${date:format}` (uses chrono format specifiers)

**Examples:**
```yaml
# ISO 8601 date
- shell: "echo 'Report generated: ${date:%Y-%m-%d}'"

# Full timestamp
- shell: "echo 'Build time: ${date:%Y-%m-%d %H:%M:%S}'"

# Custom format
- shell: "mkdir backup-${date:%Y%m%d-%H%M%S}"

# Use in filenames
- shell: "cp logs.txt logs-${date:%Y-%m-%d}.txt"
```

**Common Format Specifiers:**

| Specifier | Description | Example |
|-----------|-------------|---------|
| `%Y` | 4-digit year | 2025 |
| `%m` | Month (01-12) | 01 |
| `%d` | Day (01-31) | 15 |
| `%H` | Hour 24h (00-23) | 14 |
| `%M` | Minute (00-59) | 30 |
| `%S` | Second (00-59) | 45 |
| `%F` | ISO 8601 date | 2025-01-15 |
| `%T` | ISO 8601 time | 14:30:45 |

!!! info "Caching"
    Date values are **not cached** because they change over time. Each reference evaluates to the current timestamp.

### UUID Generation (`uuid`)

Generate a random UUID version 4. Useful for unique identifiers, temporary filenames, or correlation IDs.

<!-- Source: src/cook/execution/variables.rs - UuidVariable struct at lines 267-285 -->

**Examples:**
```yaml
# Generate unique identifier
- shell: "echo 'Request ID: ${uuid}'"

# Create unique temporary file
- shell: "mkdir /tmp/build-${uuid}"

# Correlation ID for tracking
- shell: "curl -H 'X-Correlation-ID: ${uuid}' https://api.example.com"

# Unique test run ID
- shell: "cargo test -- --test-id ${uuid}"
```

!!! warning "Each Reference Creates a New UUID"
    Each `${uuid}` reference generates a **NEW** unique UUID. If you need the same UUID multiple times, capture it first:

    ```yaml
    - shell: "echo '${uuid}'"
      capture_output: "run_id"
    - shell: "echo 'Run ID: ${run_id}'"  # Same UUID
    - shell: "echo 'Same ID: ${run_id}'" # Still same UUID
    ```

## Computed Variable Caching

Computed variables have different caching behaviors based on their expense and volatility:

=== "Cached (Expensive Operations)"

    | Variable Type | Reason |
    |---------------|--------|
    | `env.*` | Environment variable reads |
    | `file:*` | File system operations |
    | `cmd:*` | Shell command execution |

=== "Not Cached (Fast or Volatile)"

    | Variable Type | Reason |
    |---------------|--------|
    | `json:*` | JSON parsing is fast |
    | `date:*` | Values change over time |
    | `uuid` | Must be unique each time |

**Cache Details:**

| Property | Value |
|----------|-------|
| **Type** | LRU (Least Recently Used) cache |
| **Size** | 100 entries maximum |
| **Scope** | Per workflow execution |
| **Thread Safety** | Async RwLock protection |

<!-- Source: src/cook/execution/variables.rs - VariableContext cache implementation -->

## Workflow Context Variables

Variables that provide information about the current workflow execution:

| Variable | Description | Example |
|----------|-------------|---------|
| `${workflow.name}` | Workflow name from YAML config | `echo "Running ${workflow.name}"` |
| `${workflow.id}` | Unique workflow identifier | `log-${workflow.id}.txt` |
| `${workflow.iteration}` | Current iteration number (for loops) | `Iteration ${workflow.iteration}` |

**Available in:** All phases (setup, map, reduce, merge)

## Step Context Variables

Variables providing information about the current execution step:

| Variable | Description | Example |
|----------|-------------|---------|
| `${step.name}` | Step name or identifier | `echo "Step: ${step.name}"` |
| `${step.index}` | Zero-based step index | `Step ${step.index} of ${total_steps}` |

**Available in:** All phases

## Validation Variables

Variables for workflow validation and completion tracking. These are populated by validation commands and provide feedback on requirement completion.

| Variable | Description | Example |
|----------|-------------|---------|
| `${validation.completion}` | Completion percentage (0-100) | `echo "${validation.completion}%"` |
| `${validation.gaps}` | Array of missing requirements | `echo '${validation.gaps}'` |
| `${validation.missing}` | Human-readable list of missing items | `echo "${validation.missing}"` |
| `${validation.status}` | Status: complete/incomplete/failed | `if [ "${validation.status}" = "complete" ]` |

!!! note "When Validation Variables Are Available"
    These variables are only populated **after** a validation command executes. They are typically used in `on_incomplete` handlers to provide context for fix attempts.

**Example with on_incomplete handler:**

```yaml
# Source: Example validation workflow with gap handling
validate:
  command: "cargo test"
  threshold: 90
  on_incomplete:
    - claude: "/prodigy-fix-tests ${validation.gaps}"
      max_attempts: 3
      fail_workflow: false
```

**The `validation.gaps` structure** contains an array of objects describing what's missing:

```json
[
  {"requirement": "test coverage", "current": 85, "threshold": 90},
  {"requirement": "documentation", "missing": ["module_a", "module_b"]}
]
```

**Available in:** Validation phases and `on_incomplete` handlers
