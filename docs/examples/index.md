# Examples

> **Last Verified**: 2025-01-11 against codebase commit 753f90e0
>
> All examples in this chapter have been validated against the current implementation. Field names, syntax, and configuration options are verified against source code definitions.

This chapter demonstrates practical Prodigy workflows with real-world examples. Examples progress from simple to advanced, covering standard workflows, MapReduce parallel processing, error handling, and advanced features.

## Quick Reference

Find the right example for your use case:

| Use Case | Example | Key Features |
|----------|---------|--------------|
| Simple build/test pipeline | Example 1 | Basic commands, error handling |
| Loop over configurations | Example 2 | Foreach iteration, parallel processing |
| Parallel code processing | Example 3, 7 | MapReduce, distributed work |
| Conditional logic | Example 4 | Capture output, when clauses |
| Multi-step validation | Example 5 | Validation with gap filling |
| Environment configuration | Example 6 | Env vars, secrets, profiles |
| Dead Letter Queue (DLQ) | Example 7 | Error handling, retry failed items |
| Generate config files | Example 8 | write_file with JSON/YAML/text |
| Advanced git tracking | Example 9 | Git context variables, working_dir |
| External service resilience | Example 10 | Circuit breakers, fail fast |
| Retry with backoff | Example 11 | Exponential/linear/custom backoff |
| Reusable workflows | Example 12 | Composition (preview feature) |
| Custom merge process | Example 13 | Merge workflows, pre-merge validation |

## Example Categories

<div class="grid cards" markdown>

-   :material-rocket-launch:{ .lg .middle } **Basic Workflows**

    ---

    Simple linear workflows, foreach iteration, and parallel code review

    [:octicons-arrow-right-24: Basic Workflows](basic-workflows.md)

-   :material-source-branch:{ .lg .middle } **Conditional Workflows**

    ---

    Conditional deployment, multi-step validation, and environment-aware workflows

    [:octicons-arrow-right-24: Conditional Workflows](conditional-workflows.md)

-   :material-map:{ .lg .middle } **MapReduce Examples**

    ---

    Complex MapReduce with error handling and configuration file generation

    [:octicons-arrow-right-24: MapReduce Examples](mapreduce-examples.md)

-   :material-cog:{ .lg .middle } **Advanced Examples**

    ---

    Advanced features, circuit breakers, and retry configuration with backoff strategies

    [:octicons-arrow-right-24: Advanced Examples](advanced-examples.md)

-   :material-puzzle:{ .lg .middle } **Composition Examples**

    ---

    Workflow composition (preview) and custom merge workflows

    [:octicons-arrow-right-24: Composition Examples](composition-examples.md)

</div>
