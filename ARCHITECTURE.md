# Prodigy Architecture

## Overview

Prodigy is a workflow orchestration tool that executes Claude commands through structured YAML workflows. It provides session management, state tracking, and supports parallel execution through MapReduce patterns.

## Core Components

### 1. Unified Session Management (`src/unified_session/`)

The unified session management system provides a single, consolidated approach to handling all session-related functionality across the application.

#### Key Components:

- **SessionManager** (`manager.rs`): Central interface for all session operations
  - Creates, updates, and manages session lifecycle
  - Handles session persistence and recovery
  - Provides filtering and listing capabilities

- **SessionState** (`state.rs`): Session state and metadata
  - `UnifiedSession`: Core session representation
  - `SessionStatus`: Running, Completed, Failed, Paused, etc.
  - `SessionType`: Workflow or MapReduce
  - Support for checkpointing and resumption

- **CookSessionAdapter** (`cook_adapter.rs`): Bridge between cook module and unified sessions
  - Implements cook's SessionManager trait
  - Maps cook session operations to unified session operations
  - Maintains backward compatibility

- **Migration** (`migration.rs`): Handles migration from legacy session formats
  - Auto-detects legacy session data
  - Migrates to new unified format
  - Archives old data after successful migration

- **Timing** (`timing.rs`): Performance tracking utilities
  - `TimingTracker`: Tracks iteration and command durations
  - `format_duration`: Human-readable duration formatting

### 2. Cook Module (`src/cook/`)

The cook module handles workflow execution and orchestration.

#### Components:

- **Orchestrator** (`orchestrator.rs`): Main workflow execution engine
  - Manages workflow lifecycle
  - Coordinates between different executors
  - Handles error recovery and retries

- **Workflow** (`workflow/`): Workflow definition and execution
  - `WorkflowExecutor`: Executes workflow steps sequentially
  - Checkpoint support for resumption
  - Validation and error handling

- **MapReduce** (`execution/mapreduce/`): Parallel execution framework
  - Distributes work across multiple agents
  - Handles map phase, reduce phase, and aggregation
  - Dead Letter Queue (DLQ) for failed items

- **Session** (`session/`): Cook-specific session abstractions
  - SessionManager trait definition
  - Session state and status tracking
  - Integration with unified session via adapter

### 3. Storage (`src/storage/`)

Global storage architecture for persistent data.

#### Features:

- **GlobalStorage**: Centralized storage management
  - Events, state, and DLQ storage
  - Cross-worktree data sharing
  - Efficient deduplication

- **Event Logging**: Comprehensive event tracking
  - Structured event storage
  - Support for querying and filtering
  - Integration with Claude streaming

### 4. Worktree Management (`src/worktree/`)

Git worktree-based isolation for parallel execution.

- Creates isolated environments for each session
- Manages worktree lifecycle
- Handles merge operations back to main branch

### 5. State Management (`src/simple_state/`)

Simple JSON-based state persistence for project metadata.

- Human-readable JSON files
- Git-friendly text format
- Zero configuration required
- Atomic operations for concurrent access

## Data Flow

1. **Session Creation**:
   ```
   User Command → Cook Module → CookSessionAdapter → UnifiedSessionManager → Storage
   ```

2. **Workflow Execution**:
   ```
   Workflow Config → Orchestrator → WorkflowExecutor → Session Updates → Storage
   ```

3. **MapReduce Processing**:
   ```
   MapReduce Config → MapReduceExecutor → Agent Spawning → Parallel Execution → Aggregation
   ```

## Session Architecture

### Unified Session Model

The unified session system consolidates all session management into a single, consistent model:

```rust
UnifiedSession {
    id: SessionId,
    session_type: SessionType (Workflow | MapReduce),
    status: SessionStatus,
    metadata: SessionMetadata,
    started_at: DateTime,
    ended_at: Option<DateTime>,
    workflow_data: Option<WorkflowSession>,
    mapreduce_data: Option<MapReduceSession>,
    checkpoints: Vec<Checkpoint>,
    error: Option<String>,
}
```

### Session Lifecycle

1. **Initialization**: Session created with metadata and configuration
2. **Running**: Active execution with progress tracking
3. **Checkpointing**: Periodic state saves for resumption
4. **Completion**: Final state with summary and metrics
5. **Migration**: Legacy sessions automatically migrated on first run

### Adapter Pattern

The `CookSessionAdapter` provides a compatibility layer between the cook module's session abstractions and the unified session system:

- Cook module uses its own `SessionManager` trait
- Adapter implements this trait using unified session operations
- Allows gradual migration without breaking existing code

## Storage Architecture

### Directory Structure

```
~/.prodigy/
├── events/
│   └── {repo_name}/
│       └── {job_id}/
│           └── events-{timestamp}.jsonl
├── state/
│   └── {repo_name}/
│       └── sessions/
│           └── {session_id}.json
├── dlq/
│   └── {repo_name}/
│       └── {job_id}/
│           └── failed_items.json
└── worktrees/
    └── {repo_name}/
        └── {session_id}/
```

### Benefits of Unified Architecture

1. **Consistency**: Single source of truth for all session data
2. **Reliability**: Automatic persistence and recovery
3. **Scalability**: Support for parallel execution and cross-worktree coordination
4. **Maintainability**: Clear separation of concerns and modular design
5. **Migration**: Seamless transition from legacy formats
6. **Observability**: Comprehensive event logging and metrics

## Testing Strategy

- **Unit Tests**: Each module has comprehensive unit tests
- **Integration Tests**: Test interaction between components
- **Migration Tests**: Verify legacy data migration
- **Mock Implementations**: Testing abstractions for isolated testing

## Future Enhancements

1. **Distributed Execution**: Support for multi-machine orchestration
2. **Enhanced Monitoring**: Real-time metrics and dashboards
3. **Plugin System**: Extensible command and executor architecture
4. **Cloud Storage**: Optional cloud-based storage backends
5. **Advanced Scheduling**: Cron-based and event-driven workflows