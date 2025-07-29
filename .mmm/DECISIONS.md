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

---

## ADR-019: Git Worktree Isolation for Parallel Sessions

### Status
Accepted

### Context
Spec 24 identified the need for running multiple MMM improvement sessions concurrently without conflicts. When multiple sessions run in the same repository, they create commits that can clash, making parallel improvements difficult to manage.

### Decision
Implement git worktree isolation where each MMM session creates and operates in its own worktree with a unique branch. Sessions are opt-in via MMM_USE_WORKTREE environment variable. Add CLI subcommands for worktree management (list, merge, clean).

### Consequences
- **Positive**: True parallel execution, no conflicts between sessions, complete isolation, preserved debugging context on failure, backward compatible (opt-in)
- **Negative**: Requires git 2.5+, additional disk space for worktrees, slightly more complex workflow, manual merge step required
- **Implementation**: Created worktree module with WorktreeManager, integrated into improve command flow, added CLI subcommands, automatic cleanup on success with preservation on failure

---

## ADR-020: Worktree Location in Home Directory

### Status
Accepted

### Context
Initial implementation placed worktrees in `.mmm/worktrees/` inside the repository. However, Git has a restriction that worktrees cannot be subdirectories of the repository they belong to. This caused git worktree commands to fail with "fatal: 'path' is inside repository" errors.

### Decision
Move worktrees to `~/.mmm/worktrees/{repo-name}/` in the user's home directory. This ensures worktrees are outside any repository while maintaining organization by project name.

### Consequences
- **Positive**: Complies with git requirements, centralized worktree management, works across all repositories, organized by project name
- **Negative**: Worktrees persist in home directory after project deletion, requires home directory access, slightly less discoverable than project-local storage
- **Implementation**: Updated WorktreeManager to use dirs::home_dir(), added repository name extraction, updated tests to clean up home directory worktrees

---

## ADR-021: Claude-Assisted Merge for Conflict Resolution

### Status
Accepted

### Context
Spec 25 identified that standard git merge fails when conflicts occur during worktree merging, breaking the automated parallel workflow. Manual conflict resolution defeats the purpose of automation.

### Decision
Replace direct git merge with Claude CLI command `/mmm-merge-worktree` that intelligently resolves conflicts. Add --all flag for bulk merging multiple worktrees.

### Consequences
- **Positive**: Zero failed merges due to conflicts, truly parallel workflows, complete automation, intelligent conflict resolution, bulk merge capability
- **Negative**: Dependency on Claude CLI for merges, slightly slower merge process, requires trust in Claude's conflict resolution
- **Implementation**: Updated WorktreeManager::merge_session to call Claude CLI, added --all flag to merge command, automatic cleanup after successful merge

---

## ADR-022: Worktree CLI Flag for Better UX

### Status
Accepted

### Context
Spec 26 identified that using environment variable MMM_USE_WORKTREE=true for enabling worktree mode violates CLI best practices. The flag was not discoverable through --help, required extra typing, and made command history less clear.

### Decision
Replace environment variable with --worktree (-w) CLI flag while maintaining backward compatibility with deprecation warning. This follows standard CLI conventions and improves discoverability.

### Consequences
- **Positive**: Better discoverability through help text, shorter commands, consistent with other CLI flags, cleaner command history
- **Negative**: Minor breaking change requiring users to update scripts, additional CLI argument to maintain
- **Implementation**: Added --worktree flag to ImproveCommand, updated logic to check flag first then env var with warning, updated all documentation

---

## ADR-023: Structured Command Objects for Type Safety

### Status
Accepted

### Context
Spec 28 identified that MMM's workflow system used simple string-based command representations, which limited type safety, command validation, and extensibility. String parsing for special cases made it difficult to extend with new command features without breaking existing configurations.

### Decision
Transform the workflow command system from string-based to structured command objects using a WorkflowCommand enum that supports both legacy string format and new structured Command objects. Implement command registry with validation, defaults, and type-safe parameter handling.

### Consequences
- **Positive**: Type-safe command definitions, first-class support for command arguments and options, extensible command metadata (retries, timeouts, error handling), better integration with Claude CLI interface, maintained backward compatibility
- **Negative**: Additional complexity in configuration parsing, more code to maintain, learning curve for advanced configuration features
- **Implementation**: Created command.rs with Command/CommandMetadata/WorkflowCommand structures, command_parser.rs for string conversion, command_validator.rs with CommandRegistry, updated workflow executor to use structured commands, maintained full backward compatibility with string-based configs

