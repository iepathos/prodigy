# Architecture Decision Records

## ADR-006: Project Health Check System
- **Date**: 2025-07-25
- **Status**: Accepted
- **Context**: Need to validate project state before operations
- **Decision**: Implement comprehensive health check system
- **Consequences**:
  - Early detection of configuration issues
  - Better user experience with clear error messages
  - Prevents operations on invalid projects
  - Additional validation overhead

## ADR-007: Built-in Project Templates
- **Date**: 2025-07-25
- **Status**: Accepted
- **Context**: Users need quick start options for common project types
- **Decision**: Include built-in templates for web-app, cli-tool, library, api-service
- **Consequences**:
  - Faster project initialization
  - Consistent project structures
  - Template maintenance required
  - Templates stored as YAML for flexibility

## ADR-001: Use SQLite for State Management
- **Date**: 2025-07-25
- **Status**: Accepted
- **Context**: Need persistent state management with query capabilities
- **Decision**: Use SQLite as embedded database
- **Consequences**: 
  - Simple deployment (no external dependencies)
  - ACID compliance for data integrity
  - SQL query capabilities
  - Single-writer limitation (acceptable for our use case)

## ADR-002: TOML for Configuration
- **Date**: 2025-07-25
- **Status**: Accepted
- **Context**: Need human-readable configuration format
- **Decision**: Use TOML for all configuration files
- **Consequences**:
  - Easy to read and write
  - Good Rust ecosystem support
  - Hierarchical structure support
  - Limited to static configuration (no scripting)

## ADR-003: Plugin Architecture
- **Date**: 2025-07-25
- **Status**: Proposed
- **Context**: Need extensibility without modifying core
- **Decision**: Implement plugin system using dynamic loading
- **Consequences**:
  - Extensible architecture
  - Third-party plugin support
  - Additional complexity
  - Security considerations for plugins

## ADR-005: Runtime SQL Queries vs Compile-time Macros
- **Date**: 2025-07-25
- **Status**: Accepted
- **Context**: SQLx requires DATABASE_URL at compile time for query macros
- **Decision**: Use runtime SQL queries instead of compile-time macros
- **Consequences**:
  - No compile-time SQL validation
  - More flexible deployment (no DATABASE_URL needed at build)
  - Manual type conversions required
  - Slightly more verbose query code

## ADR-004: Specification-Driven Development
- **Date**: 2025-07-25
- **Status**: Accepted
- **Context**: Need clear requirements and implementation tracking
- **Decision**: Use markdown specifications with frontmatter
- **Consequences**:
  - Clear requirements documentation
  - Version-controlled specifications
  - Easy Claude integration
  - Manual spec writing required