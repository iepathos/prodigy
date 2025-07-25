# Memento Mori Manager (mmm)

## Project Overview
A Git-based project management system that integrates with Claude for automated implementation of specifications. The system uses a "git good" methodology to track project evolution through specifications, implementations, and validation cycles.

## Current State
- Progress: 100% (Workflow automation implemented)
- Phase: Foundation Complete - All core modules including workflow automation

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
- Claude Integration with:
  - Advanced prompt engineering with Tera templates
  - Context window optimization with priority queue
  - Response parsing and validation
  - Retry logic with exponential backoff
  - Token usage tracking and optimization
  - Conversation memory management
  - Custom Claude commands (implement, review, debug, plan, explain)
  - Response caching for efficiency
- Workflow Automation System with:
  - YAML-based workflow definitions
  - Conditional execution based on workflow state
  - Sequential and parallel step execution
  - Human-in-the-loop checkpoints
  - Event-driven triggers
  - Workflow state persistence
  - Template inheritance for reusable workflows
  - Debugging and dry-run capabilities

## Key Capabilities
- [x] Multi-project management with templates
- [x] Project health monitoring
- [x] Specification-driven development
- [x] Claude integration for automated implementation
- [x] State management with SQLite
- [x] Workflow automation with YAML definitions
- [ ] Plugin system for extensibility

## Technology Stack
- Language: Rust
- Build System: Cargo
- Task Runner: Just
- Database: SQLite (for state management)
- Configuration: TOML