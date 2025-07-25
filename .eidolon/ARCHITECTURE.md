# MMM Architecture

## System Overview
Memento Mori Manager is designed as a modular, extensible project management system with the following core principles:
- Specification-driven development
- Git-based state tracking
- Plugin architecture for extensibility
- Claude integration for automation
- Multi-project support with isolation

## Core Components

### 1. Project Manager
- Handles project registration, discovery, and context switching
- Manages project templates and scaffolding
- Project health monitoring and validation
- Multi-project operations and bulk actions
- Location: `src/project/`
  - `manager.rs`: Core project management functionality
  - `template.rs`: Template system with built-in templates
  - `health.rs`: Project health check system

### 2. Specification Engine
- Parses specifications with frontmatter support
- Manages spec dependencies and ordering
- Handles spec templates and dynamic generation
- Location: `src/spec/`

### 3. State Manager
- SQLite backend for complex state queries
- Supports state versioning and history
- Checkpoint and rollback capabilities
- Location: `src/state/`

### 4. Configuration System
- TOML-based hierarchical configuration
- Environment variable overrides
- Hot-reload support
- Location: `src/config/`

### 5. Command Dispatcher
- Plugin-based command system
- Command aliases and batch execution
- History and replay functionality
- Location: `src/command/`

### 6. Claude Integration
- API client for Claude interactions
- Context management for conversations
- Implementation automation
- Location: `src/claude/`

## Data Flow
1. User creates specifications in markdown format
2. Specification Engine parses and validates specs
3. Command Dispatcher routes implementation requests
4. Claude Integration generates implementation
5. State Manager tracks progress and history
6. Git commits capture system evolution

## Directory Structure
```
src/
├── main.rs           # Application entry point with CLI commands
├── lib.rs            # Library exports
├── project/          # Project management
│   ├── mod.rs        # Project types and module exports
│   ├── manager.rs    # Project lifecycle management
│   ├── template.rs   # Template system
│   └── health.rs     # Health check system
├── spec/             # Specification handling
│   ├── mod.rs        # Spec types and exports
│   ├── engine.rs     # Spec execution engine
│   ├── parser.rs     # Spec parser
│   └── template.rs   # Spec templates
├── state/            # State management
│   ├── mod.rs        # State types
│   ├── manager.rs    # State operations
│   └── migrations.rs # Database migrations
├── config/           # Configuration system
│   ├── mod.rs        # Config types
│   ├── loader.rs     # Config loading and hot-reload
│   └── validator.rs  # Config validation
├── command/          # Command processing
│   ├── mod.rs        # Command types
│   ├── dispatcher.rs # Command routing
│   ├── registry.rs   # Plugin registry
│   └── history.rs    # Command history
├── claude/           # Claude integration (planned)
├── plugin/           # Plugin system (planned)
├── workflow/         # Workflow automation (planned)
├── monitor/          # Monitoring and reporting (planned)
└── error.rs          # Error handling
```