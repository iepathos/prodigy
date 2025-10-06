---
number: 122
title: Checkpoint Path Resolution System
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-10-06
---

# Specification 122: Checkpoint Path Resolution System

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

Prodigy's checkpoint/resume system suffers from a critical path mismatch issue. Checkpoints are saved to local `.prodigy/checkpoints/` directories during `prodigy run`, but `prodigy resume` attempts to load them from global `~/.prodigy/state/{session_id}/checkpoints/` directories. This inconsistency results in "No checkpoint files found" errors, making the resume functionality completely non-functional.

The current implementation in `src/cook/workflow/checkpoint.rs` uses an opaque `storage_path: PathBuf` field with no clear strategy for determining where checkpoints should be saved or loaded from. Different parts of the system construct `CheckpointManager` instances with different paths, leading to save/load asymmetry.

**Root Cause**: The `CheckpointManager::new(storage_path: PathBuf)` API provides no guidance on which path should be used, and callers pass inconsistent values:
- Workflow executor: passes local `.prodigy/checkpoints`
- Resume command: looks in `~/.prodigy/state/{session_id}/checkpoints`

## Objective

Create a type-safe, pure function-based checkpoint path resolution system that ensures symmetric save/load paths across all checkpoint operations. The system must make storage strategy explicit, prevent path mismatches, and follow functional programming principles by separating pure path resolution logic from I/O operations.

## Requirements

### Functional Requirements

1. **Storage Strategy Type**: Create an enum that explicitly represents different checkpoint storage strategies
   - Local project storage (`.prodigy/checkpoints/`)
   - Global repository-scoped storage (`~/.prodigy/state/{repo_name}/checkpoints/`)
   - Session-scoped storage (`~/.prodigy/state/{session_id}/checkpoints/`)

2. **Pure Path Resolution**: Implement pure functions that resolve checkpoint paths deterministically
   - Given a storage strategy and checkpoint ID, always return the same path
   - No side effects or I/O during path resolution
   - Testable with simple assertions

3. **Symmetric Save/Load**: Ensure that save and load operations use identical path resolution logic
   - Same storage strategy → same base directory
   - Same checkpoint ID → same file path
   - No hidden state or global variables

4. **Error Handling**: Provide clear error messages when paths cannot be resolved
   - Report missing home directory
   - Report invalid session IDs
   - Use `Result<PathBuf>` for fallible operations

### Non-Functional Requirements

1. **Type Safety**: Use Rust's type system to prevent invalid storage configurations at compile time
2. **Testability**: All path resolution must be testable without filesystem I/O
3. **Clarity**: Code should make storage strategy explicit and obvious
4. **Performance**: Path resolution should be O(1) with minimal allocations
5. **Functional Purity**: Separate pure logic (path construction) from I/O (directory creation)

## Acceptance Criteria

- [ ] `CheckpointStorage` enum exists with Local, Global, and Session variants
- [ ] `resolve_base_dir()` pure function returns consistent paths for each variant
- [ ] `checkpoint_file_path()` combines base dir with checkpoint ID correctly
- [ ] Unit tests verify path resolution for all storage variants
- [ ] Unit tests verify that same strategy + ID always produces same path
- [ ] Error cases (no home dir) return proper `Result::Err` with context
- [ ] No `unwrap()` or `panic!()` in path resolution code (production code requirement from Spec 101)
- [ ] All functions are pure (no hidden state, no I/O, no side effects)
- [ ] Documentation explains when to use each storage strategy
- [ ] Property-based tests verify path resolution invariants

## Technical Details

### Implementation Approach

Create a new module `src/cook/workflow/checkpoint_path.rs` with:

1. **CheckpointStorage enum**:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CheckpointStorage {
    /// Local project storage (.prodigy/checkpoints/)
    Local(PathBuf),

    /// Global repository-scoped storage (~/.prodigy/state/{repo}/checkpoints/)
    Global { repo_name: String },

    /// Session-scoped storage (~/.prodigy/state/{session_id}/checkpoints/)
    Session { session_id: String },
}
```

2. **Pure path resolution functions**:
```rust
impl CheckpointStorage {
    /// Pure function: resolve base directory for checkpoint storage
    pub fn resolve_base_dir(&self) -> Result<PathBuf> {
        match self {
            Self::Local(path) => Ok(path.clone()),
            Self::Global { repo_name } => {
                resolve_global_base_dir()?
                    .join("state")
                    .join(repo_name)
                    .join("checkpoints")
                    .pipe(Ok)
            }
            Self::Session { session_id } => {
                resolve_global_base_dir()?
                    .join("state")
                    .join(session_id)
                    .join("checkpoints")
                    .pipe(Ok)
            }
        }
    }

    /// Pure function: construct file path for specific checkpoint
    pub fn checkpoint_file_path(&self, checkpoint_id: &str) -> Result<PathBuf> {
        self.resolve_base_dir()?
            .join(format!("{}.checkpoint.json", checkpoint_id))
            .pipe(Ok)
    }
}

/// Pure function: get global Prodigy storage directory
fn resolve_global_base_dir() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .context("Could not determine home directory")?
        .home_dir()
        .join(".prodigy")
        .pipe(Ok)
}
```

3. **Strategy Selection Guidelines**:
   - Use **Session** storage for workflow checkpoints (default for most workflows)
   - Use **Global** storage for repository-level metadata
   - Use **Local** storage for backwards compatibility or testing

### Architecture Changes

- **New module**: `src/cook/workflow/checkpoint_path.rs`
- **Modified**: `CheckpointManager` will use `CheckpointStorage` instead of raw `PathBuf`
- **Affected**: All code that creates `CheckpointManager` instances must specify strategy

### Data Structures

```rust
// Explicit storage strategy with type-safe variants
pub enum CheckpointStorage {
    Local(PathBuf),
    Global { repo_name: String },
    Session { session_id: String },
}

