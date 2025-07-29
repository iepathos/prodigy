# ROADMAP.md - Focused Development Plan

## Overview

This roadmap focuses on making `mmm improve` a robust, reliable tool that actually works. No more scaffolding - just working functionality.

## ✅ Phase 1: Core Foundation (COMPLETED)

### What Works Now
- [x] Dead simple CLI: `mmm improve [--target 8.0] [--verbose]`
- [x] Project analysis (language, framework, health detection)
- [x] JSON-based state management (simple and readable)
- [x] Basic Claude CLI integration structure
- [x] Minimal codebase (learning system removed)

## ✅ Phase 2: Working Claude Integration (COMPLETED)

### Priority: Make It Actually Work
- [x] **Real Claude CLI subprocess calls**
  - Execute actual `claude` commands with /mmm-code-review and /mmm-implement-spec
  - Handle stdout/stderr properly
  - Error handling for Claude CLI failures

- [x] **File Modification Pipeline**
  - Parse Claude responses for file changes
  - Apply changes safely with backups
  - Validate changes before applying

- [x] **Working Improvement Loop**
  - Analyze → Call Claude Review → Call Claude Implement → Apply Changes → Re-analyze → Repeat
  - Automatic termination when target reached or no issues found
  - Progress tracking and feedback with verbose mode

### Success Criteria
- User runs `mmm improve`
- Tool actually modifies files
- Code quality demonstrably improves
- Process is hands-off and reliable

## ✅ Phase 2.5: Remove Developer Experience Bloat (COMPLETED)

### Simplification Complete
- [x] **Removed src/developer_experience/ module entirely** (~1000+ lines removed)
- [x] **Simplified ImproveCommand to 2 fields** (target, verbose only)
- [x] **Replaced fancy display with basic println!** (removed indicatif, colored dependencies)
- [x] **Simplified session types** (removed ImprovementType enum, complex Improvement struct)
- [x] **Updated lib.rs** (removed developer_experience module reference)
- [x] **Removed UI dependencies** (indicatif, colored, ctrlc, crossterm)

## ✅ Phase 2.75: Simplify State Management (COMPLETED)

### Simplification Complete
- [x] **Simplified State structure** (removed SessionInfo, Statistics, complex metrics)
- [x] **Simplified SessionRecord** (removed improvements tracking, file changes, session metrics)
- [x] **Simplified state persistence** (basic session recording, no daily summaries)
- [x] **Removed cache statistics** (simplified CacheManager without stats tracking)
- [x] **Updated StateAdapter** (simplified to work with essential data only)

## ✅ Phase 2.8: Consolidate Core Modules (COMPLETED)

### Simplification Complete
- [x] **Consolidated improve/ module** (removed analyzer.rs, context.rs, display.rs, state_adapter.rs, command_enhanced.rs)
- [x] **Simplified improve/mod.rs** (single core loop implementation with Claude CLI integration)
- [x] **Minimized improve/command.rs** (CLI args only)
- [x] **Simplified improve/session.rs** (basic session data structures only)
- [x] **Cleaner architecture** (3 files max in improve/ module as per spec requirements)

## ✅ Phase 2.9: Git-Native Improvement Flow (COMPLETED)

### Git-Native Architecture Complete
- [x] **Updated /mmm-code-review** to commit specs instead of JSON output
- [x] **Created /mmm-lint** command for automated formatting, linting, and testing
- [x] **Updated /mmm-implement-spec** to commit changes with descriptive messages
- [x] **Implemented git log parsing** in improvement loop to extract spec IDs
- [x] **Removed JSON parsing** from improve flow - now uses git commits for communication
- [x] **Added automation mode** support with MMM_AUTOMATION environment variable
- [x] **Complete audit trail** through git history for all changes
- [x] **Dynamic spec generation** with specs/temp/ directory for temporary improvement specs

## ✅ Phase 2.10: Focus-Directed Improvements (COMPLETED)

### Focus Directive Support Complete
- [x] **Added --focus flag** to CLI for directing initial analysis
- [x] **Updated ImproveCommand** to include optional focus field
- [x] **Modified improvement loop** to pass focus directive on first iteration only
- [x] **Environment variable passing** via MMM_FOCUS to /mmm-code-review
- [x] **Focus directive display** in progress output for first iteration
- [x] **Natural language interpretation** by Claude for flexible focus areas

