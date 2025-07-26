# PROJECT.md - Memento Mori (mmm)

## Overview

Memento Mori (mmm) is a Rust CLI tool for implementing self-sufficient loops with Claude CLI, enabling automated specification processing and iterative development.

## Current State

- **Project Status**: Active Development
- **Core Features**: Specification engine, workflow automation, Claude integration
- **Latest Version**: 0.1.0
- **Implementation Progress**: 48% (Specs 01, 08, 09, 10, 11, 12 completed)

## What Exists

### Core Components
- **Specification Engine**: Parse and execute development specifications
- **Workflow Engine**: YAML-based workflow automation with conditions
- **Claude Integration**: API client with caching and context management
- **Command System**: Extensible command registry and dispatcher
- **State Management**: Simple JSON-based state persistence (replaced SQLite)
- **Monitoring**: Analytics, metrics, and performance tracking
- **Plugin System**: Dynamic plugin loading with security sandbox
- **Iterative Improvement Loop**: Automated code quality improvement cycles
- **Dead Simple Improve**: Zero-configuration code improvement command
- **Smart Project Analyzer**: Automatic language, framework, and quality detection
- **Simple State Management**: Human-readable JSON state files with caching and learning
- **Developer Experience**: Beautiful progress displays, interactive improvements, smart suggestions

### Project Structure
```
mmm/
├── .claude/           # Claude CLI custom commands
├── .mmm/              # Project context files
│   ├── config.toml    # Project-specific configuration
│   ├── logs/          # Execution logs directory
│   └── *.md           # Context documentation files
├── specs/             # Development specifications
├── src/               # Rust source code
├── templates/         # Workflow templates
├── migrations/        # Database migrations
└── ~/.mmm/            # Global configuration
    ├── config.toml    # Global settings
    ├── projects/      # Project registry
    └── templates/     # Global templates
```

## Key Capabilities

1. **Specification Processing**
   - Load and parse Markdown specifications
   - Execute specs iteratively with Claude
   - Track progress and completion state

2. **Workflow Automation**
   - YAML workflow definitions
   - Conditional execution with Pest parser
   - Checkpoint and state management

3. **Claude Integration**
   - API client with retry logic
   - Response caching
   - Context window management
   - Token usage tracking

4. **Project Management**
   - Health checks and validation
   - Template-based project creation
   - Specification lifecycle management

5. **Monitoring & Analytics**
   - Performance metrics collection
   - Dashboard visualization
   - Alert system for issues
   - Export capabilities

6. **Iterative Improvement Loops**
   - Automated code quality improvement cycles
   - Integration with Claude CLI for review and improvement
   - Session management and progress tracking
   - Termination condition evaluation
   - Structured output for automation

## Technology Stack

- **Language**: Rust (2021 edition)
- **CLI Framework**: Clap v4
- **Async Runtime**: Tokio
- **Database**: JSON files (replaced SQLite)
- **Serialization**: Serde (JSON, YAML, TOML)
- **Web Framework**: Axum (for dashboard)
- **Parsing**: Pest (for conditions)

## Development Philosophy

- **Self-Sufficient Loops**: Enable automated development cycles
- **Specification-Driven**: Use specs as source of truth
- **Extensible**: Plugin system for custom functionality
- **Observable**: Comprehensive monitoring and analytics
- **Secure**: Sandboxed plugin execution

## Next Steps

See ROADMAP.md for planned features and development priorities.