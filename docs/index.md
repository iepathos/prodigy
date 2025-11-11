# Introduction

Prodigy is an AI-powered workflow orchestration tool that enables development teams to automate complex tasks using Claude AI through structured YAML workflows.

## What is Prodigy?

Prodigy combines the power of Claude AI with workflow orchestration to:

- **Automate repetitive development tasks** - Code reviews, refactoring, testing
- **Process work in parallel** - MapReduce-style parallel execution across git worktrees
- **Resume long-running operations** - Checkpoint and resume capabilities for workflows that span hours or days
- **Handle failures gracefully** - Dead Letter Queue (DLQ) for automated retry of failed items
- **Maintain quality** - Built-in validation, error handling, and retry mechanisms
- **Track changes** - Full git integration with automatic commits and merge workflows
- **Generate living documentation** - Keep docs synchronized with code automatically

!!! tip "Production-Ready Features"
    Prodigy includes enterprise features like checkpoints for resuming interrupted workflows, a Dead Letter Queue for automatic failure recovery, and git worktree isolation to keep your main repository clean during execution.

## Quick Start

Create a simple workflow in `workflow.yml`:

```yaml
# Source: workflows/complex-build-pipeline.yml
name: build-and-test

- shell: "cargo build"
- shell: "cargo test"
  on_failure:
    claude: "/fix-failing-tests"
- shell: "cargo clippy"
```

Run it:

```bash
prodigy run workflow.yml
```

## Documentation Features

This book itself is maintained using Prodigy's automated documentation system! Learn how to set up automated, always-up-to-date documentation for your own project:

- [Automated Documentation Overview](automated-documentation/index.md) - How it works
- [Quick Start (15 minutes)](automated-documentation/quick-start.md) - Fast setup
- [Tutorial (30 minutes)](automated-documentation/tutorial.md) - Comprehensive guide

## Key Concepts

- **Workflows**: YAML files defining sequences of commands
- **Commands**: Shell commands, Claude AI invocations, or control flow
- **Variables**: Dynamic values captured (json, lines, number) and interpolated with defaults, aliases, and nested field access
- **Environment**: Configuration with secrets management and profile-based values
- **MapReduce**: Parallel processing across multiple git worktrees
- **Checkpoints**: Save and resume workflow state for long-running operations
- **Validation**: Automated testing and implementation completeness checking

## Next Steps

- [Workflow Basics](workflow-basics/index.md) - Learn workflow fundamentals
- [MapReduce Workflows](mapreduce/index.md) - Parallel processing at scale
- [Command Types](commands.md) - Explore available command types
- [Examples](examples.md) - See real-world workflows
