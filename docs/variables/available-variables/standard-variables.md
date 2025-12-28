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

**Note:** Use `${last.output}` when you need output from any command type. Use `${shell.output}` or `${claude.output}` when you specifically want output from that command type.

**Example:**
```yaml
- shell: "cargo test --lib"
- shell: "echo 'Test output: ${shell.output}'"

# last.output works with any command type
- claude: "/analyze-code"
- shell: "echo 'Claude analysis: ${last.output}'"
```

## Computed Variables

Computed variables are dynamically evaluated at runtime, providing access to external data sources and generated values. These variables are prefixed with specific identifiers that trigger their evaluation.

**Source:** src/cook/execution/variables.rs:100-305

| Variable Type | Syntax | Description | Cached | Example |
|---------------|--------|-------------|--------|---------|
| **Environment** | `${env.VAR_NAME}` | Read environment variable | Yes | `${env.HOME}`, `${env.PATH}` |
| **File Content** | `${file:path/to/file}` | Read file contents | Yes | `${file:config.txt}`, `${file:data.json}` |
| **Command Output** | `${cmd:shell-command}` | Execute command and capture output | Yes | `${cmd:git rev-parse HEAD}`, `${cmd:date +%Y}` |
| **JSON Path** | `${json:path:from:source_var}` | Extract from JSON using JSONPath | No | `${json:$.items[0].name:from:data}` |
| **Date Format** | `${date:format}` | Current date/time with format | No | `${date:%Y-%m-%d}`, `${date:%H:%M:%S}` |
| **UUID** | `${uuid}` | Generate random UUID v4 | No | `${uuid}` (always unique) |

**Available in:** All phases

### Environment Variables (`env.*`)

Access environment variables at runtime. Useful for reading system configuration or secrets.

**Source:** src/cook/execution/variables.rs:160-187

**Examples:**
```yaml
# Read user's home directory
- shell: "echo 'Home: ${env.HOME}'"

# Use CI environment variables
- shell: "echo 'Running in ${env.CI_PROVIDER:-local}'"

# Access secrets from environment
- shell: "curl -H 'Authorization: Bearer ${env.API_TOKEN}' https://api.example.com"
```

**Caching:** Environment variable reads are cached for performance (LRU cache, 100 entries).

### File Content (`file:`)

Read file contents directly into variables. Useful for configuration, templates, or data files.

**Source:** src/cook/execution/variables.rs:189-216

**Examples:**
```yaml
# Read version from file
- shell: "echo 'Version: ${file:VERSION}'"

# Use file content in command
- shell: "git commit -m '${file:.commit-message.txt}'"

# Read JSON configuration
- shell: "echo '${file:config.json}' | jq '.database.host'"
```

**Caching:** File reads are cached (file content is expensive to read repeatedly).

**Note:** File paths are relative to workflow execution directory.

### Command Output (`cmd:`)

Execute shell commands and capture their output as variable values. Powerful for dynamic configuration.

**Source:** src/cook/execution/variables.rs:218-256

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

**Caching:** Command execution results are cached (commands are expensive to execute repeatedly).

**Security Warning:** Be cautious with `cmd:` variables in untrusted workflows - they execute arbitrary shell commands.

### JSON Path Extraction (`json:`)

Extract values from JSON data using JSONPath syntax. Useful for processing complex JSON structures.

**Source:** src/cook/execution/variables.rs:350-379

**Syntax:** `${json:path:from:source_variable}`

**Examples:**
```yaml
# Extract from captured variable
- shell: "curl https://api.example.com/data"
  capture_output: "api_response"
- shell: "echo 'ID: ${json:$.id:from:api_response}'"

# Extract array element
- shell: "echo 'First item: ${json:$.items[0].name:from:api_response}'"

# Extract nested field
- shell: "echo 'Author: ${json:$.metadata.author:from:config}'"
```

**Not Cached:** JSON path extraction is fast and not cached.

**Requires:** Source variable must contain valid JSON.

### Date Formatting (`date:`)

Generate current date/time with custom formatting using chrono format specifiers.

**Source:** src/cook/execution/variables.rs:278-305

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
- `%Y` - 4-digit year (2025)
- `%m` - Month (01-12)
- `%d` - Day (01-31)
- `%H` - Hour 24h (00-23)
- `%M` - Minute (00-59)
- `%S` - Second (00-59)
- `%F` - ISO 8601 date (2025-01-15)
- `%T` - ISO 8601 time (14:30:45)

**Not Cached:** Date values change over time and are not cached.

### UUID Generation (`uuid`)

Generate a random UUID version 4. Useful for unique identifiers, temporary filenames, or correlation IDs.

**Source:** src/cook/execution/variables.rs:258-276

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

**Not Cached:** Each `${uuid}` reference generates a NEW unique UUID. If you need the same UUID multiple times, capture it first:

```yaml
- shell: "echo '${uuid}'"
  capture_output: "run_id"
- shell: "echo 'Run ID: ${run_id}'"  # Same UUID
- shell: "echo 'Same ID: ${run_id}'" # Still same UUID
```

## Computed Variable Caching

Computed variables have different caching behaviors based on their expense and volatility:

**Cached (Expensive Operations):**
- `env.*` - Environment variable reads
- `file:*` - File system operations
- `cmd:*` - Shell command execution

**Not Cached (Fast or Volatile):**
- `json:*` - JSON parsing is fast
- `date:*` - Values change over time
- `uuid` - Must be unique each time

**Cache Details:**
- **Type:** LRU (Least Recently Used) cache
- **Size:** 100 entries maximum
- **Scope:** Per workflow execution
- **Thread Safety:** Async RwLock protection

**Source:** src/cook/execution/variables.rs:218-256 (caching implementation)

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

Variables for workflow validation and completion tracking:

| Variable | Description | Example |
|----------|-------------|---------|
| `${validation.completion}` | Completion percentage (0-100) | `echo "${validation.completion}%"` |
| `${validation.gaps}` | Array of missing requirements | `echo '${validation.gaps}'` |
| `${validation.status}` | Status: complete/incomplete/failed | `if [ "${validation.status}" = "complete" ]` |

**Available in:** Validation phases