---

## ADR-024: Centralized Worktree State Management

### Status
Accepted

### Context
Spec 29 identified issues with the existing worktree implementation: focus directives embedded in directory names caused problems with long prompts, no tracking of worktree-specific state (iterations, status), no persistence of metadata after cleanup, and timestamp-based naming could cause collisions with parallel sessions.

### Decision
Implement centralized worktree state management using UUID-based naming and a `.metadata/` directory in `~/.mmm/worktrees/{repo}/`. Store rich session state including iterations, status, focus, and statistics. Add `.metadata/` to `.gitignore` to prevent git pollution.

### Consequences
- **Positive**: Clean worktree names (just `session-{uuid}`), no collisions with UUIDs, rich state tracking for better UX, state persists after worktree cleanup, no git pollution, improved list command showing status and progress
- **Negative**: Additional disk space for metadata, more complex state management, requires migration for legacy worktrees
- **Implementation**: Created state.rs with WorktreeState types, updated WorktreeManager to use UUIDs and save/update state, integrated state tracking into improve loop, enhanced list command to show rich state information

---

## ADR-025: CLI Help as Default Behavior

### Status
Accepted

### Context
Spec 32 identified that running `mmm` without arguments automatically executed the improve command, violating Unix CLI conventions where commands without arguments typically display help information. This caused confusion for new users and accidental code improvements.

### Decision
Modify the CLI to display help information when invoked without arguments, aligning with standard CLI conventions. Use clap's built-in help functionality to maintain consistency with other help outputs.

### Consequences
- **Positive**: Better new user experience, follows Unix CLI conventions, prevents accidental command execution, clear guidance on available commands, consistent with user expectations
- **Negative**: Minor breaking change for users who relied on default improve behavior
- **Implementation**: Modified main.rs to call print_help() when no subcommand provided, maintained all existing command functionality, no changes to subcommand behavior

---

## ADR-026: Batch Specification Implementation

### Status
Accepted

### Context
Spec 33 identified the need for implementing multiple pre-written specifications without going through the code-review step. Developers who have already documented specifications in the specs/ directory need a streamlined way to implement them in batch, enabling planned features and improvements to be implemented efficiently.

### Decision
Create a new `mmm implement` subcommand that accepts specification file paths (with glob support) and implements them sequentially using the implement-spec → lint cycle. Each spec is processed independently with clear progress tracking and summary reporting.

### Consequences
- **Positive**: Efficient batch implementation of planned features, supports glob patterns for flexible spec selection, reuses existing Claude integration, maintains git audit trail, clear progress tracking, optional worktree support for parallel execution
- **Negative**: Additional subcommand complexity, potential for longer execution times with many specs
- **Implementation**: Created implement module with command.rs, state.rs, and mod.rs; integrated with existing git_ops and worktree managers; added progress tracking and summary reporting; supports dry-run mode for safety

### Decision
Modify the CLI to display help information when invoked without arguments, aligning with standard CLI conventions. Use clap's built-in help functionality to maintain consistency with other help outputs.

### Consequences
- **Positive**: Better new user experience, follows Unix CLI conventions, prevents accidental command execution, clear guidance on available commands, consistent with user expectations
- **Negative**: Minor breaking change for users who relied on default improve behavior
- **Implementation**: Modified main.rs to call print_help() when no subcommand provided, maintained all existing command functionality, no changes to subcommand behavior

---

## ADR-027: Unified Improve Command with Mapping

### Status
Accepted

### Context
Spec 35 identified that having separate `mmm improve` and `mmm implement` commands created unnecessary complexity and duplicated functionality. Both commands essentially run improvement loops with different inputs and workflows. The improve command already supported configurable workflows, making the implement command redundant.

### Decision
Unify functionality into a single `mmm improve` command by adding --map and --args flags for batch processing. Remove the implement subcommand entirely. Support variable substitution in workflow commands to enable flexible, parameterized workflows.

### Consequences
- **Positive**: Single coherent interface, reduced code duplication, more flexible batch processing, enables any workflow via configuration, consistent with Unix philosophy of composable tools
- **Negative**: Breaking change for users of mmm implement command, requires migration to new syntax
- **Implementation**: Added --map and --args flags to ImproveCommand, implemented CommandArg enum for variable support, updated workflow executor to resolve variables, removed implement module, created examples/implement.yml for migration

