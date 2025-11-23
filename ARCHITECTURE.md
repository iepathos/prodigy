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

## Validation Architecture (Spec 163)

### Overview

Prodigy's validation system is built on the stillwater library's `Validation` type, which provides error accumulation and functional composition patterns. This architecture replaces traditional fail-fast validation with a comprehensive approach that reports all errors at once.

### Core Principles

1. **Error Accumulation**: Collect all validation errors before failing
2. **Pure Functions**: Validation logic separated from I/O operations
3. **Composable Validators**: Build complex validators from simple, reusable functions
4. **Error Classification**: Distinguish between errors (blocking) and warnings (informational)

### Validation Module Structure

The validation module is located in `src/core/validation/` and contains pure validation functions:

```rust
// Core validation type from stillwater
use stillwater::Validation;

// All validators return Validation<T, Vec<ValidationError>>
pub fn validate_command(command: &str) -> ValidationResult {
    // Accumulates multiple errors in single pass
}

pub fn validate_paths(
    paths: &[&Path],
    exists_check: FileExistsCheck
) -> ValidationResult {
    // I/O injected as parameter - pure function
}

pub fn validate_environment(
    required_vars: &[&str],
    env_vars: &HashMap<String, String>
) -> ValidationResult {
    // Validates all variables, accumulates all errors
}
```

### Error Accumulation Pattern

Traditional validation stops at the first error:

```rust
// ❌ Fail-fast approach (old pattern)
fn validate(items: &[Item]) -> Result<()> {
    for item in items {
        if item.is_invalid() {
            return Err(error); // Stops here - user only sees first error
        }
    }
    Ok(())
}
```

Stillwater validation accumulates all errors:

```rust
// ✅ Error accumulation (stillwater pattern)
fn validate(items: &[Item]) -> ValidationResult {
    let mut all_errors = Vec::new();

    for item in items {
        match validate_item(item).into_result() {
            Ok(_) => {},
            Err(errors) => all_errors.extend(errors), // Collect all errors
        }
    }

    ValidationResult::from_validation(
        if all_errors.is_empty() {
            Validation::success(())
        } else {
            Validation::failure(all_errors)
        }
    )
}
```

### Error Types and Severity

All validation errors are defined in the `ValidationError` enum with severity classification:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    // Path validation
    PathNotFound(PathBuf),
    PathInParentDir(PathBuf),      // Warning
    PathInTempDir(PathBuf),         // Warning

    // Environment validation
    EnvVarMissing(String),
    EnvVarEmpty(String),            // Warning

    // Command validation
    CommandEmpty,
    CommandDangerous { cmd: String, pattern: String },
    CommandSuspicious { cmd: String, reason: String }, // Warning

    // Resource validation
    IterationCountZero,
    IterationCountHigh(usize),      // Warning
    MemoryLimitZero,
    MemoryLimitLow(usize),          // Warning
    TimeoutZero,
}

pub enum ErrorSeverity {
    Error,   // Blocks execution
    Warning, // Reported but non-blocking
}
```

### Composition Patterns

Validators compose to build complex validation logic:

```rust
// Small, focused validators
fn check_dangerous_patterns(cmd: &str) -> Validation<(), Vec<ValidationError>> {
    // Returns errors for dangerous patterns
}

fn check_suspicious_patterns(cmd: &str) -> Validation<(), Vec<ValidationError>> {
    // Returns warnings for suspicious patterns
}

// Composed validator
pub fn validate_command(command: &str) -> ValidationResult {
    let mut all_errors = Vec::new();

    // Compose multiple validators
    if let Err(errors) = check_dangerous_patterns(command).into_result() {
        all_errors.extend(errors);
    }

    if let Err(errors) = check_suspicious_patterns(command).into_result() {
        all_errors.extend(errors);
    }

    ValidationResult::from_validation(
        if all_errors.is_empty() {
            Validation::success(())
        } else {
            Validation::failure(all_errors)
        }
    )
}
```

### Dependency Injection for I/O

Validators use dependency injection to remain pure:

```rust
// File existence check passed as parameter
pub type FileExistsCheck = fn(&Path) -> bool;

