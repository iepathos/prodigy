# Advanced Git Context

Advanced git context features enable powerful filtering and formatting of git information in your workflows. This chapter covers automatic git tracking, variable modifiers, format options, and pattern filtering.

## Overview

Prodigy automatically tracks git changes throughout workflow execution and exposes them through variables. No configuration is needed—git context variables are available out-of-the-box in any git repository. You can access file changes, commits, and modification statistics at both the step and workflow level.

## How Git Tracking Works

### Automatic Tracking

Git context is automatically tracked when you run workflows in a git repository:

- **GitChangeTracker** is initialized at workflow start
- Each step's changes are tracked between `begin_step` and `complete_step`
- Variables are automatically available for interpolation in all commands
- No YAML configuration needed—tracking happens transparently

### When Tracking is Active

Git tracking is active in:
- Regular workflows running in git repositories
- MapReduce setup, map, and reduce phases
- Child worktrees created for map agents

Git tracking is **not** active in:
- Non-git repositories
- Workflows without git integration

## Git Context Variables

### Step-Level Variables

Track changes made during the current step:

```yaml
# Access files changed in this step
- shell: "echo Changed: ${step.files_changed}"
- shell: "echo Added: ${step.files_added}"
- shell: "echo Modified: ${step.files_modified}"
- shell: "echo Deleted: ${step.files_deleted}"

# Access commit information
- shell: "echo Commits: ${step.commits}"
- shell: "echo Commit count: ${step.commit_count}"

# Access modification statistics
- shell: "echo Insertions: ${step.insertions}"
- shell: "echo Deletions: ${step.deletions}"
```

### Workflow-Level Variables

Track cumulative changes across all steps:

```yaml
# Access all files changed in workflow
- shell: "echo Changed: ${workflow.files_changed}"
- shell: "echo Added: ${workflow.files_added}"
- shell: "echo Modified: ${workflow.files_modified}"
- shell: "echo Deleted: ${workflow.files_deleted}"

# Access all commits
- shell: "echo Commits: ${workflow.commits}"
- shell: "echo Commit count: ${workflow.commit_count}"

# Access total modifications
- shell: "echo Insertions: ${workflow.insertions}"
- shell: "echo Deletions: ${workflow.deletions}"
```

### Variable Reference

| Variable | Scope | Description |
|----------|-------|-------------|
| `step.files_added` | Step | Files added in current step |
| `step.files_modified` | Step | Files modified in current step |
| `step.files_deleted` | Step | Files deleted in current step |
| `step.files_changed` | Step | All files changed (added + modified + deleted) |
| `step.commits` | Step | Commit SHAs from current step |
| `step.commit_count` | Step | Number of commits in current step |
| `step.insertions` | Step | Lines added in current step |
| `step.deletions` | Step | Lines deleted in current step |
| `workflow.files_added` | Workflow | All files added in workflow |
| `workflow.files_modified` | Workflow | All files modified in workflow |
| `workflow.files_deleted` | Workflow | All files deleted in workflow |
| `workflow.files_changed` | Workflow | All files changed in workflow |
| `workflow.commits` | Workflow | All commit SHAs in workflow |
| `workflow.commit_count` | Workflow | Total commits in workflow |
| `workflow.insertions` | Workflow | Total lines added in workflow |
| `workflow.deletions` | Workflow | Total lines deleted in workflow |

## Pattern Filtering

Filter git context variables using glob patterns with the `:pattern` modifier syntax:

### Basic Pattern Filtering

```yaml
# Only Rust files added in this step
- shell: "echo ${step.files_added:*.rs}"

# Only source files changed in workflow
- shell: "echo ${workflow.files_changed:src/**/*.rs}"

# Multiple file types
- shell: "echo ${step.files_modified:**/*.{rs,toml}}"

# Module files only
- shell: "echo ${workflow.files_added:**/mod.rs}"
```

### Pattern Syntax

Use glob patterns to match files precisely:

- `*` - Match any characters except `/`
- `**` - Match any characters including `/`
- `?` - Match single character
- `{a,b}` - Match either `a` or `b`
- `[abc]` - Match character class

**Examples:**

```yaml
# Match Rust and TOML files
- shell: "echo ${step.files_changed:**/*.{rs,toml}}"

# Match module files in src/
- shell: "echo ${workflow.files_added:src/**/mod.rs}"

# Match integration tests
- shell: "echo ${step.files_modified:tests/integration/**}"

# Match any test files
- shell: "echo ${workflow.files_changed:**/*_test.rs}"
```

### Combining Filters

For complex filtering, use multiple variable references or shell commands:

```yaml
# Pass different file sets to different commands
- shell: "cargo fmt $(echo ${step.files_changed:*.rs})"
- shell: "markdownlint $(echo ${step.files_changed:*.md})"

# Combine with shell filtering
- shell: |
    files="${workflow.files_changed:src/**/*.rs}"
    echo "$files" | grep -v test | xargs cargo clippy
```

## Format Modifiers

Customize how git context variables are formatted:

### Default Format (Space-Separated)

By default, variables are space-separated:

```yaml
- shell: "echo ${step.files_changed}"
# Output: src/main.rs src/lib.rs tests/test.rs
```

### JSON Format

Use `:json` for JSON array output:

