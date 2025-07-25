# MMM Documentation

This directory contains comprehensive documentation for the MMM (Memento Mori) project - a Rust CLI tool for implementing self-sufficient development loops with Claude CLI.

## Documentation Overview

### Core Concepts
- **[Development Loop Flow](development-loop-flow.md)** - Detailed explanation of how MMM creates and manages automated development cycles

### Architecture Documents
For detailed technical specifications, see the main specs directory:
- [Core Architecture](../specs/01-core-architecture.md) - Multi-project support, state management, configuration
- [Project Management](../specs/02-project-management.md) - Project lifecycle, templates, health checks
- [Claude Integration](../specs/03-claude-integration.md) - Advanced prompting, context optimization, response processing
- [Workflow Automation](../specs/04-workflow-automation.md) - YAML workflows, conditional execution, parallel processing
- [Monitoring & Reporting](../specs/05-monitoring-reporting.md) - Analytics, dashboards, performance tracking
- [Plugin System](../specs/06-plugin-system.md) - Extensible architecture, marketplace, security

## Quick Start

1. **Understanding the Flow**: Start with [Development Loop Flow](development-loop-flow.md) to understand how MMM operates
2. **Core Architecture**: Review the [Core Architecture spec](../specs/01-core-architecture.md) for technical foundation
3. **Examples**: Check the [example specification](../specs/example-feature.md) for spec format

## Key Features

- **Self-Sufficient Loops**: Automated development cycles with minimal human intervention
- **Multi-Project Support**: Manage multiple projects with isolated configurations
- **Intelligent Context Management**: Priority-based context optimization for Claude interactions  
- **Workflow Automation**: YAML-based multi-stage development processes
- **State Persistence**: SQLite-backed state management with recovery capabilities
- **Plugin Architecture**: Extensible system for custom commands and integrations

## Development Process

MMM follows a specification-driven development approach:

1. **Specifications**: Write markdown specs with clear objectives and acceptance criteria
2. **Automation**: MMM processes specs through iterative Claude interactions
3. **Validation**: Automatic quality checks, testing, and human review checkpoints
4. **Completion**: Specs are marked complete when all criteria are met

## Getting Help

- **Issues**: Report bugs and feature requests in the GitHub repository
- **Specs**: All technical details are documented in the `specs/` directory
- **Examples**: See `specs/example-feature.md` for specification format
- **Configuration**: Check `mmm.toml` for configuration options

## File Organization

```
docs/
├── README.md                    # This file - documentation overview
└── development-loop-flow.md     # Core development loop explanation

specs/                           # Technical specifications
├── 01-core-architecture.md      # Foundation and data models
├── 02-project-management.md     # Project lifecycle management  
├── 03-claude-integration.md     # Claude API integration details
├── 04-workflow-automation.md    # YAML workflow system
├── 05-monitoring-reporting.md   # Analytics and monitoring
├── 06-plugin-system.md         # Plugin architecture
└── example-feature.md          # Example spec format

src/                            # Implementation
├── claude/                     # Claude integration modules
├── workflow/                   # Workflow engine
├── state/                      # State management
└── ...
```

## Contributing

When adding documentation:
1. Keep it clear and actionable
2. Include code examples where relevant
3. Cross-reference related specs and docs
4. Update this README when adding new documents