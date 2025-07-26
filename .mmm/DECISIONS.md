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
Superseded by ADR-013

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

---

## ADR-013: Dead Simple Improve Command

### Status
Accepted

### Context
Implemented spec 09 for zero-configuration code improvement that "just works" out of the box.

### Decision
- Create single `mmm improve` command with smart defaults
- Auto-detect project language, framework, and characteristics
- Build context automatically based on project analysis
- Use simple JSON state file instead of complex database
- Focus on immediate value with minimal setup

### Consequences
- **Positive**: Extremely user-friendly, works immediately after installation, clear progress feedback
- **Negative**: Less configurable than full workflow system, simulated Claude integration for now

---

## ADR-014: Smart Project Analyzer Architecture

### Status
Accepted

### Context
Implemented spec 10 for intelligent project analysis that automatically detects language, framework, and quality indicators.

### Decision
- Create comprehensive analyzer module with specialized sub-analyzers
- Language detection via build files, extensions, and content
- Framework detection through dependencies and config files
- Quality analysis using static code metrics
- Health scoring based on multiple factors
- Smart focus area detection for targeted improvements

### Consequences
- **Positive**: Zero-configuration analysis, accurate detection, actionable insights
- **Negative**: Additional complexity, some heuristics may need tuning

---

## ADR-015: JSON-Based State Management

### Status
Accepted

### Context
Implemented spec 11 to replace complex SQLite database with simple JSON files for state management.

### Decision
- Replace SQLite with human-readable JSON files
- Implement atomic writes with corruption recovery
- Add TTL-based caching for temporary data
- Create learning system for tracking improvement patterns
- Provide migration path from SQLite to JSON

### Consequences
- **Positive**: Zero configuration, human-readable, git-friendly, easy recovery
- **Negative**: No concurrent write protection, larger file sizes for big datasets