```yaml
- shell: "echo ${step.files_added:json}"
# Output: ["src/main.rs","src/lib.rs","tests/test.rs"]

# Parse with jq
- shell: "echo ${workflow.commits:json} | jq -r '.[]'"
```

### Newline-Separated Format

Use `:lines` or `:newline` for one item per line:

```yaml
- shell: "echo ${step.files_changed:lines}"
# Output:
# src/main.rs
# src/lib.rs
# tests/test.rs

# Useful with xargs
- shell: "echo ${workflow.files_modified:lines} | xargs -I {} cp {} backup/"
```

### Comma-Separated Format

Use `:csv` or `:comma` for comma-separated output:

```yaml
- shell: "echo ${step.files_added:csv}"
# Output: src/main.rs,src/lib.rs,tests/test.rs
```

### Combining Format and Pattern

Apply both pattern filtering and format modifiers:

```yaml
# JSON array of Rust files
- shell: "echo ${step.files_changed:*.rs:json}"

# Newline-separated source files
- shell: "echo ${workflow.files_added:src/**:lines}"

# CSV of modified test files
- shell: "echo ${step.files_modified:**/*_test.rs:csv}"
```

## Use Cases

### Code Review Workflows

Review only source code changes:

```yaml
- claude: "/review-changes"
  args:
    files: "${step.files_changed:src/**/*.rs}"

- shell: |
    echo "Reviewing ${step.commit_count} commits"
    echo "Changed files: ${step.files_changed:src/**/*.rs:lines}"
```

### Documentation Updates

Work with documentation changes:

```yaml
- claude: "/update-docs"
  args:
    docs: "${workflow.files_changed:**/*.md}"

- shell: "markdownlint ${step.files_modified:*.md}"

# Check if docs were updated
- when: "${workflow.files_changed:**/*.md}"
  shell: "echo Documentation was updated"
```

### Test Verification

Focus on test-related changes:

```yaml
# Run tests for changed test files
- shell: "cargo test $(echo ${step.files_changed:**/*_test.rs})"

# Verify test coverage
- when: "${step.files_added:tests/**}"
  claude: "/verify-test-coverage"
```

### Conditional Execution

Use git context in conditional logic:

```yaml
# Only run if Rust files changed
- when: "${step.files_changed:*.rs}"
  shell: "cargo clippy"

# Run different linters based on changes
- when: "${workflow.files_changed:*.rs}"
  shell: "cargo fmt --check"

- when: "${workflow.files_changed:*.md}"
  shell: "markdownlint **/*.md"

# Check commit count
- when: "${step.commit_count} > 1"
  shell: "echo Multiple commits detected"
```

### MapReduce Workflows

Git context works across MapReduce phases:

```yaml
name: review-changes
mode: mapreduce

setup:
  # Workflow-level tracking starts here
  - shell: "git diff main --name-only > changed-files.txt"
  - shell: "echo Setup modified: ${step.files_changed}"

map:
  input: "changed-files.txt"
  agent_template:
    # Each agent has its own step tracking
    - claude: "/review ${item}"
    - shell: "echo Agent changed: ${step.files_changed}"

reduce:
  # Access workflow-level changes from all agents
  - shell: "echo Total changes: ${workflow.files_changed}"
  - shell: "echo Total commits: ${workflow.commit_count}"
```

## Best Practices

- **Use Pattern Filtering**: Filter variables to only relevant files using glob patterns
- **Choose Appropriate Format**: Use `:json` for parsing, `:lines` for iteration, default for simple commands
- **Scope Appropriately**: Use `step.*` for current changes, `workflow.*` for cumulative tracking
- **Combine with Conditionals**: Use `when:` to execute steps only when relevant files change
- **Test Your Patterns**: Verify patterns match intended files with `echo ${var:pattern}`
- **Document Intent**: Add comments explaining why specific patterns are used

## Performance Considerations

- Git operations are performed once per step and cached
- Pattern filtering is efficient using compiled glob patterns
- Variables are computed on-demand during interpolation
- Workflow-level tracking maintains cumulative state without re-scanning

## Troubleshooting

### Pattern Not Matching Files

**Issue**: Your pattern doesn't match any files

```yaml
# Debug: Echo the unfiltered variable first
- shell: "echo All files: ${step.files_changed}"
- shell: "echo Filtered: ${step.files_changed:*.rs}"
```

**Solution**: Use `git ls-files` to verify file paths match your pattern

### Empty Variables

**Issue**: Git context variables are empty

**Possible causes:**
- Not running in a git repository
- No commits have been made in the current step
- Pattern filter is too restrictive

**Solution**: Check if git tracking is active and verify patterns

### Format Modifier Not Working

**Issue**: Format modifier produces unexpected output

```yaml
# Verify correct syntax: variable:pattern:format
- shell: "echo ${step.files_added:*.rs:json}"
```

**Solution**: Ensure proper syntax with colon separators and valid format name (json, lines, csv)

### Variables Not Interpolating

**Issue**: Variables appear as literal strings like `${step.files_changed}`

**Solution**: Ensure you're using proper YAML syntax and the command supports interpolation

## See Also

- [Variables and Interpolation](variables.md) - Basic variable usage and interpolation syntax
- [Workflow Basics](workflow-basics.md) - Git integration fundamentals and workflow structure
- [MapReduce Workflows](mapreduce.md) - Using git context in parallel jobs
- [Conditional Execution](conditionals.md) - Using git context with `when:` conditions
