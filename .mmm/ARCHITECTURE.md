# ARCHITECTURE.md - Simple, Focused Design

## Overview

MMM follows a dead simple architecture with clear separation of concerns. The entire system is focused on one thing: making code better through Claude CLI integration.

## Core Modules

### 1. CLI Interface (`src/main.rs`)
- Single command: `mmm improve`
- Optional flags: `--target`, `--verbose`
- Direct entry point to improvement logic

### 2. Improve Command (`src/improve/`)
- **mod.rs**: Single core improvement loop with Claude CLI integration
- **command.rs**: Simplified CLI with only target and verbose flags
- **session.rs**: Minimal session data structures
- **workflow.rs**: Simple configurable workflow execution

### 3. Project Analysis (`src/analyzer/`)
- **language.rs**: Programming language detection
- **framework.rs**: Framework and library detection  
- **health.rs**: Basic code quality metrics
- **focus.rs**: Improvement area prioritization

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
- **loader.rs**: Configuration file loading (including .mmm/workflow.toml)
- **validator.rs**: Configuration validation
- **mod.rs**: Config structures and defaults
- **workflow.rs**: Simple workflow configuration structure

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
- **mod.rs**: WorktreeSession data structure and exports

## Data Flow

```
User runs `mmm improve`
        ↓
[Optional] Create git worktree if MMM_USE_WORKTREE=true
        ↓
Analyze project (language, framework, health score)
        ↓
Load configuration (including .mmm/workflow.toml)
        ↓
Execute workflow commands in sequence
        ↓
Each command: Call Claude CLI (auto-extract spec ID for mmm-implement-spec)
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
├── improve/             # Core improvement logic
│   ├── mod.rs           # Single consolidated improvement loop
│   ├── command.rs       # CLI args only
│   ├── session.rs       # Basic session data
│   └── workflow.rs      # Configurable workflow execution
├── analyzer/            # Analysis components
│   ├── mod.rs
│   ├── language.rs      # Language detection
│   ├── framework.rs     # Framework detection
│   ├── health.rs        # Health scoring
│   └── focus.rs         # Focus areas
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
├── simple_state/        # Minimal state
│   ├── mod.rs
│   ├── state.rs         # JSON state management
│   ├── cache.rs         # Temporary caching
│   └── types.rs         # Data structures
└── worktree/            # Git worktree management
    ├── mod.rs
    ├── manager.rs       # Worktree lifecycle
    └── tests.rs         # Unit tests
```