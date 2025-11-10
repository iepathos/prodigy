# Introduction

Prodigy is an AI-powered workflow orchestration tool that enables development teams to automate complex tasks using Claude AI through structured YAML workflows.

## What is Prodigy?

Prodigy combines the power of Claude AI with workflow orchestration to:

- **Automate repetitive development tasks** - Code reviews, refactoring, testing
- **Process work in parallel** - MapReduce-style parallel execution across git worktrees
- **Maintain quality** - Built-in validation, error handling, and retry mechanisms
- **Track changes** - Full git integration with automatic commits and merge workflows
- **Generate living documentation** - Keep docs synchronized with code automatically

## Quick Start

Create a simple workflow in `workflow.yml`:

```yaml
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
- **Variables**: Dynamic values captured and interpolated across steps
- **MapReduce**: Parallel processing across multiple git worktrees
- **Validation**: Automatic testing and quality checks

## Next Steps

- [Workflow Basics](workflow-basics/index.md) - Learn workflow fundamentals
- [MapReduce Workflows](mapreduce/index.md) - Parallel processing at scale
- [Command Types](commands.md) - Explore available command types
- [Examples](examples.md) - See real-world workflows