## ✅ Phase 2.11: Configurable Workflows (COMPLETED)

### Workflow Configuration Support Complete
- [x] **Added workflow configuration** module in src/config/workflow.rs
- [x] **Created workflow executor** in src/improve/workflow.rs
- [x] **Simplified configuration** to just a list of Claude commands
- [x] **Updated config loader** to read .mmm/workflow.toml configuration
- [x] **Modified improve::run** to support both configurable and legacy workflows
- [x] **Automatic spec ID extraction** for mmm-implement-spec command
- [x] **Added documentation** with example workflows (security, testing, documentation)
- [x] **Backward compatibility** maintained - no config = default workflow

## ✅ Phase 2.12: Git Worktree Isolation (COMPLETED)

### Parallel Execution Support Complete
- [x] **Created worktree module** in src/worktree/ with manager.rs
- [x] **Implemented WorktreeManager** with full lifecycle management
- [x] **Added worktree subcommands** to CLI (list, merge, clean)
- [x] **Integrated with improve command** via MMM_USE_WORKTREE environment variable
- [x] **Automatic branch naming** with focus-based naming when applicable
- [x] **Worktree cleanup logic** with preservation on failure for debugging
- [x] **Unit tests** for worktree functionality
- [x] **Updated .gitignore** to exclude .mmm/worktrees/

## ✅ Phase 2.13: Worktree CLI Flag (COMPLETED)

### CLI Usability Enhancement Complete
- [x] **Added --worktree flag** to improve command for discoverable parallel execution
- [x] **Short form -w** supported for convenience
- [x] **Deprecation warning** for legacy MMM_USE_WORKTREE environment variable
- [x] **Updated all documentation** to use new flag syntax
- [x] **Backward compatibility** maintained for smooth migration

## 📋 Phase 3: Robustness (NEXT)

### Core Reliability
- [ ] **Better Error Handling**
  - Graceful Claude CLI failures
  - Safe file operation rollbacks
  - Clear error messages to user

- [ ] **Enhanced Context Building**
  - Better project analysis
  - Smarter file selection for Claude
  - More accurate health scoring

- [ ] **Improved UX**
  - Basic progress feedback
  - Better success/failure feedback
  - Clearer improvement summaries

### Testing Infrastructure
- [ ] **End-to-End Workflow Testing** (Spec 39)
  - Mock Claude CLI responses for deterministic testing
  - Test multiple workflows (legacy, implement, documentation, product)
  - Verify git operations and state management
  - Enable comprehensive CI/CD testing without external dependencies

- [ ] **Improve Test Coverage** (Spec 40)
  - Add abstraction layers for external commands (git, Claude CLI)
  - Create comprehensive unit tests without MMM_TEST_MODE
  - Test error scenarios and edge cases
  - Achieve at least 70% overall test coverage

### Configuration Enhancements
- [x] **Configurable Iteration Limit** (Spec 22) ✅
  - Add --max-iterations flag for custom iteration limits
  - Default remains 10 for backward compatibility
  
- [x] **Command Line Config Option** (Spec 23) ✅
  - Added --config flag to specify custom config file paths
  - Support for both TOML and YAML configuration formats
  - Precedence: --config flag > .mmm/config.toml > defaults
  - Backward compatibility with deprecated .mmm/workflow.toml

- [x] **Structured Command Objects** (Spec 28) ✅
  - Refactor workflow commands from strings to structured objects
  - Type-safe command definitions with validated parameters
  - Support for command-specific options and metadata
  - Maintain backward compatibility with string-based configs

- [x] **Unified Improve Command with Mapping** (Spec 35) ✅
  - Added --map flag to improve command for file pattern processing
  - Removed separate implement subcommand
  - Support variable substitution in command arguments ($ARG, $FILE, etc.)
  - Enable batch processing with custom workflows
  - Created examples/implement.yml for migration path

- [x] **Rename Improve Subcommand to Cook** (Spec 36) ✅
  - Rename main command from `improve` to `cook` for better memorability
  - Maintain `improve` as alias for backward compatibility
  - Update all documentation and internal references
  - Provides more distinctive CLI experience aligned with tool personality

