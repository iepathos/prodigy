# Git Context Best Practices

This page covers best practices, performance considerations, troubleshooting, and planned future features for git context in Prodigy workflows.

## Best Practices

- **Use Shell Filtering**: Filter variables to only relevant files using `grep`, `tr`, and other shell utilities
- **Choose Appropriate Format**: Convert to JSON with `jq`, newlines with `tr`, or CSV for different use cases
- **Scope Appropriately**: Use `step.*` for current changes, `workflow.*` for cumulative tracking
- **Handle Empty Results**: Always check if filtered results are non-empty before using them
- **Test Your Filters**: Debug with `echo` commands to verify filtering works as expected
- **Document Intent**: Add comments explaining complex shell filtering pipelines
- **Combine Operations**: Chain `tr`, `grep`, and `jq` for powerful filtering and formatting

## Performance Considerations

- Git operations are performed once per step and cached (src/cook/workflow/git_context.rs)
- Variables are pre-formatted when added to the interpolation context
- Shell filtering happens at runtime, so complex filters may add overhead
- Workflow-level tracking maintains cumulative state without re-scanning git history
- Variable resolution is fast since values are pre-computed strings

## Troubleshooting

### Filter Not Matching Any Files

**Issue**: Your grep filter doesn't match any files

```yaml
# Debug: Echo the unfiltered variable first
- shell: "echo All files: ${step.files_changed}"
- shell: |
    filtered=$(echo "${step.files_changed}" | tr ' ' '\n' | grep '\.rs$')
    echo "Filtered: $filtered"
```

**What happens**: When a filter matches no files, the variable is empty. This is expected behavior.

**Solution**: Always check if filtered results are non-empty:

```yaml
- shell: |
    rust_files=$(echo "${step.files_changed}" | tr ' ' '\n' | grep '\.rs$' | tr '\n' ' ')
    if [ -n "$rust_files" ]; then
      cargo fmt $rust_files
    else
      echo "No Rust files changed"
    fi
```

### Empty Git Context Variables

**Issue**: Git context variables are empty

**Possible causes:**
- Not running in a git repository
- No commits have been made in the current step
- Git tracking not initialized

**Solution**: Verify git tracking is active:

```yaml
# Check if variables are populated
- shell: |
    echo "Step files changed: ${step.files_changed}"
    echo "Workflow files changed: ${workflow.files_changed}"
    echo "Commit count: ${step.commit_count}"
```

If all are empty, check:
1. Are you in a git repository? (`git status`)
2. Has the step made any commits yet?
3. Is git tracking active for this workflow type?

### Pattern Syntax Not Working

**Issue**: Trying to use `:*.rs` or `:json` modifiers produces errors or unexpected results

**Cause**: Pattern filtering and format modifiers are **not implemented** in variable interpolation. Git context variables are always space-separated strings.

**What you tried** (doesn't work):
```yaml
# These do NOT work - modifiers not implemented
- shell: "echo ${step.files_changed:*.rs}"
- shell: "echo ${step.files_added:json}"
- shell: "echo ${workflow.files_modified:lines}"
```

**Solution**: Use shell commands for all filtering and formatting:

```yaml
# Filter with grep
- shell: "echo ${step.files_changed} | tr ' ' '\n' | grep '\.rs$'"

# Format as JSON
- shell: "echo ${step.files_added} | tr ' ' '\n' | jq -R | jq -s"

# Format as newlines
- shell: "echo ${workflow.files_modified} | tr ' ' '\n'"
```

See [Shell-Based Filtering and Formatting](shell-filtering.md) for complete examples.

### Variables Not Interpolating

**Issue**: Variables appear as literal strings like `${step.files_changed}`

**Possible causes:**
- Variable name misspelled
- Using unsupported variable
- YAML quoting issues

**Solution**: Verify the variable name and use proper quoting:

```yaml
# Correct syntax
- shell: "echo ${step.files_changed}"
- shell: |
    echo "${workflow.files_modified}"
```

### Shell Filtering Complexity

**Issue**: Shell filtering pipelines are getting too complex

**Solution**: Extract complex filtering to separate shell scripts:

```yaml
# Create a helper script
- shell: |
    cat > /tmp/filter-rust.sh <<'EOF'
    #!/bin/bash
    echo "$1" | tr ' ' '\n' | grep '\.rs$' | tr '\n' ' '
    EOF
    chmod +x /tmp/filter-rust.sh

# Use the helper
- shell: |
    rust_files=$(/tmp/filter-rust.sh "${step.files_changed}")
    if [ -n "$rust_files" ]; then
      cargo clippy $rust_files
    fi
```

## Future Features

The git context infrastructure includes methods that are not yet exposed to workflows. These are planned for future releases:

### Pattern Filtering (Planned)

The `GitChangeTracker::resolve_variable()` method (src/cook/workflow/git_context.rs:489-505) supports pattern filtering, but it's not currently called during workflow execution.

**Planned syntax**:
```yaml
# Not yet implemented - planned for future release
- shell: "echo ${step.files_changed:*.rs}"
- shell: "echo ${workflow.files_modified:src/**/*.rs}"
```

Currently variables are pre-formatted as space-separated strings during interpolation context creation (src/cook/workflow/executor/context.rs:106-172).

### Format Modifiers (Planned)

The `GitChangeTracker::format_file_list()` method (src/cook/workflow/git_context.rs:477-486) supports JSON, newline, and CSV formats, but it's not used during variable resolution.

**Planned syntax**:
```yaml
# Not yet implemented - planned for future release
- shell: "echo ${step.files_added:json}"
- shell: "echo ${workflow.files_changed:lines}"
- shell: "echo ${step.files_modified:csv}"
```

### Implementation Note

To enable these features, the interpolation engine would need to support custom resolvers that call `git_tracker.resolve_variable()` instead of using pre-formatted string values. This would allow runtime formatting and filtering based on variable modifier syntax.

**Until then**, use shell post-processing as documented in [Shell-Based Filtering and Formatting](shell-filtering.md).
