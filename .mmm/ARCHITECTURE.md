# ARCHITECTURE.md - Simple, Focused Design

## Overview

MMM follows a dead simple architecture with clear separation of concerns. The entire system is focused on one thing: making code better through Claude CLI integration.

## Core Modules

### 1. CLI Interface (`src/main.rs`)
- Main commands: `mmm cook`, `mmm worktree`, `mmm init`
- Global flags: `--verbose`
- Direct entry point to subcommands

### 2. Cook Command (`src/cook/`)
- **mod.rs**: Core cooking loop with Claude CLI integration and mapping support
- **command.rs**: CLI with playbook positional argument, path option, focus, config, map, args, and fail-fast flags
- **session.rs**: Minimal session data structures
- **workflow.rs**: Playbook-driven workflow execution with command chaining and variable resolution
- **git_ops.rs**: Thread-safe git operations
- Supports file mapping with `--map` for batch processing
- Command output/input chaining with variable substitution (${command_id.output_name})
- Flexible output extraction from git commits, stdout, files, or direct values

### 3. Context Analysis (`src/context/`)
- **analyzer.rs**: Main analyzer orchestrating all components
- **dependencies.rs**: Dependency graph and module relationships
- **architecture.rs**: Architecture pattern detection
- **conventions.rs**: Convention and naming style detection
- **debt.rs**: Technical debt mapping and prioritization  
- **test_coverage.rs**: Test coverage gap analysis
- **mod.rs**: Module exports and data structures

### 4. State Management (`src/simple_state/`)
- **state.rs**: JSON-based session tracking
- **cache.rs**: Temporary analysis caching
- **types.rs**: Clean data structures
- **mod.rs**: Module exports

### 5. Claude Integration (`src/claude/`)
- **api.rs**: Claude CLI subprocess execution
- **commands.rs**: Command registry for Claude interactions
- **context.rs**: Context management for Claude prompts
- **memory.rs**: Conversation memory tracking
- **models.rs**: Data models for Claude interactions
- **prompt.rs**: Prompt engineering utilities
- **response.rs**: Response parsing and processing
- **token.rs**: Token counting and management
- **cache.rs**: Response caching for efficiency

### 6. Configuration Management (`src/config/`)
- **loader.rs**: Configuration file loading (YAML/JSON playbooks)
- **validator.rs**: Configuration validation
- **mod.rs**: Config structures and defaults
- **workflow.rs**: Workflow configuration structure (no default)
- **command.rs**: Structured command objects with ID, inputs, outputs
- **command_parser.rs**: String-to-Command conversion utilities
- **command_validator.rs**: Command registry and validation logic
- Supports command output/input declarations
- Variable resolution between commands
- Multiple output extraction methods

### 7. Error Handling (`src/error.rs`)
- Centralized error types using thiserror
- Consistent error propagation throughout codebase
- Context-aware error messages

### 8. Project Management (`src/project/`)
- **manager.rs**: Project lifecycle management
- **health.rs**: Project health checking
- **template.rs**: Project template utilities
- **mod.rs**: Project data structures

### 9. Worktree Management (`src/worktree/`)
- **manager.rs**: Git worktree lifecycle management
- **state.rs**: WorktreeState data structures and metadata persistence
- **mod.rs**: WorktreeSession data structure and exports
- Worktrees are stored in `~/.mmm/worktrees/{repo-name}/` to comply with git restrictions
- State metadata stored in `~/.mmm/worktrees/{repo-name}/.metadata/` (gitignored)

### 10. Init Command (`src/init/`)
- **mod.rs**: Main initialization logic and git repository validation
- **command.rs**: CLI command structure for init subcommand
- **templates.rs**: Embedded MMM command templates
- Bootstraps new projects with required .claude/commands
- Handles command conflicts and selective installation

### 11. Metrics (`src/metrics/`)
- **collector.rs**: Orchestrates metrics collection across analyzers
- **quality.rs**: Test coverage, lint warnings, documentation metrics
- **complexity.rs**: Cyclomatic and cognitive complexity analysis using syn
- **performance.rs**: Compile time, binary size, benchmark tracking
- **history.rs**: Metrics history and trend analysis
- **storage.rs**: Persistence and report generation
- **mod.rs**: Public interfaces and data structures
- Metrics are collected after each iteration when --metrics flag is used
- Historical data stored in .mmm/metrics/ for trend analysis


