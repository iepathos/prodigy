# Memento Mori Manager (mmm)

## Project Overview
A Git-based project management system that integrates with Claude for automated implementation of specifications. The system uses a "git good" methodology to track project evolution through specifications, implementations, and validation cycles.

## Current State
- Progress: 100% (Plugin system implemented)
- Phase: Extensibility Complete - All core modules including plugin system

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
- Monitoring and Reporting System with:
  - Real-time metrics collection and storage
  - Advanced analytics with bottleneck detection
  - Cost analysis and optimization recommendations
  - Comprehensive alerting system
  - Performance tracking and tracing
  - Report generation with multiple export formats
  - Web-based dashboard with API endpoints
  - CLI commands for monitoring operations
- Plugin System with:
  - Plugin discovery and loading from multiple formats (dynamic libraries, WebAssembly, scripts)
  - Sandboxed execution environment with resource limits
  - Comprehensive plugin API with project, state, and Claude integration
  - Plugin marketplace for discovery, installation, and management
  - Security model with permission-based access control
  - Support for multiple plugin types (commands, hooks, integrations)
  - Hot-reload capability for development
  - Plugin development kit with templates and CLI commands

## Key Capabilities
- [x] Multi-project management with templates
- [x] Project health monitoring
- [x] Specification-driven development
- [x] Claude integration for automated implementation
- [x] State management with SQLite
- [x] Workflow automation with YAML definitions
- [x] Comprehensive monitoring and reporting system
- [x] Plugin system for extensibility

## Technology Stack
- Language: Rust
- Build System: Cargo
- Task Runner: Just
- Database: SQLite (for state management)
- Configuration: TOML