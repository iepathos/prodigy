# ARCHITECTURE.md - System Design

## Overview

The mmm system follows a modular architecture with clear separation of concerns. Each module is designed to be independent yet composable.

## Core Modules

### 1. Specification Engine (`src/spec/`)
- **parser.rs**: Parses Markdown specifications with YAML frontmatter
- **engine.rs**: Executes specifications iteratively
- **template.rs**: Manages specification templates

### 2. Workflow Engine (`src/workflow/`)
- **engine.rs**: Core workflow execution logic
- **parser.rs**: YAML workflow parsing
- **executor.rs**: Task execution and state management
- **condition.rs**: Pest-based condition evaluation
- **checkpoint.rs**: State persistence and recovery

### 3. Claude Integration (`src/claude/`)
- **api.rs**: HTTP client for Claude API
- **cache.rs**: Response caching layer
- **context.rs**: Context window management
- **memory.rs**: Conversation memory handling
- **models.rs**: API request/response types

### 4. Command System (`src/command/`)
- **registry.rs**: Command registration and lookup
- **dispatcher.rs**: Command routing and execution
- **history.rs**: Command history tracking

### 5. State Management (`src/state/`)
- **manager.rs**: SQLite state persistence (legacy)
- **migrations.rs**: Database schema management (legacy)

### 5a. Simple State Management (`src/simple_state/`) - NEW
- **state.rs**: JSON-based state persistence with corruption recovery
- **cache.rs**: Temporary cache with TTL support
- **learning.rs**: Pattern tracking and improvement suggestions
- **types.rs**: State data structures
- **migration.rs**: SQLite to JSON migration support

### 6. Monitoring (`src/monitor/`)
- **collector.rs**: Metrics collection
- **dashboard.rs**: Web-based monitoring UI
- **analytics.rs**: Usage analytics
- **alert.rs**: Alert system

### 7. Plugin System (`src/plugin/`)
- **loader.rs**: Dynamic plugin loading
- **sandbox.rs**: Security sandbox
- **registry.rs**: Plugin management
- **api.rs**: Plugin API surface

### 8. Iterative Improvement Loop (`src/loop/`)
- **engine.rs**: Core loop orchestration and session management
- **config.rs**: Loop configuration and termination conditions
- **session.rs**: Session state and iteration data management
- **metrics.rs**: Quality metrics and performance tracking
- **commands.rs**: Workflow step commands for loop execution

### 9. Dead Simple Improve (`src/improve/`)
- **analyzer.rs**: Project analysis and language detection
- **context.rs**: Smart context building for Claude
- **session.rs**: Improvement session management
- **display.rs**: Progress display and user feedback
- **command.rs**: CLI command implementation

### 10. Smart Project Analyzer (`src/analyzer/`)
- **language.rs**: Programming language detection
- **framework.rs**: Framework and library detection
- **structure.rs**: Project structure analysis
- **health.rs**: Project health indicators
- **build.rs**: Build tool detection and analysis
- **quality.rs**: Code quality metrics
- **focus.rs**: Improvement area prioritization
- **context.rs**: Analysis report generation

### 11. Simple State Management (`src/simple_state/`)
- **state.rs**: JSON-based state persistence with atomic writes
- **cache.rs**: TTL-based caching for temporary data
- **learning.rs**: Learning system for tracking improvement patterns
- **types.rs**: Clean type definitions for state
- **migration.rs**: SQLite to JSON migration support

### 12. Developer Experience (`src/developer_experience/`)
- **progress.rs**: Real-time progress displays with beautiful animations
- **summary.rs**: Result summaries with quality scores and impact metrics
- **interactive.rs**: Live preview mode and graceful interruption
- **error_handling.rs**: Smart error messages with rollback capability
- **suggestions.rs**: Context-aware help and next action suggestions
- **celebration.rs**: Achievements, streaks, and gamification
- **shell.rs**: Shell completions and git hook integration
- **performance.rs**: Fast startup and incremental processing

## Data Flow

```
User Input → Command Parser → Command Dispatcher
                                    ↓
                            Specification Engine
                                    ↓
                            Workflow Engine ←→ Iteration Engine
                                    ↓              ↓
                            Claude API Client ←----┘
                                    ↑
                            Improve Engine
                                    ↓
                            State Manager
                                    ↓
                            Simple State (JSON)
                                    ↓
                            Monitor/Analytics
```

## Key Design Patterns

### 1. Registry Pattern
Used for commands, plugins, and specifications to allow dynamic extension.

### 2. Builder Pattern
Used for complex object construction (workflows, API requests).

### 3. Repository Pattern
State management abstracts storage details behind a clean interface.

### 4. Observer Pattern
Monitoring system observes and reacts to system events.

### 5. Strategy Pattern
Different execution strategies for specs and workflows.

## Error Handling

- Custom error types with `thiserror`
- Result<T, Error> throughout
- Graceful degradation
- Comprehensive error context

## Concurrency Model

- Tokio async runtime
- Parallel spec execution where possible
- Resource pooling for API calls
- Lock-free state updates where feasible

## Security Considerations

- Plugin sandboxing
- API key encryption
- Input validation
- Rate limiting

## Performance Optimizations

- Response caching
- Lazy loading
- Connection pooling
- Efficient serialization

## Extension Points

1. **Custom Commands**: Register via command system
2. **Plugins**: Load dynamically with API access
3. **Workflow Steps**: Add custom executors
4. **Monitors**: Implement custom collectors