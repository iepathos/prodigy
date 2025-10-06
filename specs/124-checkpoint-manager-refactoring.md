---
number: 124
title: CheckpointManager Functional Refactoring
category: optimization
priority: high
status: draft
dependencies: [122, 123]
created: 2025-10-06
---

# Specification 124: CheckpointManager Functional Refactoring

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: Spec 122 (Checkpoint Path Resolution), Spec 123 (Checkpoint-on-Error)

## Context

The current `CheckpointManager` implementation in `src/cook/workflow/checkpoint.rs` violates several functional programming principles:

1. **Mutable Configuration**: Uses `configure(&mut self)` to modify state after construction
2. **Opaque Path Handling**: Uses raw `PathBuf` without explicit storage strategy
3. **Mixed Concerns**: Combines path resolution, I/O, and business logic
4. **Implicit State**: Enabled/disabled state can change after construction

**Current API** (problematic):
```rust
let mut manager = CheckpointManager::new(storage_path);
manager.configure(interval, enabled); // Mutation after construction
```

This makes it difficult to reason about checkpoint behavior and violates Rust idioms favoring immutable configuration.

## Objective

Refactor `CheckpointManager` to follow functional programming principles:
- Immutable after construction (no `&mut self` configuration methods)
- Explicit storage strategy using `CheckpointStorage` enum from Spec 122
- Clear separation between pure logic and I/O operations
- Composition over mutation for configuration

The refactored API should be:
```rust
let manager = CheckpointManager::with_storage(storage)
    .with_interval(interval)
    .with_enabled(true);
```

## Requirements

### Functional Requirements

1. **Immutable Configuration**: Remove mutable `configure()` method
   - Replace with builder pattern for configuration
   - All configuration happens during construction
   - `CheckpointManager` is immutable after construction

2. **Explicit Storage Strategy**: Replace `PathBuf` with `CheckpointStorage`
   - Use `CheckpointStorage` enum from Spec 122
   - Remove raw path handling from public API
   - Make storage location explicit and type-safe

3. **Pure Function Separation**: Extract pure logic from I/O operations
   - `serialize_checkpoint()` - pure function for JSON serialization
   - `write_checkpoint_atomically()` - separate I/O function
   - Path resolution delegated to `CheckpointStorage`

4. **Composition API**: Use builder pattern for optional configuration
   - `with_interval(Duration)` - set checkpoint interval
   - `with_enabled(bool)` - enable/disable checkpointing
   - Chainable methods returning `Self`

### Non-Functional Requirements

1. **Type Safety**: Use Rust's type system to prevent invalid configurations
2. **Backwards Compatibility**: Provide migration path from old API
3. **Testability**: All pure functions testable without I/O
4. **Clarity**: Make checkpoint configuration obvious and explicit
5. **Performance**: No overhead from immutability (zero-cost abstractions)

## Acceptance Criteria

- [ ] `CheckpointManager` fields are not `pub` or `pub(crate)`
- [ ] No `configure(&mut self)` method exists
- [ ] Builder pattern methods (`with_*`) return `Self` for chaining
- [ ] `CheckpointManager::with_storage(CheckpointStorage)` is primary constructor
- [ ] `serialize_checkpoint()` exists as pure function
- [ ] `write_checkpoint_atomically()` exists as separate I/O function
- [ ] All configuration happens during construction
- [ ] Unit tests verify immutability (no mutation after construction)
- [ ] Integration tests verify builder pattern works correctly
- [ ] No panics in any CheckpointManager methods (Spec 101 compliance)
- [ ] Documentation explains builder pattern usage

## Technical Details

### Implementation Approach

#### 1. Refactored CheckpointManager Structure

```rust
/// Immutable checkpoint manager with explicit storage strategy
pub struct CheckpointManager {
    /// Immutable storage configuration
    storage: CheckpointStorage,
    /// Checkpoint interval (immutable)
    checkpoint_interval: Duration,
    /// Whether checkpointing is enabled (immutable)
    enabled: bool,
}

impl CheckpointManager {
    /// Create checkpoint manager with explicit storage strategy
    pub fn with_storage(storage: CheckpointStorage) -> Self {
        Self {
            storage,
            checkpoint_interval: DEFAULT_CHECKPOINT_INTERVAL,
            enabled: true,
        }
    }

    /// Configure checkpoint interval (builder pattern)
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.checkpoint_interval = interval;
        self
    }

    /// Enable or disable checkpointing (builder pattern)
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Get checkpoint path using storage strategy
    fn get_checkpoint_path(&self, checkpoint_id: &str) -> Result<PathBuf> {
        self.storage.checkpoint_file_path(checkpoint_id)
    }
}
```

