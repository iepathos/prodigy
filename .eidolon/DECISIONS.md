# Architecture Decision Records

## ADR-008: Claude Integration Architecture
- **Date**: 2025-07-25
- **Status**: Accepted
- **Context**: Need sophisticated Claude integration for automated implementation
- **Decision**: Implement comprehensive Claude module with multiple subsystems
- **Consequences**:
  - Advanced prompt engineering with Tera templates
  - Efficient context management with priority queues
  - Robust error handling with retry logic
  - Token usage optimization and tracking
  - Response caching reduces API costs
  - Conversation memory improves context awareness
  - Modular design allows easy extension

## ADR-010: Workflow Automation Architecture
- **Date**: 2025-07-25
- **Status**: Accepted
- **Context**: Need powerful workflow automation for complex development processes
- **Decision**: Implement YAML-based workflow system with conditional logic and checkpoints
- **Consequences**:
  - YAML provides readable, version-controlled workflow definitions
  - Conditional execution enables intelligent workflow paths
  - Human-in-the-loop checkpoints ensure quality control
  - Event-driven triggers automate workflow initiation
  - Template inheritance reduces duplication
  - State persistence enables workflow recovery

## ADR-011: Pest for Condition Parsing
- **Date**: 2025-07-25  
- **Status**: Accepted
- **Context**: Need to parse conditional expressions in workflows
- **Decision**: Use Pest parser generator for condition syntax
- **Consequences**:
  - Clean grammar definition for expressions
  - Type-safe parsing with good error messages
  - Additional dependency but worth the robustness
  - Extensible for future expression features

## ADR-009: Tera for Prompt Templates
- **Date**: 2025-07-25
- **Status**: Accepted
- **Context**: Need flexible prompt template system for Claude
- **Decision**: Use Tera template engine for prompt generation
- **Consequences**:
  - Powerful template syntax with variables and logic
  - Reusable prompt templates
  - Easy to maintain and update prompts
  - Additional dependency but worth the flexibility

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
- **Status**: Accepted
- **Context**: Need extensible architecture for third-party integrations and custom functionality
- **Decision**: Implement comprehensive plugin system with multiple format support and sandboxed execution
- **Consequences**:
  - Extensible architecture supporting dynamic libraries, WebAssembly, and scripts
  - Robust security model with permission-based access control
  - Plugin marketplace for distribution and discovery
  - Comprehensive API providing access to mmm's core functionality
  - Additional complexity but enables unlimited extensibility
  - Strong foundation for third-party ecosystem development

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