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

---

## ADR-012: Remove Developer Experience Bloat

### Status
Accepted

### Context
Spec 15 identified extensive premature developer experience features that added complexity without providing core value before the basic improvement loop was proven to work reliably.

### Decision
Remove entire src/developer_experience/ module, simplify CLI to essential flags only, replace fancy progress displays with basic console output.

### Consequences
- **Positive**: ~1000+ lines removed, simpler codebase, faster compilation, focus on core functionality, reduced dependencies
- **Negative**: Less polished user experience, basic progress feedback only
- **Implementation**: Deleted developer_experience module, simplified ImproveCommand to 2 fields, replaced indicatif/colored with println!, removed unused dependencies

---

## ADR-013: Simplify State Management to Essentials

### Status
Accepted

### Context
Spec 16 identified that the state management system was over-engineered with complex session tracking, detailed metrics, file-level change tracking, and statistics that added complexity without providing essential value for the core improvement loop.

### Decision
Simplify state management to track only essential data: current score, total runs, and basic session history with summaries.

### Consequences
- **Positive**: ~300 lines removed, smaller JSON files, faster startup, easier debugging, clearer mental model
- **Negative**: Loss of detailed metrics and analytics capabilities
- **Implementation**: Simplified State struct, removed SessionInfo/Statistics/SessionMetrics, simplified SessionRecord to essential fields, removed cache statistics, updated StateAdapter and tests

---

## ADR-014: Consolidate Core Modules for Clarity

### Status
Accepted

### Context
Spec 17 identified that the improve functionality was scattered across too many files (analyzer.rs, context.rs, display.rs, state_adapter.rs, command_enhanced.rs) which created confusion and made the core loop hard to follow.

### Decision
Consolidate into 3 files maximum: mod.rs (core loop), command.rs (CLI args only), session.rs (basic data structures only). Remove redundant files and integrate Claude CLI calls directly into the main loop.

### Consequences
- **Positive**: Single clear core loop in mod.rs, easier to understand flow, significant code reduction, direct Claude CLI integration without abstractions
- **Negative**: Less modularity, larger single file for core logic
- **Implementation**: Deleted 5 redundant files, created consolidated mod.rs with direct Claude CLI subprocess calls, simplified command.rs and session.rs to essentials only

---

## ADR-015: Git-Native Dynamic Spec Generation

### Status
Accepted

### Context
Spec 18 required implementing dynamic spec generation for improvements using git commits as the communication mechanism between Claude CLI commands. The previous approach was already mostly implemented but needed the specs/temp/ directory structure.

### Decision
Complete the git-native architecture with dynamic spec generation in specs/temp/ directory, where /mmm-code-review generates temporary specs and commits them, then mmm improve extracts spec IDs from git history.

### Consequences
- **Positive**: Complete git-native workflow, robust audit trail, debuggable intermediate specs, no JSON parsing complexity
- **Negative**: Additional directory structure, temporary files need cleanup
- **Implementation**: Created specs/temp/ directory structure, confirmed git log parsing for spec extraction, complete three-step commit sequence (review → implement → lint)

---

## ADR-016: Focus-Directed Initial Analysis

### Status
Accepted

### Context
Spec 20 identified the need for users to guide the initial code analysis phase towards specific areas of concern like "user experience", "performance", or "security". Without this, Claude prioritizes issues automatically based on severity alone.

### Decision
Add optional --focus flag to CLI that passes a focus directive via MMM_FOCUS environment variable to the first iteration of /mmm-code-review only. Claude naturally interprets the focus area and adjusts issue prioritization accordingly.

### Consequences
- **Positive**: User control over initial priorities, natural language interpretation, simple implementation, no validation needed
- **Negative**: Only affects first iteration, relies on Claude's interpretation abilities
- **Implementation**: Added --focus flag to CLI, passed through ImproveCommand, environment variable MMM_FOCUS set on first iteration only, /mmm-code-review already had comprehensive focus support

---

## ADR-017: Configurable Workflows

### Status
Accepted

### Context
Spec 21 identified the need for customizable improvement workflows to support different use cases like security-focused improvements, test-driven development, or documentation generation. The existing hardcoded workflow limited flexibility.

### Decision
Implement configurable workflows via optional .mmm/workflow.toml file that allows users to define a simple list of Claude commands to execute in sequence. Automatic spec ID extraction for mmm-implement-spec command.

### Consequences
- **Positive**: Users can customize workflows for specific needs, very simple configuration format, backward compatible, minimal complexity
- **Negative**: Less flexibility than complex configuration, limited to sequential command execution
- **Implementation**: Created simple workflow.rs configuration, workflow executor that runs commands in sequence, automatic spec ID extraction from git for mmm-implement-spec

---

## ADR-018: Command Line Configuration Option

### Status
Accepted

### Context
Spec 23 identified the need for users to specify custom configuration file paths via command line. This supports shared configurations across projects, CI/CD environments, and testing different configurations without modifying project files.

### Decision
Add --config (-c) command-line option to specify custom configuration paths. Support both TOML and YAML formats. Follow precedence: explicit path > .mmm/config.toml > defaults. Rename workflow.toml to config.toml for consistency.

### Consequences
- **Positive**: Flexible configuration management, support for shared configs, CI/CD compatibility, both TOML and YAML formats supported, clear precedence rules
- **Negative**: Minor breaking change (workflow.toml → config.toml), additional complexity in config loading
- **Implementation**: Added --config flag to ImproveCommand, updated ConfigLoader with load_with_explicit_path method, support for YAML parsing, backward compatibility warnings for workflow.toml