pub fn validate_paths(
    paths: &[&Path],
    exists_check: FileExistsCheck  // I/O injected here
) -> ValidationResult {
    // Pure logic - no filesystem access
    for path in paths {
        if !exists_check(path) {
            errors.push(ValidationError::PathNotFound(path.to_path_buf()));
        }
    }
    // ...
}

// Shell code provides I/O implementation
fn validate_workflow_paths(paths: &[&Path]) -> ValidationResult {
    validate_paths(paths, |p| p.exists()) // I/O at the edge
}

// Test code provides mock implementation
#[test]
fn test_validate_paths() {
    fn mock_exists(path: &Path) -> bool {
        path.to_str().unwrap().contains("exists")
    }

    let result = validate_paths(&paths, mock_exists); // No filesystem access
    assert_eq!(result.errors.len(), 2);
}
```

### Backward Compatibility

The `ValidationResult` type provides backward compatibility with existing code:

```rust
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Convert from stillwater Validation
    pub fn from_validation<T>(v: Validation<T, Vec<ValidationError>>) -> Self {
        match v.into_result() {
            Ok(_) => Self::valid(),
            Err(errors) => {
                let mut result = Self::valid();
                for error in errors {
                    match error.severity() {
                        ErrorSeverity::Error => result.add_error(error.to_string()),
                        ErrorSeverity::Warning => result.add_warning(error.to_string()),
                    }
                }
                result
            }
        }
    }
}
```

### Benefits

1. **Better User Experience**: Users see ALL validation errors at once
2. **Improved Testing**: Pure functions with no I/O dependencies
3. **Composability**: Build complex validators from simple building blocks
4. **Consistency**: Uniform error types and handling across the codebase
5. **Performance**: Single-pass validation with no additional overhead

### Performance Characteristics

The stillwater validation migration maintains zero performance regression:

- **No allocation overhead**: Error vectors are allocated once
- **Single-pass validation**: All errors found in one traversal
- **Compiler optimization**: Small validators inline effectively
- **Benchmark verification**: See `benches/execution_benchmarks.rs::bench_validation_performance`

Run validation benchmarks:
```bash
cargo bench --bench execution_benchmarks -- validation_performance
```

### Migration Guide

For detailed migration examples and patterns, see:
- `docs/stillwater-validation-migration.md` - Before/after code examples
- `src/core/validation/mod.rs` - Reference implementation
- `specs/163-stillwater-validation-library.md` - Original specification

## Testing Strategy

- **Unit Tests**: Each module has comprehensive unit tests
- **Integration Tests**: Test interaction between components
- **Migration Tests**: Verify legacy data migration
- **Mock Implementations**: Testing abstractions for isolated testing
- **Property-Based Tests**: Verify system invariants across arbitrary inputs using proptest
- **Validation Tests**: Error accumulation tests verify all errors are reported

## Spec 166: Complex Function Refactoring (2025-11-22)

As part of continuous improvement, Prodigy underwent systematic refactoring to reduce function complexity and improve maintainability following functional programming principles.

### Refactoring Objectives

- **Function Size**: Reduce all functions to < 20 lines (prefer 5-10)
- **Nesting Depth**: Maximum 2 levels of nesting
- **Single Responsibility**: Each function does one thing well
- **Pure Function Extraction**: Separate business logic from I/O
- **Composability**: Build complex behavior from small, testable functions

### Modules Refactored

#### 1. `cook/orchestrator/core.rs` (2829 lines)
- **setup_environment**: 98 lines → 27 lines + 3 helper functions
- **cleanup**: 71 lines → 8 lines + 4 helper functions
- **execute_and_validate_command**: 95 lines → 33 lines + 4 pure validators
- Extracted 13 pure functions to `cook/orchestrator/construction.rs`
- All pure functions have comprehensive unit tests

####2. `cook/execution/mapreduce/checkpoint_integration.rs` (2515 lines)
- **initialize_checkpoint_state**: 65 lines → 5 lines + 7 helper functions
- **get_next_batch**: 34 lines (3-level nesting) → 8 lines (1-level nesting)
- **process_batch**: 32 lines → 5 lines + 2 pure functions
- **update_checkpoint_with_results**: 51 lines → 16 lines + 3 handlers
- **resume_from_checkpoint**: 100 lines → 24 lines + 3 phase handlers
- Created 24 new focused functions, all < 20 lines

#### 3. `cook/workflow/executor.rs` (2218 lines)
- **determine_command_type**: 77 lines → 7 lines + 5 pure helpers
- **save_workflow_state**: 22 lines → 14 lines (pure extraction)
- **handle_no_commits_error**: 55 lines → 8 lines + 4 message builders
- **execute_internal**: 293 lines → 3 orchestration functions
- Extracted 9 pure functions to `executor/pure.rs`

#### 4. `cook/execution/variables.rs` (2204 lines)
- **extract_json_path**: 37 lines, 4-level nesting → 4 lines, 1-level nesting
- **resolve_by_type**: 38 lines → 13 lines + 5 specialized resolvers
- **resolve_json_variable**: 38 lines, 6-level nesting → 16 lines, 2-level nesting
- **aggregate functions** (min, max, median, variance): 24-29 lines → 10-16 lines
- Created 38 new helper functions, all pure and focused
- Eliminated code duplication across aggregation functions

#### 5. `cook/execution/state.rs` (1749 lines)
- **update_agent_result**: 57 lines → 6 lines + 6 helper functions + 3 pure extractors
- **save_checkpoint**: 80 lines → 14 lines + 4 I/O pipelines + 3 pure helpers
- **load_checkpoint_by_version**: 51 lines → 10 lines + 4 path resolution helpers
- **list_checkpoints**: 39 lines → 8 lines + 5 pure parsing functions
- Created reusable `write_file_atomically` primitive

### Functional Programming Patterns Applied

#### 1. Pure Function Extraction
```rust
// BEFORE: Mixed logic and I/O
fn process_item(item: &Item) -> Result<()> {
    if item.validate() {  // Pure logic
        fs::write("result.txt", "success")?;  // I/O
    }
    Ok(())
}

