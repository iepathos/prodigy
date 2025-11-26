## Available Variables

Prodigy provides a comprehensive set of built-in variables that are automatically available based on your workflow context. All variables use the `${variable.name}` interpolation syntax.

### Standard Variables

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

### Computed Variables

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

#### Environment Variables (`env.*`)

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

#### File Content (`file:`)

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

#### Command Output (`cmd:`)

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

#### JSON Path Extraction (`json:`)

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

#### Date Formatting (`date:`)

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

#### UUID Generation (`uuid`)

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

#### Computed Variable Caching

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

### Workflow Context Variables

Variables that provide information about the current workflow execution:

| Variable | Description | Example |
|----------|-------------|---------|
| `${workflow.name}` | Workflow name from YAML config | `echo "Running ${workflow.name}"` |
| `${workflow.id}` | Unique workflow identifier | `log-${workflow.id}.txt` |
| `${workflow.iteration}` | Current iteration number (for loops) | `Iteration ${workflow.iteration}` |

**Available in:** All phases (setup, map, reduce, merge)

### Step Context Variables

Variables providing information about the current execution step:

| Variable | Description | Example |
|----------|-------------|---------|
| `${step.name}` | Step name or identifier | `echo "Step: ${step.name}"` |
| `${step.index}` | Zero-based step index | `Step ${step.index} of ${total_steps}` |

**Available in:** All phases

### Item Variables (Map Phase Only)

Variables for accessing work item data during parallel processing. The `${item.*}` syntax supports **arbitrary field access** - you can access any field present in your JSON work items, not just the predefined ones shown below.

**Source:** src/cook/workflow/variables.rs:16-23 (item variable resolution)

| Variable | Description | Example |
|----------|-------------|---------|
| `${item}` | Full item object (as string) | `echo ${item}` |
| `${item.value}` | Item value for simple types | `process ${item.value}` |
| `${item.path}` | File path (for file inputs) | `cat ${item.path}` |
| `${item.name}` | Item display name | `echo "Processing ${item.name}"` |
| `${item_index}` | Zero-based item index | `Item ${item_index}` |
| `${item_total}` | Total number of items | `of ${item_total}` |
| `${item.*}` | **Any JSON field** - Access arbitrary fields from your work items | `${item.priority}`, `${item.custom_field}` |

**Available in:** Map phase only

#### Arbitrary Field Access

The `${item.*}` syntax provides full access to any field in your JSON work items. This includes:

- **Top-level fields:** `${item.priority}`, `${item.status}`, `${item.category}`
- **Nested fields:** `${item.metadata.author}`, `${item.config.database.host}`
- **Array indices:** `${item.tags[0]}`, `${item.dependencies[2].version}`
- **Mixed access:** `${item.data.results[0].score}`

**Example with custom JSON structure:**
```yaml
# Input: items.json
# [
#   {
#     "file": "src/main.rs",
#     "priority": 10,
#     "owner": "backend-team",
#     "metadata": {
#       "last_modified": "2025-01-10",
#       "reviewer": "alice"
#     },
#     "tags": ["critical", "security"]
#   }
# ]

map:
  input: "items.json"
  json_path: "$[*]"
  agent_template:
    # Access any field from your JSON structure
    - shell: "echo 'Processing ${item.file}'"
    - shell: "echo 'Priority: ${item.priority}'"
    - shell: "echo 'Owner: ${item.owner}'"
    - shell: "echo 'Reviewer: ${item.metadata.reviewer}'"
    - shell: "echo 'First tag: ${item.tags[0]}'"
    - claude: "/analyze '${item.file}' --priority ${item.priority} --owner ${item.owner}"
```

**Best Practice:** Use descriptive field names in your JSON work items - they become your variable names.

### MapReduce Variables (Reduce Phase Only)

Variables for accessing aggregated results from map phase. Map results support **indexed access** for retrieving individual agent results and **nested field access** for extracting specific properties.

**Source:** src/cook/execution/mapreduce/utils.rs:119-121, src/cook/execution/mapreduce/reduce_phase.rs:146

