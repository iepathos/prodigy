# Prodigy Architecture

## Overview

Prodigy is a workflow orchestration tool that executes Claude commands through structured YAML workflows. It provides session management, state tracking, and supports parallel execution through MapReduce patterns.

## Architectural Pattern: Functional Core, Imperative Shell

Prodigy follows the "Functional Core, Imperative Shell" pattern to achieve:
- **Testability**: Pure functions are easy to test without mocks or complex setup
- **Maintainability**: Clear separation between business logic and I/O operations
- **Predictability**: Pure functions always produce the same output for the same input
- **Composability**: Small, focused functions that combine to create complex behavior

### Functional Core (`src/core/`)

The functional core contains all business logic as pure functions:

- **Pure Functions Only**: No side effects, no I/O operations
- **Data Transformations**: Functions take inputs and return outputs
- **Immutable Data Flow**: Data is transformed, not mutated
- **Easy Testing**: No mocks required, just input/output assertions

#### Core Modules:

- **`src/core/config/`**: Configuration validation and transformation logic
- **`src/core/session/`**: Session state calculations and transitions
- **`src/core/workflow/`**: Workflow parsing and validation logic
- **`src/core/mapreduce/`**: MapReduce work distribution calculations
- **`src/core/validation/`**: Data validation and constraint checking

### Imperative Shell

The imperative shell handles all I/O and side effects, delegating business logic to the core:

- **I/O Operations**: File system, network, database access
- **Side Effects**: Logging, metrics, external system calls
- **Thin Layer**: Minimal logic, primarily orchestration
- **Core Delegation**: Business logic delegated to pure functions

#### Shell Modules:

- **`src/storage/`**: Persistence layer wrapping core data structures
- **`src/worktree/`**: Git operations wrapping core worktree logic
- **`src/cook/execution/`**: Command execution wrapping core workflow logic
- **`src/cli/`**: User interface wrapping core command processing

## Guidelines for Identifying and Refactoring Mixed Functions

### Identifying Mixed Functions

Look for functions that:
1. **Mix I/O with Logic**: File operations interleaved with business rules
2. **Have Multiple Responsibilities**: Both calculating and persisting data
3. **Are Hard to Test**: Require mocks, stubs, or complex test setup
4. **Have Side Effects**: Modify global state, write files, or make network calls
5. **Are Large**: Functions over 20 lines often mix concerns

### Refactoring Strategy

#### Step 1: Extract Pure Logic

```rust
// BEFORE: Mixed function
fn process_workflow(path: &Path) -> Result<()> {
    let content = fs::read_to_string(path)?;  // I/O
    let workflow = serde_yaml::from_str(&content)?;  // Logic

    // Validation logic mixed with I/O
    if workflow.steps.is_empty() {
        log::error!("Empty workflow");  // Side effect
        return Err(anyhow!("Invalid workflow"));
    }

    fs::write("output.json", serde_json::to_string(&workflow)?)?;  // I/O
    Ok(())
}

// AFTER: Separated concerns
// Pure function in src/core/workflow/
pub fn validate_workflow(workflow: &Workflow) -> Result<ValidatedWorkflow> {
    if workflow.steps.is_empty() {
        return Err(ValidationError::EmptyWorkflow);
    }
    Ok(ValidatedWorkflow::from(workflow))
}

// I/O wrapper in src/cook/workflow/
fn process_workflow(path: &Path) -> Result<()> {
    let content = fs::read_to_string(path)?;  // I/O
    let workflow = serde_yaml::from_str(&content)?;

    let validated = core::workflow::validate_workflow(&workflow)?;  // Pure logic

    log::info!("Workflow validated");  // Side effect
    fs::write("output.json", serde_json::to_string(&validated)?)?;  // I/O
    Ok(())
}
```

#### Step 2: Create Data Transformation Pipelines