// AFTER: Separated concerns
fn validate_item(item: &Item) -> bool {  // Pure
    // Validation logic
}

fn write_result(path: &Path, content: &str) -> Result<()> {  // I/O wrapper
    fs::write(path, content)
}

fn process_item(item: &Item) -> Result<()> {  // Thin orchestration
    if validate_item(item) {
        write_result(Path::new("result.txt"), "success")?;
    }
    Ok(())
}
```

#### 2. Function Composition
```rust
// BEFORE: Monolithic function
fn complex_operation(data: Data) -> Result<Output> {
    // 100 lines of sequential logic
}

// AFTER: Composed from small functions
fn complex_operation(data: Data) -> Result<Output> {
    step1(data)
        .and_then(step2)
        .and_then(step3)
        .map(finalize)
}
```

#### 3. Reduced Nesting
```rust
// BEFORE: Deep nesting
fn process(item: Option<Item>) -> Result<()> {
    if let Some(item) = item {
        if item.valid {
            if let Some(data) = item.data {
                // Process...
            }
        }
    }
    Ok(())
}

// AFTER: Early returns + functional chains
fn process(item: Option<Item>) -> Result<()> {
    let item = item.ok_or(Error::MissingItem)?;
    if !item.valid {
        return Ok(());
    }
    let data = item.data.ok_or(Error::MissingData)?;
    process_data(&data)
}
```

### Benefits Achieved

1. **Testability**: Pure functions testable without I/O mocks
2. **Readability**: Small functions self-document through clear names
3. **Maintainability**: Easy to locate and modify specific behavior
4. **Reusability**: Helper functions composable across modules
5. **Debugging**: Reduced complexity simplifies troubleshooting
6. **Code Review**: Smaller units easier to review thoroughly

### Metrics

- **Functions Refactored**: 14 major complex functions
- **Helper Functions Created**: 90+ new focused functions
- **Average Function Length**: Reduced from ~50 lines to ~12 lines
- **Max Nesting Depth**: Reduced from 4-6 levels to 1-2 levels
- **Test Coverage**: Pure functions have dedicated unit tests

### Ongoing Work

Some refactored functions are still being integrated:
- Test failures in `cook/orchestrator/core.rs` (6 tests) due to behavior changes
- These will be addressed in follow-up work to ensure exact behavioral equivalence

### Refactoring Guidelines

When refactoring complex functions in Prodigy:

1. **Identify** functions > 20 lines or complexity > 5
2. **Extract** pure logic to helper functions (< 10 lines each)
3. **Separate** I/O operations into thin wrappers
4. **Compose** at higher level using functional patterns
5. **Test** each pure function independently
6. **Verify** original tests still pass

See spec 166 for detailed refactoring patterns and examples.

## Spec 169: Pure State Transitions with Stillwater Effects (2025-11-23)

### Overview

The MapReduce state management module (`src/cook/execution/state_pure/`) implements a pure functional approach to state transitions using the Stillwater Effect library. This design provides a clear separation between pure business logic and I/O operations, enabling comprehensive testing and improved maintainability.

### Module Structure

```
src/cook/execution/state_pure/
├── mod.rs         # Public API and module exports
├── pure.rs        # Pure state transition functions (40+ unit tests)
├── io.rs          # Effect-based I/O operations (20+ integration tests)
└── types.rs       # State types and data structures
```

### Pure Core (pure.rs)

All MapReduce state transitions are implemented as pure functions:

```rust
// Pure function: takes state, returns new state
pub fn apply_agent_result(
    state: MapReduceJobState,
    result: AgentResult
) -> MapReduceJobState {
    // No I/O, no side effects
    // Same inputs always produce same outputs
    // Easy to test with simple assertions
}
```

**Key Pure Functions**:
- `apply_agent_result()` - Updates state with agent completion
- `should_transition_to_reduce()` - Determines phase transition
- `get_retriable_items()` - Calculates retry-eligible items
- `start_reduce_phase()` - Initializes reduce phase state
- `complete_reduce_phase()` - Finalizes reduce phase
- `mark_complete()` - Marks job as complete
- `update_variables()` - Updates workflow variables
- `set_parent_worktree()` - Sets worktree reference

All pure functions:
- Return new state instead of mutating
- Contain zero I/O operations
- Are independently testable
- Have comprehensive unit test coverage

### Imperative Shell (io.rs)

I/O operations are wrapped in Stillwater Effect types for lazy evaluation and composition:

```rust
// Effect type for state operations
pub type StateEffect<T> = Effect<T, anyhow::Error, StateEnv>;