| Variable | Description | Example |
|----------|-------------|---------|
| `${map.total}` | Total items in map phase | `echo "Processed ${map.total} items"` |
| `${map.successful}` | Successfully processed items | `echo "${map.successful} succeeded"` |
| `${map.failed}` | Failed items count | `echo "${map.failed} failed"` |
| `${map.results}` | All map results as JSON array | `echo '${map.results}' \| jq` |
| `${map.results_json}` | Alias for `map.results` (same value) | `echo '${map.results_json}' \| jq` |
| `${map.results[index]}` | Individual result by index (0-based) | `${map.results[0]}`, `${map.results[5]}` |
| `${map.results[index].field}` | Nested field access | `${map.results[0].output}`, `${map.results[2].item_id}` |
| `${map.key}` | Key for map output (optional) | `${map.key}` |
| `${worker.id}` | Worker ID for tracking | `Worker ${worker.id}` |

**Available in:** Reduce phase only

#### Indexed Access to Map Results

You can access individual agent results using bracket notation `[index]` and drill into nested fields with dot notation.

**Syntax patterns:**
- `${map.results[0]}` - First agent result (full object)
- `${map.results[0].output}` - Output from first agent
- `${map.results[0].item_id}` - Item ID processed by first agent
- `${map.results[0].success}` - Success status ("true" or "false")

**Example:**
```yaml
reduce:
  # Access specific agent results
  - shell: "echo 'First result: ${map.results[0]}'"
  - shell: "echo 'First output: ${map.results[0].output}'"
  - shell: "echo 'Second agent processed: ${map.results[1].item_id}'"

  # Combine with shell commands
  - shell: |
      if [ "${map.results[0].success}" = "true" ]; then
        echo "First agent succeeded"
      fi

  # Process multiple results
  - shell: |
      echo "Results 0-2:"
      echo "${map.results[0].item_id}"
      echo "${map.results[1].item_id}"
      echo "${map.results[2].item_id}"
```

#### Full Array Processing

For processing all results, use `${map.results}` with JSON tools like `jq`:

```yaml
reduce:
  # Count errors using jq
  - shell: |
      echo '${map.results}' | jq '[.[] | select(.status == "error")] | length'
    capture_output: "error_count"

  # Extract all item IDs
  - shell: |
      echo '${map.results}' | jq -r '.[].item_id'
    capture_output: "processed_items"

  # Calculate average score
  - shell: |
      echo '${map.results}' | jq '[.[].score] | add / length'
    capture_output: "avg_score"

  # Filter successful results
  - shell: |
      echo '${map.results}' | jq '[.[] | select(.success == true)]'
    capture_output: "successful_results"

  # Generate summary
  - claude: "/summarize ${map.results} --total ${map.total} --failed ${map.failed}"
```

**Note:** `${map.results}` and `${map.results_json}` are equivalent - use whichever is clearer in your context.

### Git Context Variables

Variables tracking git changes throughout workflow execution:

| Variable | Description | Example |
|----------|-------------|---------|
| `${step.files_added}` | Files added in current step | `echo ${step.files_added}` |
| `${step.files_modified}` | Files modified in current step | `echo ${step.files_modified}` |
| `${step.files_deleted}` | Files deleted in current step | `echo ${step.files_deleted}` |
| `${step.files_changed}` | All changed files (added + modified + deleted) | `echo ${step.files_changed}` |
| `${step.commits}` | Commits in current step | `echo ${step.commits}` |
| `${step.commit_count}` | Number of commits in step | `echo "${step.commit_count} commits"` |
| `${step.insertions}` | Lines inserted in step | `echo "+${step.insertions}"` |
| `${step.deletions}` | Lines deleted in step | `echo "-${step.deletions}"` |
| `${workflow.commits}` | All commits in workflow | `git show ${workflow.commits}` |
| `${workflow.commit_count}` | Total number of commits | `echo "${workflow.commit_count} commits"` |

**Available in:** All phases (requires git repository)

#### Format Modifiers

