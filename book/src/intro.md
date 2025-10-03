# Introduction

Prodigy is an AI-powered workflow orchestration tool that enables development teams to automate complex tasks using Claude AI through structured YAML workflows.

## What is Prodigy?

Prodigy combines the power of Claude AI with workflow orchestration to:

- **Automate repetitive development tasks** - Code reviews, refactoring, testing
- **Process work in parallel** - MapReduce-style parallel execution across git worktrees
- **Maintain quality** - Built-in validation, error handling, and retry mechanisms
- **Track changes** - Full git integration with automatic commits and merge workflows

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

## Key Concepts

- **Workflows**: YAML files defining sequences of commands
- **Commands**: Shell commands, Claude AI invocations, or control flow
- **Variables**: Dynamic values captured and interpolated across steps
- **MapReduce**: Parallel processing across multiple git worktrees
- **Validation**: Automatic testing and quality checks

## Next Steps

- [Workflow Basics](workflow-basics.md) - Learn workflow fundamentals
- [Command Types](commands.md) - Explore available command types
- [Examples](examples.md) - See real-world workflows