// Storage backend trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn write_checkpoint(&self, job_id: &str, data: &str) -> Result<()>;
    async fn read_checkpoint(&self, job_id: &str) -> Result<String>;
}

// Event log trait
#[async_trait]
pub trait EventLog: Send + Sync {
    async fn log_checkpoint_saved(&self, job_id: &str) -> Result<()>;
    async fn log_phase_transition(&self, job_id: &str, phase: &str) -> Result<()>;
}
```

**Effect-Based Operations**:
- `save_checkpoint()` - Persists state to storage
- `load_checkpoint()` - Retrieves state from storage
- `update_with_agent_result()` - Composes pure update + save
- `complete_batch()` - Processes batch + checkpoint + transition
- `start_reduce_phase_with_save()` - Reduces boilerplate for common operations

### Stillwater Effect Integration

Effects enable lazy evaluation and composition of I/O operations:

```rust
// Effect: lazy computation that hasn't run yet
let effect = save_checkpoint(state);

// Compose effects
let composed = effect
    .and_then(|_| load_checkpoint(job_id))
    .map(|state| transform_state(state));

// Execute with environment
let result = composed.run(&env).await?;
```

**Benefits**:
- Lazy evaluation: Effects don't run until explicitly invoked
- Composable: Chain operations using `map`, `and_then`, `or_else`
- Testable: Use mock environments for testing
- Type-safe: Compile-time guarantees about effect types

### Testing Strategy

#### Pure Function Tests (40+ tests)
```rust
#[test]
fn test_apply_agent_result_success() {
    let state = test_state();
    let result = test_agent_result("item-0", AgentStatus::Success);

    let new_state = apply_agent_result(state, result);

    assert_eq!(new_state.successful_count, 1);
    assert!(new_state.pending_items.is_empty());
}
```

**No mocks required** - pure functions test input/output directly.

#### Effect-Based Tests (20+ tests)
```rust
#[tokio::test]
async fn test_save_checkpoint() {
    let env = test_env();  // Mock storage backend
    let state = test_state();

    save_checkpoint(state).run(&env).await.unwrap();

    // Verify through mock storage
}
```

Mock implementations provide testable I/O without actual file system access.

### Design Patterns

#### 1. Pure Core Pattern
```rust
// Pure logic in pure.rs
pub fn calculate_next_state(state: State, event: Event) -> State {
    // Pure transformation
}

