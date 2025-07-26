# ARCHITECTURE.md - Simple, Focused Design

## Overview

MMM follows a dead simple architecture with clear separation of concerns. The entire system is focused on one thing: making code better through Claude CLI integration.

## Core Modules

### 1. CLI Interface (`src/main.rs`)
- Single command: `mmm improve`
- Optional flags: `--target`, `--verbose`
- Direct entry point to improvement logic

### 2. Improve Command (`src/improve/`)
- **command.rs**: CLI argument parsing and main workflow
- **analyzer.rs**: Project analysis (language, framework, health)
- **context.rs**: Smart context building for Claude
- **session.rs**: Improvement session management
- **state_adapter.rs**: Bridge to simple state management

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

## Data Flow

```
User runs `mmm improve`
        ↓
Analyze project (language, framework, health score)
        ↓
Build context for Claude CLI
        ↓
Call Claude CLI with improvement request
        ↓
Parse Claude response and apply changes
        ↓
Update state and repeat until target reached
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
├── improve/             # Core improvement logic
│   ├── mod.rs
│   ├── command.rs       # Main improve command
│   ├── analyzer.rs      # Project analysis  
│   ├── context.rs       # Claude context building
│   ├── session.rs       # Session management
│   └── state_adapter.rs # State bridge
├── analyzer/            # Analysis components
│   ├── mod.rs
│   ├── language.rs      # Language detection
│   ├── framework.rs     # Framework detection
│   ├── health.rs        # Health scoring
│   └── focus.rs         # Focus areas
└── simple_state/        # Minimal state
    ├── mod.rs
    ├── state.rs         # JSON state management
    ├── cache.rs         # Temporary caching
    └── types.rs         # Data structures
```