#### 2. Pure Function Extraction

```rust
/// Pure function: serialize checkpoint to JSON
fn serialize_checkpoint(checkpoint: &WorkflowCheckpoint) -> Result<String> {
    serde_json::to_string_pretty(checkpoint)
        .context("Failed to serialize checkpoint")
}

/// I/O operation: atomic write to filesystem
async fn write_checkpoint_atomically(
    final_path: &Path,
    temp_path: &Path,
    checkpoint: &WorkflowCheckpoint,
) -> Result<()> {
    // Serialize (pure)
    let json = serialize_checkpoint(checkpoint)?;

    // Write temp file (I/O)
    fs::write(temp_path, json)
        .await
        .context("Failed to write checkpoint to temp file")?;

    // Atomic rename (I/O)
    fs::rename(temp_path, final_path)
        .await
        .context("Failed to move checkpoint to final location")
}

/// I/O operation: create parent directories
async fn ensure_checkpoint_dir_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .context("Failed to create checkpoint directory")?;
    }
    Ok(())
}
```

#### 3. Refactored Save Method

```rust
impl CheckpointManager {
    /// Save checkpoint to disk (I/O at boundary)
    pub async fn save_checkpoint(&self, checkpoint: &WorkflowCheckpoint) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Pure: resolve paths
        let checkpoint_path = self.get_checkpoint_path(&checkpoint.workflow_id)?;
        let temp_path = checkpoint_path.with_extension("tmp");

        // I/O: ensure directory exists
        ensure_checkpoint_dir_exists(&checkpoint_path).await?;

        // I/O: atomic write
        write_checkpoint_atomically(&checkpoint_path, &temp_path, checkpoint).await?;

        tracing::info!(
            "Saved checkpoint for workflow {} at step {}",
            checkpoint.workflow_id,
            checkpoint.execution_state.current_step_index
        );

        Ok(())
    }
}
```

#### 4. Deprecation and Migration

```rust
impl CheckpointManager {
    /// DEPRECATED: Use `with_storage()` instead
    #[deprecated(
        since = "0.10.0",
        note = "Use `CheckpointManager::with_storage(CheckpointStorage)` instead"
    )]
    pub fn new(storage_path: PathBuf) -> Self {
        Self::with_storage(CheckpointStorage::Local(storage_path))
    }

    /// DEPRECATED: Use builder pattern instead
    #[deprecated(
        since = "0.10.0",
        note = "Use `.with_interval().with_enabled()` builder pattern instead"
    )]
    pub fn configure(&mut self, interval: Duration, enabled: bool) {
        // This violates immutability but kept for backwards compatibility
        self.checkpoint_interval = interval;
        self.enabled = enabled;
    }
}
```

### Architecture Changes

- **Modified**: `src/cook/workflow/checkpoint.rs`
  - Refactor `CheckpointManager` to use `CheckpointStorage`
  - Extract pure functions for serialization
  - Separate I/O operations
  - Add builder pattern methods

- **Affected Components**:
  - All code creating `CheckpointManager` instances
  - Tests using old `new()` + `configure()` API

### Functional Programming Principles

1. **Immutability**: No mutation after construction
2. **Pure Functions**: `serialize_checkpoint` has no side effects
3. **I/O at Boundaries**: Separate pure logic from filesystem operations
4. **Composition**: Builder pattern composes configuration
5. **Explicit Configuration**: Storage strategy always explicit

## Dependencies

- **Prerequisites**:
  - Spec 122 (Checkpoint Path Resolution) - provides `CheckpointStorage` type
  - Spec 101 (Error Handling Guidelines) - no panics in production code

- **Affected Components**:
  - `src/cook/workflow/checkpoint.rs` - main refactoring target
  - `src/cook/orchestrator.rs` - update to use new API
  - `src/cli/commands/resume.rs` - update to use new API
  - All tests using `CheckpointManager`

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_checkpoint_manager_immutability() {
    let storage = CheckpointStorage::Session {
        session_id: "test-123".to_string(),
    };

    let manager = CheckpointManager::with_storage(storage)
        .with_interval(Duration::from_secs(30))
        .with_enabled(true);

    // Manager should be immutable - this test verifies construction
    assert!(manager.enabled);
}