- [x] **Interactive Worktree Merge Prompt** (Spec 37) ✅
  - Prompt user to merge completed worktrees immediately
  - Execute merge automatically if user agrees
  - Detect TTY for interactive vs non-interactive environments
  - Track prompt interactions in worktree state

- [x] **Focus Directive on Every Iteration** (Spec 38) ✅
  - Pass focus directive to all iterations, not just the first
  - Ensures consistent focus throughout improvement session
  - Simple change to remove conditional logic
  - Maintains backward compatibility

### Parallel Execution
- [x] **Git Worktree Isolation** (Spec 24) ✅
  - Isolate each MMM session in its own git worktree
  - Enable parallel improvement sessions without conflicts
  - Commands for listing and merging worktrees
  - MMM_USE_WORKTREE=true enables worktree mode

- [x] **Claude-Assisted Worktree Merge** (Spec 25) ✅
  - Automatic conflict resolution via Claude CLI
  - Replaces git merge with /mmm-merge-worktree command
  - Bulk merge support with --all flag
  - Complete audit trail of merge decisions

- [x] **Centralized Worktree State Management** (Spec 29) ✅
  - UUID-based worktree naming without embedded focus
  - State metadata stored in ~/.mmm/worktrees/{repo}/.metadata/
  - Rich state tracking (iterations, status, stats)
  - Better UX with status display in list command

- [ ] **Interrupted Worktree Recovery** (Spec 30)
  - Detect and track interrupted Claude sessions
  - Resume capability from last successful iteration
  - Signal handling for graceful state updates
  - Checkpoint mechanism for recovery

- [x] **Product Management Command** (Spec 31) ✅
  - Create /mmm-product-enhance command
  - Focus on feature improvements over code quality
  - User experience and value-driven enhancements
  - Integration with existing workflow system

- [x] **Batch Spec Implementation** (Spec 33) ✅
  - New `mmm implement` subcommand
  - Accept multiple spec files as arguments
  - Support glob patterns for spec selection
  - Simplified implement-spec → lint loop

### Success Criteria
- Works reliably across different project types
- Fails gracefully with helpful messages
- Users understand what's happening

## 🚀 Phase 4: Polish (FUTURE)

### Extended Language Support
- [ ] Python projects
- [ ] Go projects
- [ ] TypeScript/JavaScript enhancements
- [ ] Generic language fallbacks

### Enhanced Features
- [ ] Custom focus areas (`--focus tests`)
- [ ] Dry run mode (`--dry-run`)
- [ ] Better progress visualization
- [ ] Integration with common tools

### Automation Support
- [x] **Auto-Accept Flag** (Spec 41) ✅
  - Add -y/--yes flag for non-interactive operation
  - Auto-accept worktree merge and deletion prompts
  - Enable fully automated improvement workflows
  - Support CI/CD and script usage

### CLI Best Practices
- [x] **CLI Help as Default** (Spec 32) ✅
  - Show help when running `mmm` without arguments
  - Follow Unix CLI conventions
  - Improve new user experience
  - Clear guidance on available commands

## Non-Goals

Things we're **NOT** building:
- ❌ Complex workflow engines
- ❌ Plugin systems
- ❌ Monitoring dashboards
- ❌ Multi-project management
- ❌ Web interfaces
- ❌ Learning/AI systems
- ❌ Complex configuration

## Success Metrics

### Phase 2 (Working)
- `mmm improve` actually modifies files
- Users see code quality improvements
- Process completes without manual intervention

### Phase 3 (Robust)
- <5% failure rate on supported projects
- Users understand what went wrong when it fails
- Works on projects beyond our test cases

### Phase 4 (Polished)
- Supports 3+ programming languages well
- Sub-10 second startup time
- Clear, helpful progress feedback

## Development Principles

1. **Working > Perfect**: Make it work first, polish later
2. **Simple > Complex**: Choose simple solutions over clever ones
3. **Real > Simulated**: Actual Claude integration over mocking
4. **Users > Features**: Focus on user value over feature count
5. **Reliable > Fast**: Correctness over performance optimization

## Release Strategy

- **v0.1.0**: Basic structure and state management
- **v0.2.0**: Git-native improvement flow with Claude CLI integration and focus directives
- **v0.3.0**: Configurable workflows and extensibility ✅ CURRENT
- **v0.4.0**: Robust - Reliable error handling and UX
- **v1.0.0**: Production - Polished, multi-language support