**Important:** These format modifiers work with **all git context variables that return file or commit lists**, not just the examples shown. Apply them to any of: `step.files_added`, `step.files_modified`, `step.files_deleted`, `step.files_changed`, `step.commits`, `workflow.commits`, and merge phase git variables.

Git context variables support multiple output formats:

| Modifier | Description | Example |
|----------|-------------|---------|
| (default) | Space-separated list | `${step.files_added}` → `file1.rs file2.rs` |
| `:json` | JSON array format | `${step.files_added:json}` → `["file1.rs", "file2.rs"]` |
| `:lines` | Newline-separated list | `${step.files_added:lines}` → `file1.rs\nfile2.rs` |
| `:csv` | Comma-separated list | `${step.files_added:csv}` → `file1.rs,file2.rs` |
| `:*.ext` | Glob pattern filter | `${step.files_added:*.rs}` → only Rust files |
| `:path/**/*.ext` | Path with glob | `${step.files_added:src/**/*.rs}` → Rust files in src/ |

**Format Examples:**
```yaml
# JSON format for jq processing
- shell: "echo '${step.files_added:json}' | jq -r '.[]'"

# Newline format for iteration
- shell: |
    echo '${step.files_modified:lines}' | while read file; do
      cargo fmt "$file"
    done

# Glob filtering for language-specific operations
- shell: "cargo clippy ${step.files_modified:*.rs}"

# Multiple glob patterns
- shell: "git diff ${step.files_modified:*.rs,*.toml}"
```

### Merge Variables (Merge Phase Only)

Variables available during the merge phase when integrating worktree changes. Merge variables include both basic context and comprehensive git tracking information.

**Source:** src/worktree/merge_orchestrator.rs:340-423

#### Basic Merge Context

| Variable | Description | Example |
|----------|-------------|---------|
| `${merge.worktree}` | Worktree name being merged | `echo ${merge.worktree}` |
| `${merge.source_branch}` | Source branch from worktree | `git log ${merge.source_branch}` |
| `${merge.target_branch}` | Target branch (where you started) | `git merge ${merge.source_branch}` |
| `${merge.session_id}` | Session ID for correlation | `echo ${merge.session_id}` |

#### Merge Git Context Variables

Additional variables tracking git changes during the merge operation:

| Variable | Description | Format | Example |
|----------|-------------|--------|---------|
| `${merge.commits}` | All commits from worktree | JSON array | `echo '${merge.commits}' \| jq` |
| `${merge.commit_count}` | Number of commits | Integer | `echo "${merge.commit_count} commits"` |
| `${merge.commit_ids}` | Short commit IDs | Comma-separated | `git show ${merge.commit_ids}` |
| `${merge.modified_files}` | Modified files with metadata | JSON array | `echo '${merge.modified_files}' \| jq` |
| `${merge.file_count}` | Number of modified files | Integer | `echo "${merge.file_count} files"` |
| `${merge.file_list}` | File paths | Comma-separated | `echo ${merge.file_list}` |

**Available in:** Merge phase only

**Limits:** Capped at 100 commits and 500 files to prevent overwhelming workflows (configurable in GitOperationsConfig).

#### Merge Context Examples

**Basic merge workflow:**
```yaml
merge:
  commands:
    - shell: "git fetch origin"
    - shell: "git merge origin/${merge.target_branch}"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

**Using git context variables:**
```yaml
merge:
  commands:
    # Show merge summary
    - shell: |
        echo "Merging worktree: ${merge.worktree}"
        echo "Commits: ${merge.commit_count}"
        echo "Files modified: ${merge.file_count}"

    # List all commits being merged
    - shell: "echo 'Commit IDs: ${merge.commit_ids}'"

    # Process commits as JSON
    - shell: |
        echo '${merge.commits}' | jq -r '.[] | "\(.short_id): \(.message)"'

    # Check specific files
    - shell: |
        echo '${merge.modified_files}' | jq -r '.[].path'

    # Conditional merge based on file count
    - shell: |
        if [ ${merge.file_count} -gt 50 ]; then
          echo "Large merge detected, requesting review"
        fi

    # Perform merge
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