#[test]
fn test_builder_pattern_chaining() {
    let storage = CheckpointStorage::Local(PathBuf::from("/tmp"));

    let manager = CheckpointManager::with_storage(storage)
        .with_interval(Duration::from_secs(60))
        .with_enabled(false);

    // Verify configuration
    assert!(!manager.enabled);
}

#[test]
fn test_serialize_checkpoint_is_pure() {
    let checkpoint = create_test_checkpoint();

    // Calling multiple times should produce same result
    let json1 = serialize_checkpoint(&checkpoint).unwrap();
    let json2 = serialize_checkpoint(&checkpoint).unwrap();

    assert_eq!(json1, json2, "Pure function must be deterministic");
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_checkpoint_save_with_new_api() {
    let temp_dir = TempDir::new().unwrap();
    let storage = CheckpointStorage::Local(temp_dir.path().to_path_buf());

    let manager = CheckpointManager::with_storage(storage)
        .with_enabled(true);

    let checkpoint = create_test_checkpoint();
    manager.save_checkpoint(&checkpoint).await.unwrap();

    // Verify file exists
    let checkpoint_path = temp_dir.path().join("test-workflow.checkpoint.json");
    assert!(checkpoint_path.exists());
}
```

### Migration Tests

Verify deprecated API still works:
```rust
#[test]
#[allow(deprecated)]
fn test_deprecated_api_backwards_compatibility() {
    let mut manager = CheckpointManager::new(PathBuf::from("/tmp"));
    manager.configure(Duration::from_secs(30), true);

    // Old API should still work (with deprecation warning)
    assert!(manager.enabled);
}
```

## Documentation Requirements

### Code Documentation

- Document builder pattern usage with examples
- Mark deprecated methods with clear migration instructions
- Explain immutability guarantees
- Document pure functions clearly

Example:
```rust
/// Create a checkpoint manager with session-scoped storage.
///
/// # Example
///
/// ```
/// use prodigy::cook::workflow::checkpoint::{CheckpointManager, CheckpointStorage};
/// use std::time::Duration;
///
/// let storage = CheckpointStorage::Session {
///     session_id: "session-123".to_string(),
/// };
///
/// let manager = CheckpointManager::with_storage(storage)
///     .with_interval(Duration::from_secs(60))
///     .with_enabled(true);
/// ```
pub fn with_storage(storage: CheckpointStorage) -> Self { ... }
```

### User Documentation

- Not user-facing (internal API)

### Architecture Updates

Update `ARCHITECTURE.md`:
- Document immutable `CheckpointManager` design
- Explain builder pattern rationale
- Show pure function separation pattern

## Implementation Notes

### Builder Pattern Best Practices

1. **Make builder methods consume `self`**: Forces immutability
2. **Return `Self` for chaining**: Enables fluent API
3. **Provide sensible defaults**: `with_storage()` sets reasonable defaults
4. **Make required config explicit**: Storage must be provided, interval has default

### Pure Function Guidelines

1. **No I/O in pure functions**: `serialize_checkpoint` only transforms data
2. **Deterministic output**: Same input always produces same output
3. **No side effects**: No logging, no mutation, no hidden state
4. **Testable in isolation**: Can test without mocking

### Migration Strategy

1. **Phase 1**: Add new API alongside old API
2. **Phase 2**: Deprecate old API with warnings
3. **Phase 3**: Migrate internal code to new API
4. **Phase 4**: Remove deprecated API in next major version

## Migration and Compatibility

### Breaking Changes

- `CheckpointManager::configure(&mut self)` deprecated
- `CheckpointManager::new(PathBuf)` deprecated

### Migration Guide

**Old API**:
```rust
let mut manager = CheckpointManager::new(path);
manager.configure(Duration::from_secs(60), true);
```

**New API**:
```rust
let manager = CheckpointManager::with_storage(
    CheckpointStorage::Session { session_id: "session-123".to_string() }
)
.with_interval(Duration::from_secs(60))
.with_enabled(true);
```

### Deprecation Timeline

- **v0.10.0**: Add new API, deprecate old API
- **v0.11.0**: Remove deprecated API

## Success Metrics

- Zero uses of deprecated `configure()` method in codebase
- All checkpoint managers use `CheckpointStorage` explicitly
- 100% of pure functions are deterministic (verified by property tests)
- No mutation of `CheckpointManager` after construction
- Clear compiler warnings guide migration
