# Git Context Best Practices

This page covers best practices, performance considerations, troubleshooting, and planned future features for git context in Prodigy workflows.

## Best Practices

!!! tip "Use Shell Filtering"
    Filter variables to only relevant files using `grep`, `tr`, and other shell utilities. Git context variables are space-separated strings, so shell processing is the recommended approach.

    ```bash
    echo "${step.files_changed}" | tr ' ' '\n' | grep '\.rs$'
    ```

!!! tip "Choose Appropriate Format"
    Convert to JSON with `jq`, newlines with `tr`, or CSV for different use cases depending on your downstream processing needs.

    === "JSON"
        ```bash
        echo "${step.files_added}" | tr ' ' '\n' | jq -R | jq -s
        ```

    === "Newlines"
        ```bash
        echo "${step.files_changed}" | tr ' ' '\n'
        ```

    === "CSV"
        ```bash
        echo "${step.files_changed}" | tr ' ' ','
        ```

!!! tip "Scope Appropriately"
    Use `step.*` variables for current changes within a single step, and `workflow.*` for cumulative tracking across all steps.

    | Scope | Use When |
    |-------|----------|
    | `step.files_changed` | Processing files from the most recent step |
    | `workflow.files_changed` | Tracking all files modified during entire workflow |

!!! tip "Handle Empty Results"
    Always check if filtered results are non-empty before using them to avoid errors in downstream commands.

    ```bash
    filtered=$(echo "${step.files_changed}" | tr ' ' '\n' | grep '\.rs$')
    if [ -n "$filtered" ]; then
      cargo fmt $filtered
    fi
    ```

!!! tip "Test Your Filters"
    Debug with `echo` commands to verify filtering works as expected before using in production workflows.

!!! tip "Document Intent"
    Add comments explaining complex shell filtering pipelines for maintainability.

!!! tip "Combine Operations"
    Chain `tr`, `grep`, and `jq` for powerful filtering and formatting in a single pipeline.

## Performance Considerations

!!! info "Caching"
    Git operations are performed once per step and cached in the `GitChangeTracker` (see `src/cook/workflow/git_context.rs`). Repeated access to the same variables doesn't trigger additional git operations.

!!! info "Pre-formatting"
    Variables are pre-formatted as space-separated strings when added to the interpolation context. This means the formatting happens once during context creation, not on each variable access.

!!! warning "Shell Filtering Overhead"
    Shell filtering happens at runtime, so complex filter pipelines may add overhead. For frequently-used complex filters, consider extracting them to helper scripts.

!!! info "Cumulative Tracking"
    Workflow-level tracking maintains cumulative state efficiently without re-scanning git history. The tracker merges change sets as each step completes.

!!! info "Fast Resolution"
    Variable resolution is fast since values are pre-computed strings. No git operations occur during interpolation.

## Troubleshooting

### Filter Not Matching Any Files

!!! question "Issue"
    Your grep filter doesn't match any files.

**Debug approach**: Echo the unfiltered variable first to see what's available.

```yaml
# Debug: Echo the unfiltered variable first
- shell: "echo All files: ${step.files_changed}"
- shell: |
    filtered=$(echo "${step.files_changed}" | tr ' ' '\n' | grep '\.rs$')
    echo "Filtered: $filtered"
```

!!! note "Expected Behavior"
    When a filter matches no files, the variable is empty. This is normal and expected.

!!! success "Solution"
    Always check if filtered results are non-empty:

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

!!! question "Issue"
    Git context variables are empty.

!!! warning "Possible Causes"
    - Not running in a git repository
    - No commits have been made in the current step
    - Git tracking not initialized

!!! success "Solution"
    Verify git tracking is active:

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

!!! question "Issue"
    Trying to use `:*.rs` or `:json` modifiers produces errors or unexpected results.

!!! warning "Cause"
    Pattern filtering and format modifiers are **not implemented** in variable interpolation. Git context variables are always space-separated strings.

!!! failure "What Doesn't Work"
    ```yaml
    # These do NOT work - modifiers not implemented
    - shell: "echo ${step.files_changed:*.rs}"
    - shell: "echo ${step.files_added:json}"
    - shell: "echo ${workflow.files_modified:lines}"
    ```

!!! success "Solution"
    Use shell commands for all filtering and formatting:

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

!!! question "Issue"
    Variables appear as literal strings like `${step.files_changed}`.

!!! warning "Possible Causes"
    - Variable name misspelled
    - Using unsupported variable
    - YAML quoting issues

!!! success "Solution"
    Verify the variable name and use proper quoting:

    ```yaml
    # Correct syntax
    - shell: "echo ${step.files_changed}"
    - shell: |
        echo "${workflow.files_modified}"
    ```

### Shell Filtering Complexity

!!! question "Issue"
    Shell filtering pipelines are getting too complex.

!!! success "Solution"
    Extract complex filtering to separate shell scripts:

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

!!! info "Planned Enhancements"
    The git context infrastructure includes methods that are not yet exposed to workflows. These are planned for future releases.

### Pattern Filtering (Planned)

The `GitChangeTracker::resolve_variable()` method (`src/cook/workflow/git_context.rs:489-505`) supports pattern filtering, but it's not currently called during workflow execution.

!!! abstract "Planned Syntax"
    ```yaml
    # Not yet implemented - planned for future release
    - shell: "echo ${step.files_changed:*.rs}"
    - shell: "echo ${workflow.files_modified:src/**/*.rs}"
    ```

Currently variables are pre-formatted as space-separated strings during interpolation context creation (`src/cook/workflow/executor/context.rs:106-172`).

### Format Modifiers (Planned)

The `GitChangeTracker::format_file_list()` method (`src/cook/workflow/git_context.rs:477-486`) supports JSON, newline, and CSV formats, but it's not used during variable resolution.

!!! abstract "Planned Syntax"
    ```yaml
    # Not yet implemented - planned for future release
    - shell: "echo ${step.files_added:json}"
    - shell: "echo ${workflow.files_changed:lines}"
    - shell: "echo ${step.files_modified:csv}"
    ```

### Implementation Note

!!! note "Technical Details"
    To enable these features, the interpolation engine would need to support custom resolvers that call `git_tracker.resolve_variable()` instead of using pre-formatted string values. This would allow runtime formatting and filtering based on variable modifier syntax.

**Until then**, use shell post-processing as documented in [Shell-Based Filtering and Formatting](shell-filtering.md).