#### Commit Object Structure

The `${merge.commits}` variable contains an array of commit objects with this structure:

```json
[
  {
    "id": "full-sha-hash",
    "short_id": "abc1234",
    "author": {
      "name": "Author Name",
      "email": "author@example.com"
    },
    "message": "Commit message",
    "timestamp": "2025-01-10T12:00:00Z",
    "files_changed": ["file1.rs", "file2.rs"]
  }
]
```

**Source:** src/cook/execution/mapreduce/resources/git_operations.rs:280-293

#### File Object Structure

The `${merge.modified_files}` variable contains an array of file modification objects:

```json
[
  {
    "path": "src/main.rs",
    "modification_type": "Modified",
    "size_before": 1024,
    "size_after": 1156,
    "last_modified": "2025-01-10T12:00:00Z",
    "commit_id": "abc1234"
  }
]
```

**Source:** src/cook/execution/mapreduce/resources/git_operations.rs:311-322

### Validation Variables

Variables for workflow validation and completion tracking:

| Variable | Description | Example |
|----------|-------------|---------|
| `${validation.completion}` | Completion percentage (0-100) | `echo "${validation.completion}%"` |
| `${validation.gaps}` | Array of missing requirements | `echo '${validation.gaps}'` |
| `${validation.status}` | Status: complete/incomplete/failed | `if [ "${validation.status}" = "complete" ]` |

**Available in:** Validation phases

### Variable Interpolation Syntax

Prodigy supports two interpolation syntaxes:

- **`${VAR}`** - Preferred syntax, works in all contexts (recommended)
- **`$VAR`** - Shell-style syntax, simpler but may have limitations

**When to use `${VAR}`:**
- In YAML values with special characters
- For nested field access: `${item.nested.field}`
- When combining with text: `prefix_${var}_suffix`
- For format modifiers: `${step.files:json}`

**When `$VAR` works:**
- Simple variable names in shell commands
- Environment variables in shell context
- Quick substitutions without special characters

**Best Practice:** Always use `${VAR}` syntax for consistency and reliability.

### Legacy Variable Aliases

For backward compatibility, Prodigy supports legacy variable aliases from earlier versions. These are still functional but **deprecated** - prefer the current variable names in new workflows.

**Source:** src/cook/workflow/variables.rs (legacy alias definitions)

| Legacy Alias | Current Variable | Context | Status |
|--------------|------------------|---------|--------|
| `${ARG}` | `${item.value}` | Map phase | Deprecated |
| `${ARGUMENT}` | `${item.value}` | Map phase | Deprecated |
| `${FILE}` | `${item.path}` | Map phase | Deprecated |
| `${FILE_PATH}` | `${item.path}` | Map phase | Deprecated |

**Example:**
```yaml
# Old style (still works but discouraged)
map:
  agent_template:
    - shell: "process ${ARG}"
    - shell: "cat ${FILE}"

# New style (recommended)
map:
  agent_template:
    - shell: "process ${item.value}"
    - shell: "cat ${item.path}"
```

**Migration Recommendation:** Update legacy aliases to current variable names when maintaining older workflows. The current names are more explicit and work better with arbitrary JSON field access.

#### Default Values

Provide fallback values for undefined or missing variables using the `:-` syntax (bash/shell convention):

**Syntax:** `${variable:-default_value}`

**Source:** src/cook/execution/interpolation.rs:277

**Examples:**
```yaml
# Use default if variable is undefined
- shell: "echo 'Timeout: ${timeout:-600}'"
  # Output: "Timeout: 600" if timeout is not defined

# Fallback for optional configuration
- shell: "cargo build --profile ${build_profile:-dev}"
  # Uses "dev" profile if build_profile not set

# Default for MapReduce variables
- shell: "echo 'Processed ${map.successful:-0} items'"
  # Shows "0" if map.successful is not available
```

**Behavior with Interpolation Modes:**
- **Non-strict mode (default):** Uses default value if variable is undefined
- **Strict mode:** Default value syntax prevents errors for optional variables

