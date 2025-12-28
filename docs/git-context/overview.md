# Git Context Overview

This page covers the fundamentals of git tracking in Prodigy workflows and the available git context variables.

## Overview

Prodigy automatically tracks git changes throughout workflow execution and exposes them through variables. No configuration is needed—git context variables are available out-of-the-box in any git repository. You can access file changes, commits, and modification statistics at both the step and workflow level.

!!! tip "Zero Configuration"
    Git context tracking is automatic. If you're running a workflow in a git repository, all git context variables are available immediately without any YAML configuration.

**What you get:**

- Automatic tracking of all git changes during workflow execution
- Variables for step-level changes (current command) and workflow-level changes (cumulative)
- Simple space-separated format ready for shell commands
- Full integration with MapReduce workflows

## How Git Tracking Works

### Automatic Tracking

Git context is automatically tracked when you run workflows in a git repository:

- **GitChangeTracker** is initialized at workflow start
- Each step's changes are tracked between `begin_step` and `complete_step` calls
- Variables are pre-formatted as space-separated strings and added to the interpolation context
- No YAML configuration needed—tracking happens transparently

??? info "Technical Details"
    When preparing the interpolation context for each command, git variables are added like this:

    ```rust
    // Source: src/cook/workflow/executor/context.rs:96-172
    // Variables are pre-formatted as space-separated strings
    context.set("step.files_added", Value::String(changes.files_added.join(" ")));
    context.set("step.files_modified", Value::String(changes.files_modified.join(" ")));
    // ... etc for all git context variables
    ```

    This means custom formatting must be done using shell commands after variable interpolation. See [Shell-Based Filtering](shell-filtering.md) for formatting techniques.

### When Tracking is Active

Git tracking is active in:

- Regular workflows running in git repositories
- MapReduce setup, map, and reduce phases
- Child worktrees created for map agents

!!! warning "Non-Git Repositories"
    Git tracking is **not** active in non-git repositories or workflows without git integration. Variables will be empty strings in these cases.

## Git Context Variables

!!! note "Space-Separated Format"
    All git context variables are provided as **space-separated strings**. This format works directly with most shell commands. For other formats (JSON, newlines, CSV) or filtering by file type, see [Shell-Based Filtering](shell-filtering.md).

### Step-Level Variables

Track changes made during the current step:

=== "File Changes"
    ```yaml
    # Access files changed in this step
    - shell: "echo Changed: ${step.files_changed}"
    - shell: "echo Added: ${step.files_added}"
    - shell: "echo Modified: ${step.files_modified}"
    - shell: "echo Deleted: ${step.files_deleted}"
    ```

=== "Commit Info"
    ```yaml
    # Access commit information
    - shell: "echo Commits: ${step.commits}"
    - shell: "echo Commit count: ${step.commit_count}"
    ```

=== "Statistics"
    ```yaml
    # Access modification statistics
    - shell: "echo Insertions: ${step.insertions}"
    - shell: "echo Deletions: ${step.deletions}"
    ```

### Workflow-Level Variables

Track cumulative changes across all steps:

=== "File Changes"
    ```yaml
    # Access all files changed in workflow
    - shell: "echo Changed: ${workflow.files_changed}"
    - shell: "echo Added: ${workflow.files_added}"
    - shell: "echo Modified: ${workflow.files_modified}"
    - shell: "echo Deleted: ${workflow.files_deleted}"
    ```

=== "Commit Info"
    ```yaml
    # Access all commits
    - shell: "echo Commits: ${workflow.commits}"
    - shell: "echo Commit count: ${workflow.commit_count}"
    ```

=== "Statistics"
    ```yaml
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

## Related Pages

- **[Shell-Based Filtering](shell-filtering.md)** - Format and filter git context variables using shell commands
- **[Use Cases](use-cases.md)** - Practical workflow patterns for code review, testing, and documentation
- **[Best Practices](best-practices.md)** - Performance tips, troubleshooting, and recommendations