```rust
// Pure transformation pipeline in core
pub fn transform_session(
    session: Session,
    event: Event
) -> Result<Session> {
    session
        .apply_event(event)
        .and_then(validate_transition)
        .map(calculate_metrics)
        .map(update_timestamps)
}

// I/O shell uses the pipeline
async fn handle_event(event: Event) -> Result<()> {
    let session = storage.load_session()?;  // I/O
    let updated = core::session::transform_session(session, event)?;  // Pure
    storage.save_session(updated)?;  // I/O
    Ok(())
}
```

#### Step 3: Extract Complex Conditionals

```rust
// BEFORE: Complex inline logic
if item.score > 5 && item.category == "critical" &&
   (item.retry_count < 3 || item.override_retry) {
    // process item
}

// AFTER: Named predicate function
fn should_process_item(item: &Item) -> bool {
    is_high_priority(item) && has_retries_available(item)
}

fn is_high_priority(item: &Item) -> bool {
    item.score > 5 && item.category == "critical"
}

fn has_retries_available(item: &Item) -> bool {
    item.retry_count < 3 || item.override_retry
}
```

### Testing Strategy

#### Testing Pure Functions

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_workflow() {
        // Simple input/output test, no mocks needed
        let workflow = Workflow { steps: vec![] };
        let result = validate_workflow(&workflow);
        assert!(result.is_err());
    }
}
```

#### Testing I/O Wrappers

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_process_workflow() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("workflow.yaml");

        // Test only the I/O orchestration
        // Business logic is tested in core module tests
        fs::write(&path, "steps: [test]").unwrap();
        let result = process_workflow(&path);
        assert!(result.is_ok());
    }
}
```

## Core Components

### 1. Unified Session Management (`src/unified_session/`)

The unified session management system provides a single, consolidated approach to handling all session-related functionality across the application. This replaces the previous multi-module session approach with a centralized, consistent model.

#### Key Components:

- **SessionManager** (`manager.rs`): Central interface for all session operations
  - Creates, updates, and manages session lifecycle
  - Direct storage integration (no abstract traits)
  - Handles session persistence and recovery
  - Provides filtering and listing capabilities
  - In-memory cache for active sessions with persistent backing

- **SessionState** (`state.rs`): Session state and metadata
  - `UnifiedSession`: Core session representation for all session types
  - `SessionStatus`: Running, Completed, Failed, Paused, Initializing
  - `SessionType`: Workflow or MapReduce
  - `WorkflowSession`: Workflow-specific data (iterations, files changed, steps)
  - `MapReduceSession`: MapReduce-specific data (items, agents, phases)
  - Support for checkpointing and resumption
  - Built-in timing tracking

- **CookSessionAdapter** (`cook_adapter.rs`): Transitional bridge for cook module
  - Implements cook's existing SessionManager trait
  - Maps cook session operations to unified session operations
  - Handles special metadata keys for incremental updates
  - Maintains backward compatibility during migration

- **Migration** (`migration.rs`): Handles migration from legacy session formats
  - Auto-detects legacy session data
  - One-time migration to new unified format
  - Preserves historical session data
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

- **Session** (`session/`): Legacy session abstractions (transitional)
  - SessionManager trait definition (being phased out)
  - Session state and status tracking (migrating to unified)
  - Integration via CookSessionAdapter bridge

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

The unified session system consolidates all session management into a single, consistent model that serves both workflow and MapReduce executions:

```rust
UnifiedSession {
    id: SessionId,
    session_type: SessionType (Workflow | MapReduce),
    status: SessionStatus,
    metadata: HashMap<String, Value>,
    started_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    checkpoints: Vec<Checkpoint>,
    timings: BTreeMap<String, Duration>,
    error: Option<String>,

    // Type-specific data
    workflow_data: Option<WorkflowSession>,
    mapreduce_data: Option<MapReduceSession>,
}
```

### Session Lifecycle

1. **Initialization**: Session created with type-specific configuration
2. **Running**: Active execution with real-time progress tracking
3. **Updates**: Incremental updates via SessionUpdate enum
4. **Checkpointing**: State snapshots for fault tolerance
5. **Completion**: Final state with success/failure status
6. **Persistence**: Automatic save to GlobalStorage