// I/O wrapper in io.rs
pub fn apply_event_with_save(state: State, event: Event) -> StateEffect<State> {
    let new_state = pure::calculate_next_state(state, event);
    save_checkpoint(new_state.clone()).map(|_| new_state)
}
```

#### 2. Effect Composition
```rust
pub fn complete_batch(
    state: MapReduceJobState,
    results: Vec<AgentResult>
) -> StateEffect<MapReduceJobState> {
    // Pure: apply all results
    let mut new_state = state;
    for result in results {
        new_state = pure::apply_agent_result(new_state, result);
    }

    // I/O: save checkpoint
    save_checkpoint(new_state.clone()).and_then(move |_| {
        // Pure: check if transition needed
        if pure::should_transition_to_reduce(&new_state) {
            transition_to_reduce(new_state)
        } else {
            Effect::pure(new_state)
        }
    })
}
```

#### 3. Dependency Injection
```rust
pub struct StateEnv {
    pub storage: Arc<dyn StorageBackend>,
    pub event_log: Arc<dyn EventLog>,
}
```

Environment contains all external dependencies, injected at runtime.

### Performance Characteristics

- **No overhead**: Pure functions compile to native code with zero abstraction cost
- **Lazy evaluation**: Effects only run when executed
- **Memory efficient**: State updates use clone-on-write patterns
- **<5% overhead**: Compared to imperative state updates (target met)

### Integration with MapReduce Executor

The state_pure module integrates with the MapReduce coordination layer:

```rust
// Executor uses pure state functions
let new_state = pure::apply_agent_result(state, result);

// Or effect-based operations for persistence
let new_state = io::update_with_agent_result(state, result)
    .run(&env)
    .await?;
```

This enables the executor to choose between pure updates (for in-memory operations) and effect-based updates (for persistent checkpoints).

### Migration Guide

**Current Status**: Pure state module is implemented and tested but not yet integrated into the MapReduce executor. Integration will occur in a future update.

**Migration Steps** (planned):
1. Update MapReduce coordination to use `state_pure::pure` functions
2. Replace direct checkpoint I/O with `state_pure::io` effects
3. Run existing integration tests to verify equivalence
4. Gradually retire old state update code

### Benefits Achieved

1. **Testability**: 40+ pure function tests with no I/O mocking
2. **Separation of Concerns**: Clear boundary between logic and I/O
3. **Composability**: Effects combine using functional patterns
4. **Type Safety**: Compile-time guarantees about state transitions
5. **Maintainability**: Small, focused functions (all < 20 lines)

See spec 169 for complete implementation details and design rationale.

## Future Enhancements

1. **Distributed Execution**: Support for multi-machine orchestration
2. **Enhanced Monitoring**: Real-time metrics and dashboards
3. **Plugin System**: Extensible command and executor architecture
4. **Cloud Storage**: Optional cloud-based storage backends
5. **Advanced Scheduling**: Cron-based and event-driven workflows