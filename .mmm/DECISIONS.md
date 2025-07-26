# DECISIONS.md - Key Architectural Decisions

## ADR-001: Use Rust for Implementation

### Status
Accepted

### Context
Need a language that provides performance, safety, and good CLI tooling for a simple improvement tool.

### Decision
Use Rust with minimal dependencies.

### Consequences
- **Positive**: Memory safety, performance, excellent CLI libraries, cross-platform
- **Negative**: Learning curve, longer compilation times

---

## ADR-002: Dead Simple CLI Interface

### Status
Accepted

### Context
Tool should be immediately usable without configuration or complex commands.

### Decision
Single command: `mmm improve [--target 8.0] [--verbose]`

### Consequences
- **Positive**: Zero learning curve, obvious usage, minimal documentation needed
- **Negative**: Limited flexibility, may need more options later

---

## ADR-003: JSON for State Persistence

### Status
Accepted

### Context
Need local state storage that's human-readable and requires zero configuration.

### Decision
Use JSON files in `.mmm/` directory for all state.

### Consequences
- **Positive**: Human-readable, git-friendly, zero configuration, easy debugging
- **Negative**: No concurrent write protection, larger files than binary

---

## ADR-004: Direct Claude CLI Integration

### Status
Accepted

### Context
Need to actually call Claude for improvements, not simulate or mock.

### Decision
Use subprocess calls to the actual `claude` CLI command.

### Consequences
- **Positive**: Real functionality, leverages existing tool, no API management
- **Negative**: Depends on Claude CLI installation, subprocess complexity

---

## ADR-005: Remove Learning System

### Status
Accepted

### Context
Learning system added complexity without providing real value to core functionality.

### Decision
Remove LearningManager, simplify Improvement struct, focus on essential tracking only.

### Consequences
- **Positive**: ~500 lines removed, simpler mental model, focus on working features
- **Negative**: Loss of potential future learning capabilities

---

## ADR-006: Minimal Error Handling Strategy

### Status
Accepted

### Context
Need consistent error handling without over-engineering.

### Decision
Use `anyhow::Result<T>` throughout with context, fail fast with clear messages.

### Consequences
- **Positive**: Simple error propagation, good error messages, minimal boilerplate
- **Negative**: Less granular error types, harder to recover from specific errors

---

## ADR-007: Project Analysis Before Improvement

### Status
Accepted

### Context
Need to understand project structure before calling Claude.

### Decision
Analyze language, framework, and basic health metrics before each improvement cycle.

### Consequences
- **Positive**: Better Claude context, smarter improvements, cached results
- **Negative**: Slight startup delay, analysis complexity

---

## ADR-008: Focus on Working Over Perfect

### Status
Accepted

### Context
Choose between polished features vs. functional core.

### Decision
Prioritize making `mmm improve` actually work over adding features.

### Consequences
- **Positive**: Real user value, working tool faster, clearer priorities
- **Negative**: May ship with rough edges, missing nice-to-have features

---

## ADR-009: No Complex Configuration

### Status
Accepted

### Context
Avoid configuration complexity that many tools suffer from.

### Decision
Minimal configuration, smart defaults, work out of the box.

### Consequences
- **Positive**: Zero setup time, no configuration drift, simpler testing
- **Negative**: Less customization, may not fit all use cases perfectly

---

## ADR-010: Single Module Focus

### Status
Accepted

### Context
Avoid premature abstraction and keep codebase simple.

### Decision
Straightforward module organization, minimal abstractions, clear data flow.

### Consequences
- **Positive**: Easy to understand, modify, and debug
- **Negative**: May need refactoring if complexity grows

---

## ADR-011: Real Claude CLI Integration via Subprocess

### Status
Accepted

### Context
Spec 14 required replacing simulation code with actual Claude CLI integration for a working self-sufficient improvement loop.

### Decision
Use subprocess calls to execute `claude /mmm-code-review` and `claude /mmm-implement-spec` commands with structured JSON output parsing.

### Consequences
- **Positive**: Real functionality, leverages existing Claude CLI commands, structured output for automation
- **Negative**: Dependency on Claude CLI installation, subprocess complexity, JSON parsing requirements
- **Implementation**: Added real command execution, JSON output parsing, file change application, and project re-analysis