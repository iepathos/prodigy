# Memento Mori Manager (mmm)

## Project Overview
A Git-based project management system that integrates with Claude for automated implementation of specifications. The system uses a "git good" methodology to track project evolution through specifications, implementations, and validation cycles.

## Current State
- Progress: 80% (Core architecture implemented)
- Phase: Foundation - Core modules complete

## What Exists
- Complete Rust project structure with all core modules
- Project Manager with multi-project support
- Specification Engine with dependency management
- State Manager with SQLite backend
- Configuration System with hot-reload capability
- Command Dispatcher with plugin architecture
- Database schema and migrations
- CLI interface with basic commands

## Key Capabilities
- [x] Multi-project management
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