// Pure function return types
impl CheckpointStorage {
    pub fn resolve_base_dir(&self) -> Result<PathBuf>
    pub fn checkpoint_file_path(&self, checkpoint_id: &str) -> Result<PathBuf>
}
```

### Functional Programming Principles

1. **Pure Functions**: All path resolution functions are deterministic and side-effect free
2. **Explicit Configuration**: Storage strategy is always explicit, never inferred
3. **Immutability**: `CheckpointStorage` enum is immutable once constructed
4. **Error as Values**: Use `Result<T>` instead of panicking
5. **Composition**: Small pure functions compose to build complex paths

## Dependencies

- **Prerequisites**: Spec 101 (Error Handling Guidelines) for production error handling
- **Affected Components**:
  - `src/cook/workflow/checkpoint.rs` - CheckpointManager will use new types
  - `src/cli/commands/resume.rs` - Resume command will use Session storage
  - `src/cook/orchestrator.rs` - Workflow execution will use Session storage
- **External Dependencies**:
  - `directories` crate (already in use)
  - `anyhow` crate (already in use)

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_session_storage_path_resolution() {
    let storage = CheckpointStorage::Session {
        session_id: "test-session-123".to_string(),
    };

    let base = storage.resolve_base_dir().unwrap();
    assert!(base.ends_with(".prodigy/state/test-session-123/checkpoints"));

    let file = storage.checkpoint_file_path("checkpoint-1").unwrap();
    assert!(file.ends_with("checkpoint-1.checkpoint.json"));
}

#[test]
fn test_path_resolution_is_deterministic() {
    let storage = CheckpointStorage::Session {
        session_id: "session-abc".to_string(),
    };

    let path1 = storage.checkpoint_file_path("test").unwrap();
    let path2 = storage.checkpoint_file_path("test").unwrap();

    assert_eq!(path1, path2, "Same inputs must produce same path");
}

#[test]
fn test_local_storage_uses_provided_path() {
    let custom_path = PathBuf::from("/tmp/checkpoints");
    let storage = CheckpointStorage::Local(custom_path.clone());

    let base = storage.resolve_base_dir().unwrap();
    assert_eq!(base, custom_path);
}
```

### Property-Based Tests

```rust
#[quickcheck]
fn prop_same_strategy_same_path(session_id: String, checkpoint_id: String) -> bool {
    let storage1 = CheckpointStorage::Session { session_id: session_id.clone() };
    let storage2 = CheckpointStorage::Session { session_id };

    let path1 = storage1.checkpoint_file_path(&checkpoint_id);
    let path2 = storage2.checkpoint_file_path(&checkpoint_id);

    path1 == path2
}
```

### Integration Tests

- Verify save and load use same paths when given same storage strategy
- Test that checkpoints written with Session storage are found by resume command
- Validate error handling when home directory cannot be determined

## Documentation Requirements

### Code Documentation

- Comprehensive module-level docs explaining storage strategies
- Doc examples showing when to use each storage variant
- Function-level documentation with examples
- Document invariants and guarantees of pure functions

### User Documentation

- Not user-facing (internal API)

### Architecture Updates

Update `ARCHITECTURE.md` to document:
- Checkpoint storage strategy system
- Pure function approach to path resolution
- Storage strategy selection guidelines

## Implementation Notes

### Functional Programming Best Practices

1. **Keep functions pure**: No I/O, no side effects in path resolution
2. **Separate I/O**: Directory creation happens separately in `CheckpointManager`
3. **Use Result for errors**: Never panic in path resolution
4. **Make strategy explicit**: Force callers to choose storage strategy consciously

### Common Pitfalls

- **Don't** mix storage strategies for same session
- **Don't** perform I/O inside path resolution functions
- **Don't** cache or memoize paths (path is cheap to recompute)
- **Do** use Session storage for normal workflows
- **Do** test path resolution separately from I/O

### Code Organization

```
src/cook/workflow/
├── checkpoint_path.rs       # New: Pure path resolution
├── checkpoint.rs            # Modified: Use CheckpointStorage
└── checkpoint_tests.rs      # Modified: Test new path logic
```

## Migration and Compatibility

### Breaking Changes

The `CheckpointManager::new(PathBuf)` constructor will be deprecated in favor of:
```rust
CheckpointManager::with_storage(CheckpointStorage)
```

### Migration Path

1. Identify all `CheckpointManager::new()` call sites
2. Determine appropriate storage strategy for each
3. Replace with `with_storage()` using correct strategy
4. Run tests to verify checkpoint save/load symmetry

### Backwards Compatibility

- Keep `CheckpointManager::new()` temporarily with deprecation warning
- Implement fallback to Session storage for existing workflows
- Migration can be gradual (code continues to work)

### Migration Example

**Before**:
```rust
let manager = CheckpointManager::new(project_dir.join(".prodigy/checkpoints"));
```

**After**:
```rust
let storage = CheckpointStorage::Session {
    session_id: session_id.to_string()
};
let manager = CheckpointManager::with_storage(storage);
```

## Success Metrics

- Zero "No checkpoint files found" errors in integration tests
- 100% symmetry between save and load paths in property tests
- All path resolution functions marked as pure (no I/O)
- No panics in path resolution code (enforced by tests)
- Clear compilation errors when storage strategy not specified
