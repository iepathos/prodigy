## Available Variables

Prodigy provides a comprehensive set of built-in variables that are automatically available based on your workflow context. All variables use the `${variable.name}` interpolation syntax.

### Standard Variables

These variables capture output from the most recently executed command:

| Variable | Description | Example |
|----------|-------------|---------|
| `${last.output}` | Output from the last command of any type | `echo ${last.output}` |
| `${last.exit_code}` | Exit code from the last command | `if [ ${last.exit_code} -eq 0 ]` |
| `${shell.output}` | Output from the last shell command | `echo ${shell.output}` |
| `${claude.output}` | Output from the last Claude command | `echo ${claude.output}` |

**Example:**
```yaml
- shell: "cargo test --lib"
- shell: "echo 'Test output: ${shell.output}'"
```

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

Variables for accessing work item data during parallel processing:

| Variable | Description | Example |
|----------|-------------|---------|
| `${item}` | Full item object (as string) | `echo ${item}` |
| `${item.value}` | Item value for simple types | `process ${item.value}` |
| `${item.path}` | File path (for file inputs) | `cat ${item.path}` |
| `${item.name}` | Item display name | `echo "Processing ${item.name}"` |
| `${item_index}` | Zero-based item index | `Item ${item_index}` |
| `${item_total}` | Total number of items | `of ${item_total}` |
| `${item.*}` | Nested field access | `${item.priority}`, `${item.metadata.author}` |

**Available in:** Map phase only

**Example:**
```yaml
map:
  input: "files.json"
  json_path: "$.files[*]"
  agent_template:
    - shell: "echo 'Processing ${item.path} (${item_index}/${item_total})'"
    - claude: "/analyze '${item.path}' --priority ${item.priority}"
```

### MapReduce Variables (Reduce Phase Only)

Variables for accessing aggregated results from map phase:

| Variable | Description | Example |
|----------|-------------|---------|
| `${map.total}` | Total items in map phase | `echo "Processed ${map.total} items"` |
| `${map.successful}` | Successfully processed items | `echo "${map.successful} succeeded"` |
| `${map.failed}` | Failed items count | `echo "${map.failed} failed"` |
| `${map.results}` | All map results as JSON array | `echo '${map.results}' \| jq` |
| `${map.key}` | Key for map output (optional) | `${map.key}` |
| `${worker.id}` | Worker ID for tracking | `Worker ${worker.id}` |

**Available in:** Reduce phase only

**Example:**
```yaml
reduce:
  - shell: |
      echo '${map.results}' | jq '[.[] | select(.status == "error")] | length'
    capture_output: "error_count"
  - claude: "/summarize ${map.results} --total ${map.total}"
```

### Git Context Variables

Variables tracking git changes throughout workflow execution:

| Variable | Description | Example |
|----------|-------------|---------|
| `${step.files_added}` | Files added in current step | `echo ${step.files_added}` |
| `${step.files_modified}` | Files modified in current step | `echo ${step.files_modified}` |
| `${step.files_deleted}` | Files deleted in current step | `echo ${step.files_deleted}` |
| `${step.commits}` | Commits in current step | `echo ${step.commits}` |
| `${workflow.commits}` | All commits in workflow | `git show ${workflow.commits}` |
| `${workflow.commit_count}` | Total number of commits | `echo "${workflow.commit_count} commits"` |

**Available in:** All phases (requires git repository)

#### Format Modifiers

Git context variables support multiple output formats:

| Modifier | Description | Example |
|----------|-------------|---------|
| (default) | Space-separated list | `${step.files_added}` → `file1.rs file2.rs` |
| `:json` | JSON array format | `${step.files_added:json}` → `["file1.rs", "file2.rs"]` |
| `:newline` | Newline-separated list | `${step.files_added:newline}` → `file1.rs\nfile2.rs` |
| `:comma` | Comma-separated list | `${step.files_added:comma}` → `file1.rs,file2.rs` |
| `:*.ext` | Glob pattern filter | `${step.files_added:*.rs}` → only Rust files |
| `:path/**/*.ext` | Path with glob | `${step.files_added:src/**/*.rs}` → Rust files in src/ |

**Format Examples:**
```yaml
# JSON format for jq processing
- shell: "echo '${step.files_added:json}' | jq -r '.[]'"

# Newline format for iteration
- shell: |
    echo '${step.files_modified:newline}' | while read file; do
      cargo fmt "$file"
    done

# Glob filtering for language-specific operations
- shell: "cargo clippy ${step.files_modified:*.rs}"

# Multiple glob patterns
- shell: "git diff ${step.files_modified:*.rs,*.toml}"
```

### Merge Variables (Merge Phase Only)

Variables available during the merge phase when integrating worktree changes:

| Variable | Description | Example |
|----------|-------------|---------|
| `${merge.worktree}` | Worktree name being merged | `echo ${merge.worktree}` |
| `${merge.source_branch}` | Source branch from worktree | `git log ${merge.source_branch}` |
| `${merge.target_branch}` | Target branch (where you started) | `git merge ${merge.source_branch}` |
| `${merge.session_id}` | Session ID for correlation | `echo ${merge.session_id}` |

**Available in:** Merge phase only

**Example:**
```yaml
merge:
  commands:
    - shell: "git fetch origin"
    - shell: "git merge origin/${merge.target_branch}"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

### Validation Variables

Variables for workflow validation and completion tracking:

| Variable | Description | Example |
|----------|-------------|---------|
| `${validation.completion}` | Completion percentage (0-100) | `echo "${validation.completion}%"` |
| `${validation.gaps}` | Array of missing requirements | `echo '${validation.gaps}'` |
| `${validation.status}` | Status: complete/incomplete/failed | `if [ "${validation.status}" = "complete" ]` |

**Available in:** Goal seek and validation phases

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
6. **Environment variables**

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

