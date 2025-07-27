# PROJECT.md - Memento Mori (mmm)

## Overview

Memento Mori (mmm) is a dead simple Rust CLI tool that makes your code better through Claude CLI integration. Just run `mmm improve` and it automatically analyzes your project, calls Claude CLI for improvements, and applies the changes.

## Current State

- **Project Status**: Active Development - Core Working
- **Core Feature**: `mmm improve` command with Claude CLI integration
- **Latest Version**: 0.1.0
- **Implementation Status**: Git-native improvement flow fully implemented with robust Claude CLI integration, complete audit trails, self-sufficient automated cycles, dynamic spec generation for improvements, focus-directed initial analysis (Spec 20), configurable workflows (Spec 21), configurable iteration limits (Spec 22), command-line config options (Spec 23), git worktree isolation for parallel sessions (Spec 24), and Spec 19 (Git-Native Improvement Flow) completed

## What Exists

### Core Functionality
- **Simple CLI**: `mmm improve [--target 8.0] [--verbose] [--focus "area"] [--max-iterations N]`
- **Git-Native Flow**: Each improvement step creates git commits for complete auditability
- **Claude Integration**: Three-step Claude CLI workflow (review → implement → lint)
- **Project Analysis**: Automatic language and framework detection
- **Focus-Directed Analysis**: Optional focus directive for initial review (e.g., "user experience", "performance", "security")
- **Automated Linting**: Integrated formatting, linting, and testing with commits
- **Minimal State**: Simple JSON files tracking essential data (score, runs, session history)
- **Configurable Workflows**: Optional `.mmm/workflow.toml` for custom improvement workflows
- **Iteration Control**: Configurable maximum iterations with --max-iterations flag (default: 10)
- **Parallel Sessions**: Git worktree isolation enables multiple concurrent improvement sessions

### Project Structure
```
mmm/
├── src/
│   ├── main.rs           # CLI entry point with subcommands
│   ├── improve/          # Core improve command logic
│   ├── analyzer/         # Project analysis
│   ├── simple_state/     # Minimal state management
│   ├── worktree/         # Git worktree management
│   └── lib.rs           # Library exports
├── .mmm/                # Project context and state
│   └── worktrees/       # Isolated git worktrees
└── README.md            # User documentation
```

## Key Capabilities

1. **Dead Simple Interface**
   - Single command: `mmm improve`
   - Optional target score, verbosity, and focus directive
   - Works out of the box

2. **Git-Native Improvement Flow**
   - `/mmm-code-review`: Analyzes code and commits improvement specs
   - `/mmm-implement-spec`: Applies fixes and commits changes
   - `/mmm-lint`: Formats, lints, tests, and commits automated fixes
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

5. **Configurable Workflows**
   - Optional `.mmm/workflow.toml` configuration file
   - Simple list of Claude commands to execute
   - Automatic spec ID extraction for mmm-implement-spec
   - Support for custom workflow sequences

6. **Parallel Execution**
   - Git worktree isolation for concurrent sessions
   - Each session runs in its own branch
   - Commands: `mmm worktree list/merge/clean`
   - Enable with `MMM_USE_WORKTREE=true`

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

Focus on making the core `mmm improve` command robust and reliable:
- Better error handling
- More language support
- Improved Claude context building
- Enhanced progress feedback