### Direct Integration Model

The unified session system is now the primary session management layer:

- **Direct Usage**: New code uses `UnifiedSessionManager` directly
- **No Abstract Traits**: Removed `SessionStorage` trait for simplicity
- **Cook Compatibility**: `CookSessionAdapter` provides temporary bridge
- **Single Source of Truth**: All session data flows through unified system
- **Migration Path**: Legacy sessions auto-migrated on first access

### Branch Tracking (Spec 110)

Prodigy tracks the original branch when creating worktrees to enable intelligent merge behavior:

**State Tracking**:
```rust
WorktreeState {
    original_branch: String,  // Branch at worktree creation time
    branch: String,            // Current worktree branch (prodigy-session-*)
    // ... other fields
}
```

**Branch Resolution Logic**:
1. **Capture**: `create_session()` captures current branch via `git rev-parse --abbrev-ref HEAD`
2. **Storage**: Original branch stored in `WorktreeState` for session lifetime
3. **Merge Target**: `get_merge_target()` returns original branch or falls back to default
4. **Fallback**: If original branch deleted, uses default branch (main/master)

**Design Rationale**:
- Supports feature branch workflows where worktrees should merge back to source branch
- Provides safe fallback when original branch is deleted
- Enables flexible merge target selection based on workflow context
- Improves user experience by showing merge target in confirmation prompts

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

## Checkpoint Storage Strategy System (Spec 122)

### Overview

The checkpoint storage strategy system provides type-safe, deterministic path resolution for workflow checkpoints using a pure functional approach. This system replaces ad-hoc path logic with an explicit, composable design that supports multiple storage strategies.

### Storage Strategies

The `CheckpointStorage` enum in `src/cook/workflow/checkpoint_path.rs` defines three explicit storage strategies:

#### 1. Local Storage
```rust
CheckpointStorage::Local(PathBuf)
```
- **Purpose**: Project-local checkpoint storage in `.prodigy/checkpoints/`
- **Use Cases**: Testing, backwards compatibility, isolated project workflows
- **Path Resolution**: Uses provided path directly (pure function behavior)
- **Example**: Local checkpoints that stay within project directory

#### 2. Global Storage
```rust
CheckpointStorage::Global { repo_name: String }
```
- **Purpose**: Repository-scoped storage in `~/.prodigy/state/{repo}/checkpoints/`
- **Use Cases**: Repository-level metadata, shared across all sessions
- **Path Resolution**: `~/.prodigy/state/{repo_name}/checkpoints/`
- **Example**: Shared checkpoint data for all sessions of a project

#### 3. Session Storage (Recommended Default)
```rust
CheckpointStorage::Session { session_id: String }
```
- **Purpose**: Session-scoped storage in `~/.prodigy/state/{session_id}/checkpoints/`
- **Use Cases**: Normal workflow checkpoints (recommended for most workflows)
- **Path Resolution**: `~/.prodigy/state/{session_id}/checkpoints/`
- **Benefits**:
  - Isolation between sessions
  - Survives worktree cleanup
  - Clean session-scoped organization

### Pure Function Path Resolution

All path resolution functions follow functional programming principles:

#### Core Pure Functions

1. **`resolve_base_dir() -> Result<PathBuf>`**
   - Pure function: Same inputs always produce same output
   - No side effects (no I/O, no state mutation)
   - Returns base directory based on storage strategy

2. **`checkpoint_file_path(checkpoint_id: &str) -> Result<PathBuf>`**
   - Composes `resolve_base_dir()` with filename construction
   - Deterministic: Same strategy + ID = Same path
   - Pattern: `{base_dir}/{checkpoint_id}.checkpoint.json`

3. **`resolve_global_base_dir() -> Result<PathBuf>`**
   - Helper function for global/session path resolution
   - Returns `~/.prodigy` from system home directory
   - Pure derivation from environment (home directory)