## Data Flow

```
User runs `mmm cook playbook.yml`
        ↓
[Optional] Create git worktree if --worktree flag used
        ↓
Analyze project context (dependencies, architecture, conventions, debt, coverage)
        ↓
Load playbook (YAML/JSON) with workflow definition
        ↓
Execute workflow commands in sequence:
  - Resolve input variables from previous command outputs
  - Call Claude CLI with resolved arguments
  - Extract outputs based on declarations
  - Store outputs for next commands
        ↓
Update state and repeat until target reached
        ↓
[Optional] Merge worktree changes back to main branch
```

## Key Design Principles

### 1. Dead Simple
- Single command interface
- Minimal configuration required
- Works out of the box

### 2. Actually Functional
- Real Claude CLI integration
- Real file modifications
- Real improvement tracking

### 3. Minimal State
- JSON files only
- Essential data only
- Human-readable format

### 4. Clear Code
- Single responsibility modules
- Straightforward control flow
- Minimal abstractions

### 5. Self-Sufficient
- Automatic project analysis
- Automatic termination conditions
- No manual intervention required

## Error Handling

- Result<T, Error> throughout
- Graceful degradation
- Clear error messages
- Safe file operations

## Performance

- Fast startup
- Efficient Claude CLI calls
- Minimal memory usage
- Cached analysis results

## Extension Points

The architecture is intentionally minimal, but allows for:
1. **Additional Languages**: Extend analyzer module
2. **Better Context**: Improve context building logic  
3. **Enhanced State**: Add fields to JSON structures
4. **Better UX**: Enhance progress feedback

## File Organization

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library exports
├── error.rs             # Centralized error handling
├── cook/                # Core cooking logic
│   ├── mod.rs           # Single consolidated improvement loop
│   ├── command.rs       # CLI args only
│   ├── session.rs       # Basic session data
│   ├── workflow.rs      # Configurable workflow execution
│   └── git_ops.rs       # Thread-safe git operations
├── context/             # Context-aware analysis
│   ├── mod.rs           # Module exports and types
│   ├── analyzer.rs      # Main analyzer
│   ├── dependencies.rs  # Dependency graphs
│   ├── architecture.rs  # Architecture detection
│   ├── conventions.rs   # Convention learning
│   ├── debt.rs          # Technical debt
│   └── test_coverage.rs # Coverage analysis
├── claude/              # Claude CLI integration
│   ├── mod.rs
│   ├── api.rs           # CLI subprocess execution
│   ├── commands.rs      # Command registry
│   ├── context.rs       # Context management
│   ├── memory.rs        # Conversation memory
│   ├── models.rs        # Data models
│   ├── prompt.rs        # Prompt engineering
│   ├── response.rs      # Response parsing
│   ├── token.rs         # Token management
│   └── cache.rs         # Response caching
├── config/              # Configuration management
│   ├── mod.rs
│   ├── loader.rs        # Config loading
│   ├── validator.rs     # Config validation
│   └── workflow.rs      # Workflow config structures
├── project/             # Project management
│   ├── mod.rs
│   ├── manager.rs       # Project lifecycle
│   ├── health.rs        # Health checking
│   └── template.rs      # Template utilities
├── session/             # Event-driven session management
│   ├── mod.rs
│   ├── state.rs         # State machine
│   ├── events.rs        # Event system
│   ├── manager.rs       # Session management
│   ├── config.rs        # Configuration types
│   ├── persistence.rs   # Persistence types
│   └── storage.rs       # Storage backends
├── simple_state/        # Minimal state
│   ├── mod.rs
│   ├── state.rs         # JSON state management
│   ├── cache.rs         # Temporary caching
│   └── types.rs         # Data structures
├── worktree/            # Git worktree management
│   ├── mod.rs
│   ├── manager.rs       # Worktree lifecycle
│   ├── state.rs         # Worktree state persistence
│   └── tests.rs         # Unit tests
└── init/                # Command initialization
    ├── mod.rs           # Initialization logic
    ├── command.rs       # CLI command structure
    └── templates.rs     # Embedded command templates

# Worktrees stored in home directory:
~/.mmm/worktrees/{repo-name}/
├── .gitignore           # Contains ".metadata/"
├── .metadata/           # State directory (ignored by git)
│   ├── session-{uuid}.json
│   └── ...
├── session-{uuid}/      # Actual git worktree
└── ...
```