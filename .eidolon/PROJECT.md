# Memento Mori Manager (mmm)

## Project Overview
A Git-based project management system that integrates with Claude for automated implementation of specifications. The system uses a "git good" methodology to track project evolution through specifications, implementations, and validation cycles.

## Current State
- Progress: 85% (Project management implemented)
- Phase: Foundation - Core modules and project management complete

## What Exists
- Complete Rust project structure with all core modules
- Project Manager with multi-project support
  - Project lifecycle commands (new, init, list, info, switch, clone, archive, delete)
  - Project templates system (web-app, cli-tool, library, api-service)
  - Project registry with metadata tracking
  - Project health check system
  - Multi-project operations support
- Specification Engine with dependency management
- State Manager with SQLite backend
- Configuration System with hot-reload capability
  - Project-specific configuration management
  - Configuration get/set commands
- Command Dispatcher with plugin architecture
- Database schema and migrations
- CLI interface with comprehensive project management commands

## Key Capabilities
- [x] Multi-project management with templates
- [x] Project health monitoring
- [x] Specification-driven development
- [ ] Claude integration for automated implementation
- [x] State management with SQLite
- [ ] Workflow automation
- [ ] Plugin system for extensibility

## Technology Stack
- Language: Rust
- Build System: Cargo
- Task Runner: Just
- Database: SQLite (for state management)
- Configuration: TOML