---

## ADR-028: Product Management Command for User-Focused Improvements

### Status
Accepted

### Context
Spec 31 identified a gap in the improvement workflow. While `/mmm-code-review` focuses on code quality and technical excellence, there was no command to analyze code from a product management perspective. Product managers prioritize user value, feature completeness, and solving real user problems over technical perfection.

### Decision
Create `/mmm-product-enhance` command that analyzes code from a product management perspective, generating improvement specs focused on user value, features, and user experience rather than code quality metrics. Register it in the command registry alongside existing commands.

### Consequences
- **Positive**: Complements code review with user-focused analysis, enables feature-driven development workflows, provides balanced perspective on improvements, supports product management workflows
- **Negative**: Additional command to maintain, potential overlap with code review in some areas
- **Implementation**: Created mmm-product-enhance.md command definition, added to CommandRegistry with appropriate options and defaults, created product-enhancement-workflow.yml example, updated documentation

---

## ADR-029: Rename Improve to Cook for Better Memorability

### Status
Accepted

### Context
Spec 36 identified that the current main command `mmm improve` could be more evocative and memorable. For a tool named "Memento Mori" (remember death), the command name "cook" suggests transformation, refinement, and the application of heat/pressure to create something better - all metaphors that align well with what the tool does to code. Additionally, "cook" is shorter to type and creates a more distinctive command-line experience.

### Decision
Rename the `improve` subcommand to `cook` throughout the codebase while maintaining backward compatibility through command aliases. The primary command changes from `mmm improve` to `mmm cook`, with `improve` remaining as a deprecated alias that shows a helpful migration notice.

### Consequences
- **Positive**: More memorable command name, shorter to type, distinctive CLI personality, metaphor aligns with tool purpose (cooking/refining code), backward compatibility maintained
- **Negative**: Breaking change for users who don't update, requires updating all documentation and examples, potential confusion during transition period
- **Implementation**: Renamed src/improve/ to src/cook/, updated all imports and type names (ImproveCommand → CookCommand), added alias support in CLI with deprecation notice, updated all documentation and tests

---

## ADR-030: Focus Directive on Every Iteration

### Status
Accepted

### Context
Spec 38 identified that the focus directive (specified via --focus flag) was only passed to /mmm-code-review on the first iteration of the improvement loop. This caused subsequent iterations to lose context of what aspect the user wanted to focus on, potentially causing the improvement process to drift away from the intended focus area after the first iteration.

### Decision
Pass the focus directive to every iteration of the improvement process, not just the first. This ensures consistent focus throughout the entire improvement session and prevents drift from the user's intended improvement goals.

### Consequences
- **Positive**: Consistent focus throughout improvement session, prevents drift from intended goals, simple implementation (remove conditional logic), maintains backward compatibility, better user experience with predictable behavior
- **Negative**: None identified - this is a straightforward improvement to existing behavior
- **Implementation**: Removed conditional logic checking iteration == 1 in four locations within src/cook/mod.rs, added explanatory comments, added test to verify consistent focus application

---

## ADR-031: Auto-Accept Flag for Non-Interactive Operation

### Status
Accepted

### Context
Spec 41 identified that when running `mmm cook` with the `--worktree` flag, the system prompts users interactively after completion to ask if they want to merge the worktree and then again to delete it. While this is great for interactive use, it creates friction in automated environments where users want to run the command unattended, such as in CI/CD pipelines or automated scripts.

### Decision
Add a `-y/--yes` flag to the cook command that automatically accepts all interactive prompts. When this flag is set, the system automatically answers "yes" to the worktree merge prompt and the worktree deletion prompt, enabling fully unattended operation. The flag follows standard Unix CLI conventions similar to tools like `apt-get -y`.

### Consequences
- **Positive**: Enables fully automated workflows, follows standard CLI conventions, supports CI/CD integration, maintains backward compatibility, clear logging of auto-accepted actions for audit trail, safe defaults (only auto-accepts on success)
- **Negative**: Users must be careful when using the flag as it will automatically merge and delete worktrees without confirmation
- **Implementation**: Added auto_accept boolean field to CookCommand, updated merge and deletion prompt logic to check flag before prompting, modified conditions to run prompts in either interactive terminal or when auto_accept is true, added clear logging when auto-accepting for transparency
