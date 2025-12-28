# Advanced Git Context

This chapter covers automatic git tracking and git context variables in Prodigy workflows. Learn how to access file changes, commits, and modification statistics, and how to filter and format this data using shell commands.

> **Warning: Current Implementation Status**
>
> Git context variables are currently provided as **space-separated strings only**. Advanced features like pattern filtering (`:*.rs`) and format modifiers (`:json`, `:lines`) are **not yet implemented** in the variable interpolation system, though the underlying infrastructure exists.
>
> **For filtering and formatting**, use shell post-processing commands like `grep`, `tr`, `jq`, and `xargs`. See [Shell-Based Filtering and Formatting](shell-filtering.md) for practical examples.

## Contents

- [Overview](overview.md) - How git tracking works and available variables
- [Shell-Based Filtering and Formatting](shell-filtering.md) - Filter and format git context using shell commands
- [Use Cases](use-cases.md) - Practical workflow patterns for code review, documentation, and testing
- [Best Practices](best-practices.md) - Performance tips, troubleshooting, and future features