#### Interpolation Modes

Prodigy supports two modes for handling undefined variables:

**Non-strict Mode (Default):**
- Leaves placeholders unresolved when variable is undefined
- Example: `${undefined}` remains as `${undefined}` in output
- With default: `${undefined:-fallback}` becomes `fallback`
- Use case: Workflows that can handle partial variable resolution

**Strict Mode:**
- Fails immediately on undefined variables
- Example: `${undefined}` causes workflow to fail with comprehensive error
- Error message lists all available variables for debugging
- Use case: Production workflows requiring all variables to be properly defined

**Source:** src/cook/execution/interpolation.rs:16-17, 104-137

**Configuration:**
Strict mode is configured per InterpolationEngine instance and controlled at the workflow execution level.

**Best Practice:** Use strict mode during development to catch variable name typos and scope issues early. Use default values (`${var:-default}`) for truly optional configuration.

**Examples:**
```yaml
# Non-strict mode (graceful degradation)
- shell: "echo 'Config: ${optional_config:-none}'"
  # Works even if optional_config is undefined

# Strict mode (fail fast)
# If required_var is undefined, workflow stops with error:
# "Variable interpolation failed: required_var not found.
#  Available variables: workflow.name, workflow.id, step.index, ..."
- shell: "echo 'Required: ${required_var}'"
```

### Variable Scoping and Precedence

#### Scope by Phase

| Phase | Variables Available |
|-------|---------------------|
| Setup | Standard, workflow context, step context, git context, custom captured |
| Map | Standard, workflow context, step context, git context, item variables, custom captured |
| Reduce | Standard, workflow context, step context, git context, MapReduce variables, custom captured |
| Merge | Standard, workflow context, step context, merge variables, custom captured |

**Important:** Setup phase captures are available in map and reduce phases. Map phase captures are only available within that specific agent. Reduce phase captures are available to subsequent reduce steps.

#### Variable Precedence (highest to lowest)

1. **Custom captured variables** (`capture_output`)
2. **Phase-specific built-in variables** (`item.*`, `map.*`, `merge.*`)
3. **Step context variables** (`step.*`)
4. **Workflow context variables** (`workflow.*`)
5. **Standard output variables** (`last.output`, `shell.output`)
6. **Environment variables** (static workflow-level `env` block)
7. **Computed variables** (`env.*`, `file:*`, `cmd:*`, `json:*`, `date:*`, `uuid`)

**Note:** Computed variables have lowest precedence because they're evaluated on-demand. If a custom variable has the same name as a computed variable, the custom variable wins.

**Shadowing Warning:** Custom captures can shadow built-in variable names. Avoid using names like `item`, `map`, `workflow`, etc. as custom variable names.

**Example:**
```yaml
# Bad: shadows built-in ${item}
- shell: "custom command"
  capture_output: "item"  # Don't do this!

# Good: descriptive custom name
- shell: "custom command"
  capture_output: "custom_result"
```

#### Parent Context Resolution

Variable resolution walks up a parent context chain when variables are not found in the current context. This enables variable inheritance across workflow phases and nested contexts.

**Source:** src/cook/execution/interpolation.rs:200-226, InterpolationContext struct at :376-381

**Resolution Order:**
1. Check current context
2. If not found, check parent context
3. If not found in parent, check parent's parent
4. Continue until variable is found or no parent exists
5. If not found and has default value, use default
6. If not found in strict mode, fail with error listing available variables

**Benefits:**
- Nested workflow contexts inherit variables from parent workflows
- Foreach loops access both loop-level and workflow-level variables
- Map agents access setup phase variables
- Reduce phase accesses both map results and setup variables

