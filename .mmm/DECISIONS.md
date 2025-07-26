# DECISIONS.md - Architectural Decision Records

## ADR-001: Use Rust for Implementation

### Status
Accepted

### Context
Need a language that provides performance, safety, and good CLI tooling.

### Decision
Use Rust with Tokio async runtime.

### Consequences
- **Positive**: Memory safety, performance, excellent CLI libraries
- **Negative**: Steeper learning curve, longer compilation times

---

## ADR-002: SQLite for State Persistence

### Status
Accepted

### Context
Need local state storage that's portable and doesn't require a server.

### Decision
Use SQLite with SQLx for async operations.

### Consequences
- **Positive**: Zero configuration, portable, robust
- **Negative**: Limited concurrent writes, no distributed support

---

## ADR-003: YAML for Workflow Definitions

### Status
Accepted

### Context
Need human-readable format for defining workflows.

### Decision
Use YAML with custom schema for workflow definitions.

### Consequences
- **Positive**: Readable, widely understood, good tooling
- **Negative**: Can be error-prone with indentation

---

## ADR-004: Pest for Condition Parsing

### Status
Accepted

### Context
Need to parse and evaluate conditional expressions in workflows.

### Decision
Use Pest parser generator with custom grammar.

### Consequences
- **Positive**: Powerful, maintainable grammar definitions
- **Negative**: Additional dependency, learning curve

---

## ADR-005: Plugin System Architecture

### Status
Proposed

### Context
Need extensibility without compromising core stability.

### Decision
Dynamic loading with WASI sandbox for security.

### Consequences
- **Positive**: Safe plugin execution, clear boundaries
- **Negative**: Performance overhead, complexity

---

## ADR-006: Claude API Integration Strategy

### Status
Accepted

### Context
Need reliable integration with Claude API including caching and retry.

### Decision
Custom client with exponential backoff and response caching.

### Consequences
- **Positive**: Resilient to failures, cost-effective
- **Negative**: Cache invalidation complexity

---

## ADR-007: Monitoring Approach

### Status
Accepted

### Context
Need visibility into system behavior and performance.

### Decision
Built-in metrics collection with Axum dashboard.

### Consequences
- **Positive**: No external dependencies, integrated experience
- **Negative**: Limited compared to dedicated monitoring tools

---

## ADR-008: Error Handling Strategy

### Status
Accepted

### Context
Need consistent error handling across the application.

### Decision
Use thiserror for error types and anyhow for error context.

### Consequences
- **Positive**: Type-safe errors, good error messages
- **Negative**: Some boilerplate for error types

---

## ADR-009: Async Runtime Choice

### Status
Accepted

### Context
Need async runtime for concurrent operations.

### Decision
Use Tokio with full features.

### Consequences
- **Positive**: Mature, full-featured, great ecosystem
- **Negative**: Large dependency, some overhead

---

## ADR-010: Configuration Management

### Status
Accepted

### Context
Need flexible configuration system.

### Decision
TOML for config files with environment variable overrides.

### Consequences
- **Positive**: Readable configs, standard format
- **Negative**: Multiple config sources can be confusing

---

## ADR-011: Project Structure Implementation

### Status
Accepted

### Context
Implemented spec 01 for core architecture with global/project separation.

### Decision
- Global config in ~/.mmm/ for cross-project settings
- Project-specific config in .mmm/ for overrides
- SQLite database per project for state management
- Project registry in global directory

### Consequences
- **Positive**: Clear separation of concerns, multi-project support
- **Negative**: Additional complexity in config resolution

---

## ADR-012: Iterative Improvement Loop Architecture

### Status
Accepted

### Context
Implemented spec 08 for iterative improvement loops that can automatically chain Claude CLI review and improvement commands while working around Claude CLI session limitations.

### Decision
- Integrate loop functionality directly into MMM's existing workflow system
- Use structured JSON output from Claude commands for automation
- Store loop sessions and iterations in SQLite database
- Implement configurable termination conditions for intelligent stopping
- Add safety mechanisms including git stashing and validation

### Consequences
- **Positive**: Self-sufficient automated improvement cycles, leverages existing MMM infrastructure
- **Negative**: Increased complexity in command orchestration, dependency on Claude CLI availability