# PROJECT.md - Memento Mori (mmm)

## Overview

Memento Mori (mmm) is a dead simple Rust CLI tool that makes your code better through Claude CLI integration. Just run `mmm cook` and it automatically analyzes your project, calls Claude CLI for improvements, and applies the changes.

## Current State

- **Project Status**: Active Development - Core Working
- **Core Feature**: `mmm cook` command with Claude CLI integration
- **Latest Version**: 0.1.0
- **Implementation Status**: Git-native improvement flow fully implemented with robust Claude CLI integration, complete audit trails, self-sufficient automated cycles, dynamic spec generation for improvements, focus-directed initial analysis (Spec 20), configurable workflows (Spec 21), configurable iteration limits (Spec 22), command-line config options (Spec 23), git worktree isolation for parallel sessions (Spec 24), Claude-assisted worktree merging with conflict resolution (Spec 25), worktree CLI flag (Spec 26), structured command objects (Spec 28), centralized worktree state management (Spec 29), interrupted worktree recovery (Spec 30), product management command (Spec 31), batch spec implementation (Spec 33), unified cook command with mapping (Spec 35), interactive worktree merge prompt (Spec 37), consistent focus directive on all iterations (Spec 38), auto-accept flag for non-interactive operation (Spec 41), MMM command initialization system (Spec 43), context-aware project understanding (Spec 44), real metrics tracking (Spec 46), cook path argument (Spec 47), command chaining with variables (Spec 48), session state management refactor (Spec 58), and isolated metrics collection system (Spec 60)

## What Exists

### Core Functionality
- **Simple CLI**: `mmm cook PLAYBOOK [-p PATH] [--show-progress] [--focus "area"] [--max-iterations N] [--worktree] [--map "pattern"] [--args "value"] [--yes] [--metrics]`
- **Command Initialization**: `mmm init` bootstraps .claude/commands for new projects
- **Unified Command**: Single `cook` command handles both iterative improvements and batch processing
- **Git-Native Flow**: Each improvement step creates git commits for complete auditability
- **Claude Integration**: Three-step Claude CLI workflow (review → implement → lint)
- **Project Analysis**: Automatic language and framework detection
- **Focus-Directed Analysis**: Optional focus directive for initial review (e.g., "user experience", "performance", "security")
- **Automated Linting**: Integrated formatting, linting, and testing with commits
- **Minimal State**: Simple JSON files tracking essential data (score, runs, session history)
- **Playbook-Driven Workflows**: Flexible workflow definition via YAML/JSON playbooks with command chaining and variables
- **Iteration Control**: Configurable maximum iterations with --max-iterations flag (default: 10)
- **Parallel Sessions**: Git worktree isolation enables multiple concurrent improvement sessions
- **Claude-Assisted Merge**: Automatic conflict resolution for worktree merges via Claude CLI
- **Worktree State Tracking**: Centralized metadata for worktree sessions with UUID-based naming
- **Interactive Merge Prompt**: Prompts to merge completed worktrees immediately in TTY environments
- **Auto-Accept Mode**: `-y/--yes` flag for fully unattended operation in scripts and CI/CD
- **Context-Aware Analysis**: Deep project understanding with dependency graphs, architecture detection, conventions, technical debt mapping, and test coverage analysis
- **Real Metrics Tracking**: `--metrics` flag enables comprehensive tracking of code quality, performance, complexity, and progress throughout iterations
- **Isolated Metrics System**: Pluggable metrics collection with file/memory/composite backends, comprehensive testing support, and zero overhead when disabled
- **Command Chaining**: Flexible command output/input chaining with variable substitution in playbooks
- **Interrupted Session Recovery**: Resume interrupted worktree sessions with `--resume <session-id>` from last checkpoint

### Project Structure
```
mmm/
├── src/
│   ├── main.rs           # CLI entry point with subcommands
│   ├── cook/             # Core cook command logic
│   ├── context/          # Context-aware project understanding
│   ├── session/          # Event-driven session state management
│   ├── simple_state/     # Minimal state management
│   ├── worktree/         # Git worktree management
│   └── lib.rs           # Library exports
├── .mmm/                # Project context and state
└── README.md            # User documentation

~/.mmm/worktrees/{repo-name}/  # Git worktrees stored in home directory
├── mmm-session-1234567890/
├── mmm-performance-1234567891/
└── mmm-security-1234567892/
```

## Key Capabilities

1. **Dead Simple Interface**
   - Main commands: `mmm cook`, `mmm worktree`, `mmm init`
   - Optional path argument to work on any repository
   - Optional flags for verbosity, focus directive, and more
   - Auto-accept flag for non-interactive automation
   - Quick onboarding with `mmm init` to install required commands
   - Works out of the box

2. **Git-Native Improvement Flow**
   - `/mmm-code-review`: Analyzes code and commits improvement specs
   - `/mmm-implement-spec`: Applies fixes and commits changes
   - `/mmm-lint`: Formats, lints, tests, and commits automated fixes
   - `/mmm-product-enhance`: Analyzes from product perspective for user value
   - Complete audit trail through git history

3. **Minimal State Management**
   - Simple `.mmm/state.json` for essential data only
   - Temporary specs in `specs/temp/` for each iteration
   - Project context files for Claude understanding
   - Git history contains complete change log

4. **Self-Sufficient Automated Loop**
   - Analyze → Review (commit spec) → Extract spec ID → Implement (commit) → Lint (commit) → Re-analyze → Repeat
   - Dynamic spec generation in `specs/temp/` directory for each iteration
   - Automatic termination when target reached or no issues found
   - Robust error handling and graceful failure recovery
   - Complete git audit trail with structured commit messages
   - Focus directive consistently applied across all iterations

5. **Configurable Workflows**
   - Optional `.mmm/workflow.toml` configuration file
   - Simple list of Claude commands to execute
   - Automatic spec ID extraction for mmm-implement-spec
   - Support for custom workflow sequences

6. **Parallel Execution**
   - Git worktree isolation for concurrent sessions
   - Each session runs in its own branch
   - Commands: `mmm worktree list/merge/clean`
   - Enable with `--worktree` flag (or `-w` short form)
   - Claude-assisted merge with automatic conflict resolution
   - Bulk merge support with `mmm worktree merge --all`
   - Legacy `MMM_USE_WORKTREE=true` supported with deprecation warning

7. **Context-Aware Analysis**
   - Dependency graph analysis for module relationships
   - Architecture pattern detection and violation checking
   - Convention learning and naming style detection
   - Technical debt mapping with prioritization
   - Test coverage analysis with gap identification
   - Context provided to Claude commands via environment variables

## Technology Stack

- **Language**: Rust (2021 edition)
- **CLI Framework**: Clap v4
- **Async Runtime**: Tokio
- **State**: JSON files
- **Serialization**: Serde (JSON)
- **Claude Integration**: Direct CLI subprocess calls

## Development Philosophy

- **Dead Simple**: Single command interface, minimal options
- **Git-Native**: Use git as the communication layer - simple, reliable, auditable
- **Actually Works**: Real Claude integration, real file changes, real git commits
- **Minimal State**: Track only what's needed, let git handle the audit trail
- **Self-Sufficient**: Fully automated improvement cycles with complete logging

## Next Steps

Focus on making the core `mmm cook` command robust and reliable:
- Better error handling
- More language support
- Improved Claude context building
- Enhanced progress feedback