**Example:**
```yaml
setup:
  - shell: "pwd"
    capture_output: "workspace_root"  # Available to all agents via parent context
  - shell: "git rev-parse HEAD"
    capture_output: "base_commit"     # Also inherited by map agents

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - shell: "echo 'processing ${item.name}'"
      capture_output: "item_status"  # Only in this agent's context
    - shell: "cd ${workspace_root}"  # Resolved from parent (setup) context
    - shell: "git diff ${base_commit}" # Also from parent context
    - shell: "echo 'Status: ${item_status}'" # From current agent context

reduce:
  # Can access setup variables but NOT individual agent's item_status
  - shell: "cd ${workspace_root}"  # From setup phase parent context
  - shell: "echo 'Base: ${base_commit}'"  # Also from setup phase
```

**Context Hierarchy:**
```
Setup Context (workspace_root, base_commit)
    ↓ parent
Map Agent Context (item, item_status, workspace_root*, base_commit*)
    ↓ parent
Reduce Context (map.results, workspace_root*, base_commit*)
```

*Inherited from parent context

### Reduce Phase Access to Item Data

In the reduce phase, individual item variables (`${item.*}`) are not directly available, but you can access all item data through `${map.results}`, which contains the aggregated results from all map agents.

**Examples:**
```yaml
reduce:
  # Count items with specific property
  - shell: |
      echo '${map.results}' | jq '[.[] | select(.type == "error")] | length'
    capture_output: "error_count"

  # Extract all file paths processed
  - shell: |
      echo '${map.results}' | jq -r '.[].item.path'
    capture_output: "all_paths"

  # Aggregate numeric field
  - shell: |
      echo '${map.results}' | jq '[.[].coverage] | add / length'
    capture_output: "avg_coverage"

  # Filter and transform results
  - shell: |
      echo '${map.results}' | jq '[.[] | select(.item.priority > 5) | .item.name]'
    capture_output: "high_priority_items"
```

### Performance: Template Caching

Prodigy implements **dual caching** for optimal performance: template parsing cache and operation result cache.

**Source:** src/cook/execution/interpolation.rs:18-19, 68-75; src/cook/execution/variables.rs:218-256

#### Template Parse Caching

When the same variable template is used multiple times, the template is parsed once and reused:

**How It Works:**
- First use: Template is parsed and cached
- Subsequent uses: Cached template is reused (no re-parsing)
- Cache key: Exact template string
- Automatic: No configuration needed

**Example:**
```yaml
# Template "${item.path} --priority ${item.metadata.priority:-5}"
# is parsed once, then reused for all 1000 items
map:
  input: "items.json"  # 1000 items
  json_path: "$.items[*]"
  agent_template:
    - shell: "process ${item.path} --priority ${item.metadata.priority:-5}"
```

#### Computed Variable Caching

Expensive computed operations (file reads, command execution) have separate result caching:

**Cached Operations:**
- `${env.VAR}` - Environment variable lookups
- `${file:path}` - File system reads
- `${cmd:command}` - Shell command execution

**Not Cached:**
- `${json:path:from:var}` - JSON parsing is fast
- `${date:format}` - Values change over time
- `${uuid}` - Must be unique

**Cache Details:**
- **Type:** LRU (Least Recently Used) cache
- **Size:** 100 entries maximum
- **Scope:** Per workflow execution
- **Thread Safety:** Async RwLock protection

**Performance Impact:**
```yaml
# First shell command: Reads .commit-message.txt from disk
- shell: "git commit -m '${file:.commit-message.txt}'"

# Second shell command: Uses cached file content (no disk read)
- shell: "echo 'Message: ${file:.commit-message.txt}'"

# Third shell command: Still uses cache
- shell: "test -n '${file:.commit-message.txt}'"
```

**Benefits:**
- **Faster interpolation** for repeated templates (template cache)
- **Reduced I/O** for repeated file reads (operation cache)
- **Lower CPU** for repeated command execution (operation cache)
- **Reduced latency** in MapReduce workflows

**When It Matters Most:**
- MapReduce workflows with many work items (>100)
- Workflows using the same computed variables repeatedly
- High-frequency variable interpolation in loops
- Templates with multiple variables and nested field access

**Note:** All caching is transparent and automatic. You don't need any configuration to benefit from it. Both caches persist for the lifetime of the workflow execution.