#### Functional Design Principles

1. **Immutability**: `CheckpointStorage` enum is immutable once constructed
2. **Explicit Configuration**: Storage strategy is always explicit, never inferred
3. **Error as Values**: Returns `Result<T>` instead of panicking
4. **Composition**: Small pure functions compose to build complex paths
5. **Determinism**: Property-based tests verify invariants

### Storage Strategy Selection Guidelines

#### When to Use Local Storage
- **Testing environments**: Keep test artifacts in project directory
- **Backwards compatibility**: Existing workflows expecting local paths
- **Isolated workflows**: Project-specific checkpoints that shouldn't be shared
- **Development**: Quick iteration without polluting global storage

#### When to Use Global Storage
- **Repository metadata**: Data shared across all sessions
- **Cross-session analysis**: Aggregating data from multiple workflow runs
- **Persistent state**: Data that should survive project cleanup
- **CI/CD environments**: Shared state across pipeline runs

#### When to Use Session Storage (Default)
- **Normal workflows**: Standard workflow execution (recommended)
- **Parallel execution**: Isolated checkpoints per session
- **Fault tolerance**: Session-specific recovery without conflicts
- **Clean separation**: Clear boundaries between workflow runs

### Path Resolution Examples

```rust
// Local: Direct path usage
let local = CheckpointStorage::Local(PathBuf::from("/tmp/checkpoints"));
let path = local.checkpoint_file_path("cp-1")?;
// Result: /tmp/checkpoints/cp-1.checkpoint.json

// Global: Repository-scoped
let global = CheckpointStorage::Global {
    repo_name: "prodigy".to_string()
};
let path = global.checkpoint_file_path("cp-1")?;
// Result: ~/.prodigy/state/prodigy/checkpoints/cp-1.checkpoint.json

// Session: Session-scoped (recommended)
let session = CheckpointStorage::Session {
    session_id: "session-abc123".to_string()
};
let path = session.checkpoint_file_path("cp-1")?;
// Result: ~/.prodigy/state/session-abc123/checkpoints/cp-1.checkpoint.json
```

### Testing Strategy

The checkpoint path system uses both unit tests and property-based tests:

#### Property-Based Tests (Using proptest)

The system includes comprehensive property-based tests that verify invariants across arbitrary inputs:

1. **Determinism**: Same strategy + ID always produces same path
2. **Isolation**: Different session IDs produce different paths
3. **Conventions**: All paths end with `.checkpoint.json`
4. **ID Preservation**: Checkpoint ID is always in the filename
5. **Scoping**: Storage paths always contain their scope identifier
6. **Pure Function Behavior**: Local storage returns exact path provided

These tests run on randomly generated inputs to verify the system behaves correctly across all possible valid inputs, not just hand-picked test cases.

### Integration with Workflow System

The checkpoint storage strategy integrates with the broader workflow system:

1. **Orchestrator**: Selects storage strategy based on workflow configuration
2. **Checkpointer**: Uses pure path functions to determine checkpoint locations
3. **Recovery**: Reconstructs paths deterministically from session/workflow ID
4. **Migration**: Legacy paths automatically detected and migrated

### Error Handling

All path resolution functions:
- Return `Result<PathBuf>` for error propagation
- Provide context via `anyhow::Context`
- Never panic in production code
- Handle missing home directory gracefully

## Testing Strategy

- **Unit Tests**: Each module has comprehensive unit tests
- **Integration Tests**: Test interaction between components
- **Migration Tests**: Verify legacy data migration
- **Mock Implementations**: Testing abstractions for isolated testing
- **Property-Based Tests**: Verify system invariants across arbitrary inputs using proptest

## Future Enhancements

1. **Distributed Execution**: Support for multi-machine orchestration
2. **Enhanced Monitoring**: Real-time metrics and dashboards
3. **Plugin System**: Extensible command and executor architecture
4. **Cloud Storage**: Optional cloud-based storage backends
5. **Advanced Scheduling**: Cron-based and event-driven workflows