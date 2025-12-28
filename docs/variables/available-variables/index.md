# Available Variables

Prodigy provides a comprehensive set of built-in variables that are automatically available based on your workflow context. All variables use the `${variable.name}` interpolation syntax.

## Variable Categories

<div class="grid cards" markdown>

-   :material-format-text:{ .lg .middle } **Standard and Computed Variables**

    ---

    Core output variables, environment access, file content, command output, JSON extraction, date formatting, and UUID generation

    [:octicons-arrow-right-24: Standard Variables](standard-variables.md)

-   :material-map:{ .lg .middle } **MapReduce Variables**

    ---

    Item variables for the map phase and aggregated result variables for the reduce phase

    [:octicons-arrow-right-24: MapReduce Variables](mapreduce-variables.md)

-   :material-source-branch:{ .lg .middle } **Git and Merge Variables**

    ---

    Git context tracking, file change monitoring, and merge phase variables

    [:octicons-arrow-right-24: Git and Merge Variables](git-merge-variables.md)

-   :material-code-braces:{ .lg .middle } **Interpolation Reference**

    ---

    Syntax reference, legacy aliases, scoping rules, precedence, and performance optimization

    [:octicons-arrow-right-24: Interpolation Reference](interpolation-reference.md)

</div>

## Quick Reference

### Standard Variables

| Variable | Description |
|----------|-------------|
| `${last.output}` | Output from the last command of any type |
| `${shell.output}` | Output from the last shell command |
| `${claude.output}` | Output from the last Claude command |
| `${env.VAR_NAME}` | Read environment variable |
| `${file:path}` | Read file contents |
| `${cmd:command}` | Execute command and capture output |
| `${date:format}` | Current date/time with format |
| `${uuid}` | Generate random UUID v4 |

### MapReduce Variables

| Variable | Phase | Description |
|----------|-------|-------------|
| `${item.*}` | Map | Access work item fields |
| `${map.total}` | Reduce | Total items processed |
| `${map.successful}` | Reduce | Successfully processed items |
| `${map.results}` | Reduce | All map results as JSON |

### Git Variables

| Variable | Description |
|----------|-------------|
| `${step.files_modified}` | Files modified in current step |
| `${step.commits}` | Commits in current step |
| `${workflow.commit_count}` | Total commits in workflow |
| `${merge.source_branch}` | Source branch from worktree |
