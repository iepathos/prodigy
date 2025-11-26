---
number: 180
title: Replace Test Environment Manipulation with MockEnv
category: testing
priority: high
status: draft
dependencies: [178, 179]
created: 2025-11-25
---

# Specification 180: Replace Test Environment Manipulation with MockEnv

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: Spec 178, 179 (Premortem Integration)

## Context

Prodigy's test suite currently uses `std::env::set_var()` and `std::env::remove_var()` to manipulate environment variables during tests. This pattern has several serious problems:

### Current State

**Files with `std::env::set_var` usage:**
- `src/storage/config.rs` (lines 397, 398, 418, 437)
- `src/init/mod.rs` (line 952)
- `src/worktree/tracking_tests.rs` (lines 225, 235)
- `src/cook/orchestrator/execution_pipeline.rs` (lines 919, 920)
- `src/cook/orchestrator/argument_processing.rs` (line 172)
- `tests/merge_workflow_integration.rs` (line 198)
- `tests/mapreduce_env_execution_test.rs` (lines 195, 228)
- `tests/environment_workflow_test.rs` (line 139)

### Problems

1. **Unsafe in Rust 2024**: `std::env::set_var` is now `unsafe` because modifying environment variables is not thread-safe
2. **Requires `#[serial]`**: Tests must run serially, slowing down the test suite
3. **Cleanup on Panic**: If assertions fail before cleanup, env vars leak to other tests
4. **Global State Pollution**: Modified env vars can affect production code running concurrently
5. **Non-Deterministic**: Test behavior depends on order of execution

### Solution

Premortem's `MockEnv` provides a clean alternative:
- Local scope - changes only visible to the test
- No global state mutation - thread-safe
- Automatic cleanup via RAII
- Full parallelization support
- Zero-cost `RealEnv` in production

## Objective

Refactor all test code that manipulates environment variables to use premortem's `MockEnv`, eliminating unsafe patterns, enabling full test parallelization, and improving test reliability.

## Requirements

### Functional Requirements

#### FR1: Identify and Refactor All env::set_var Usage
- **MUST** audit all uses of `std::env::set_var` in test code
- **MUST** refactor each to use `MockEnv.with_env()`
- **MUST** remove all `std::env::remove_var` cleanup calls
- **MUST** remove all `#[serial]` attributes made unnecessary by refactor

#### FR2: Add ConfigEnv Parameter to Config Functions
- **MUST** add `_with<E: ConfigEnv>(env: &E)` variants to config loading functions
- **MUST** keep original functions that use `RealEnv` (zero-cost default)
- **MUST** apply to:
  - `StorageConfig::from_env()`
  - `EnvironmentManager::new()`
  - Workflow environment variable resolution
  - Any function that reads `PRODIGY_*` environment variables

#### FR3: Refactor Storage Config Tests
- **MUST** refactor `test_storage_config_from_env_file_type`
- **MUST** refactor `test_storage_config_from_env_memory_type`
- **MUST** refactor `test_storage_config_from_env_invalid_type`
- **MUST** remove `#[serial]` from these tests
- **MUST** verify tests can run in parallel

#### FR4: Refactor Worktree Tests
- **MUST** refactor `test_auto_merge_detection` (tracking_tests.rs)
- **MUST** mock `PRODIGY_AUTO_MERGE` and `PRODIGY_AUTO_CONFIRM`
- **MUST** ensure worktree operations use injected environment

#### FR5: Refactor Integration Tests
- **MUST** refactor `tests/merge_workflow_integration.rs`
- **MUST** refactor `tests/mapreduce_env_execution_test.rs`
- **MUST** refactor `tests/environment_workflow_test.rs`
- **MUST** ensure integration tests use `MockEnv` for env manipulation

#### FR6: Production Code env::set_var
- **MUST** audit production uses of `env::set_var`:
  - `execution_pipeline.rs` lines 919-920 (PRODIGY_AUTO_MERGE)
  - `argument_processing.rs` line 172 (PRODIGY_ARG)
- **MUST** refactor to pass values through context/config instead of global env
- **MUST** eliminate global env mutation from production code where possible

### Non-Functional Requirements

#### NFR1: Test Performance
- **MUST** enable full parallel test execution
- **SHOULD** reduce test suite runtime by removing serial constraints
- **MUST NOT** introduce new test flakiness

#### NFR2: Code Quality
- **MUST** eliminate all `#[allow(unsafe_code)]` related to env manipulation
- **MUST** pass `cargo clippy` without env-related warnings
- **SHOULD** improve code readability and test isolation

#### NFR3: Backward Compatibility
- **MUST** maintain public API compatibility
- **MUST** keep `RealEnv` as zero-cost default for production

## Acceptance Criteria

- [ ] All `std::env::set_var` calls in test code replaced with `MockEnv`
- [ ] All `std::env::remove_var` cleanup calls removed
- [ ] All `#[serial]` attributes removed from env-related tests
- [ ] `StorageConfig::from_env_with()` function added and used in tests
- [ ] Worktree tests refactored to use `MockEnv`
- [ ] All integration tests refactored
- [ ] Production `env::set_var` in `execution_pipeline.rs` eliminated
- [ ] Production `env::set_var` in `argument_processing.rs` eliminated
- [ ] `cargo test` passes with `--test-threads=N` (parallel)
- [ ] No `unsafe` blocks related to environment manipulation
- [ ] All existing tests continue to pass

## Technical Details

### Implementation Approach

#### Phase 1: Add ConfigEnv to Core Functions

```rust
// src/storage/config.rs

impl StorageConfig {
    /// Load configuration from environment (production)
    pub fn from_env() -> Result<Self> {
        Self::from_env_with(&RealEnv)
    }

    /// Load configuration from environment (testable)
    pub fn from_env_with<E: ConfigEnv>(env: &E) -> Result<Self> {
        let storage_type = env.var("PRODIGY_STORAGE_TYPE")
            .ok()
            .unwrap_or_else(|| "file".to_string());

        let base_path = env.var("PRODIGY_STORAGE_BASE_PATH")
            .ok()
            .map(PathBuf::from);

        // ... rest of logic unchanged
    }
}
```

#### Phase 2: Refactor Storage Config Tests

```rust
// Before (current)
#[test]
#[serial]  // Required due to global env mutation
fn test_storage_config_from_env_file_type() {
    env::set_var("PRODIGY_STORAGE_TYPE", "file");
    env::set_var("PRODIGY_STORAGE_BASE_PATH", "/custom/path");

    let config = StorageConfig::from_env().unwrap();

    assert_eq!(config.backend, BackendType::File);
    if let BackendConfig::File(file_config) = config.backend_config {
        assert_eq!(file_config.base_dir, PathBuf::from("/custom/path"));
    }

    // Manual cleanup (skipped if assertion fails!)
    env::remove_var("PRODIGY_STORAGE_TYPE");
    env::remove_var("PRODIGY_STORAGE_BASE_PATH");
}

// After (with MockEnv)
#[test]  // No #[serial] needed!
fn test_storage_config_from_env_file_type() {
    let env = MockEnv::new()
        .with_env("PRODIGY_STORAGE_TYPE", "file")
        .with_env("PRODIGY_STORAGE_BASE_PATH", "/custom/path");

    let config = StorageConfig::from_env_with(&env).unwrap();

    assert_eq!(config.backend, BackendType::File);
    if let BackendConfig::File(file_config) = config.backend_config {
        assert_eq!(file_config.base_dir, PathBuf::from("/custom/path"));
    }
    // No cleanup needed - MockEnv is dropped automatically
}
```

#### Phase 3: Refactor Worktree Tests

```rust
// src/worktree/tracking_tests.rs

// Before
#[test]
fn test_auto_merge_detection() {
    std::env::set_var("PRODIGY_AUTO_MERGE", "true");
    // ... test code
    std::env::remove_var("PRODIGY_AUTO_MERGE");
}

// After
#[test]
fn test_auto_merge_detection() {
    let env = MockEnv::new()
        .with_env("PRODIGY_AUTO_MERGE", "true")
        .with_env("PRODIGY_AUTO_CONFIRM", "true");

    // Pass env to function that checks auto-merge
    let result = should_auto_merge_with(&env);
    assert!(result);
}
```

#### Phase 4: Eliminate Production env::set_var

```rust
// src/cook/orchestrator/execution_pipeline.rs

// Before (sets global env)
if config.command.auto_accept {
    std::env::set_var("PRODIGY_AUTO_MERGE", "true");
    std::env::set_var("PRODIGY_AUTO_CONFIRM", "true");
}

// After (pass through execution context)
pub struct ExecutionContext {
    pub auto_merge: bool,
    pub auto_confirm: bool,
    // ... other context
}

// Pass context to MapReduce executor instead of global env
let context = ExecutionContext {
    auto_merge: config.command.auto_accept,
    auto_confirm: config.command.auto_accept,
    // ...
};
mapreduce_executor.run_with_context(context);
```

```rust
// src/cook/orchestrator/argument_processing.rs

// Before
std::env::set_var("PRODIGY_ARG", input);

// After - pass through workflow context
workflow_context.set_arg(input);
// Or use WorkflowEnv to carry variables
```

#### Phase 5: Refactor Integration Tests

```rust
// tests/mapreduce_env_execution_test.rs

#[test]
fn test_system_env_fallback() -> Result<()> {
    let env = MockEnv::new()
        .with_env("TEST_PRODIGY_MAX_PARALLEL", "7");

    let workflow_yaml = r#"
        name: test-system-env
        mode: mapreduce
        env:
          MAX_PARALLEL: ${TEST_PRODIGY_MAX_PARALLEL}
    "#;

    // Pass MockEnv to workflow executor
    let result = execute_workflow_with_env(workflow_yaml, &env)?;

    // Assertions...
    Ok(())
}
```

### File-by-File Migration Checklist

| File | Lines | Status |
|------|-------|--------|
| `src/storage/config.rs` | 397, 398, 418, 437 | Pending |
| `src/init/mod.rs` | 952 | Pending |
| `src/worktree/tracking_tests.rs` | 225, 235 | Pending |
| `src/cook/orchestrator/execution_pipeline.rs` | 919, 920 | Pending |
| `src/cook/orchestrator/argument_processing.rs` | 172 | Pending |
| `tests/merge_workflow_integration.rs` | 198 | Pending |
| `tests/mapreduce_env_execution_test.rs` | 195, 228 | Pending |
| `tests/environment_workflow_test.rs` | 139 | Pending |

### Pattern for Testable Functions

```rust
// Standard pattern for all env-dependent code

pub trait EnvReader {
    fn var(&self, name: &str) -> Result<String, std::env::VarError>;
    fn var_os(&self, name: &str) -> Option<OsString>;
}

// Use premortem's ConfigEnv which provides this
use premortem::{ConfigEnv, RealEnv, MockEnv};

// Public API - uses real environment
pub fn load_thing() -> Result<Thing> {
    load_thing_with(&RealEnv)
}

// Testable API - accepts any environment
pub fn load_thing_with<E: ConfigEnv>(env: &E) -> Result<Thing> {
    let value = env.var("THING_CONFIG")?;
    // ... pure logic
}

// Tests
#[test]
fn test_load_thing() {
    let env = MockEnv::new().with_env("THING_CONFIG", "test-value");
    let thing = load_thing_with(&env).unwrap();
    assert_eq!(thing.config, "test-value");
}
```

## Dependencies

- **Prerequisites**: Spec 178, 179 (Premortem foundation and migration)
- **Affected Components**:
  - `src/storage/config.rs` - Add `_with` variants
  - `src/init/mod.rs` - Refactor test
  - `src/worktree/` - Add env injection
  - `src/cook/orchestrator/` - Eliminate global env mutation
  - All `tests/` integration tests
- **External Dependencies**: `premortem` crate with `MockEnv`

## Testing Strategy

### Verification of Parallelization
```bash
# Before: some tests fail with parallelization
cargo test --test-threads=8  # May have race conditions

# After: all tests pass in parallel
cargo test --test-threads=8  # Should be fully deterministic
```

### Test Isolation Verification
```rust
#[test]
fn test_isolation_a() {
    let env = MockEnv::new().with_env("SHARED_VAR", "value_a");
    let config = load_with(&env).unwrap();
    assert_eq!(config.shared_var, "value_a");
}

#[test]
fn test_isolation_b() {
    // Runs in parallel - should NOT see value_a
    let env = MockEnv::new().with_env("SHARED_VAR", "value_b");
    let config = load_with(&env).unwrap();
    assert_eq!(config.shared_var, "value_b");
}
```

### No Leakage Test
```rust
#[test]
fn test_env_does_not_leak() {
    // Real env should not have test values
    assert!(std::env::var("TEST_ONLY_VAR").is_err());

    {
        let _env = MockEnv::new().with_env("TEST_ONLY_VAR", "secret");
        // MockEnv does not modify real environment
    }

    // Still not present after MockEnv dropped
    assert!(std::env::var("TEST_ONLY_VAR").is_err());
}
```

## Documentation Requirements

- **Code Documentation**: Document `_with` function variants
- **Testing Guide**: Add section on using `MockEnv` for tests
- **Migration Notes**: Document pattern for existing test code

## Implementation Notes

1. **Start with Storage**: `src/storage/config.rs` is self-contained and good first target
2. **Production Code Last**: Refactor tests first, production env::set_var changes are more invasive
3. **Preserve Behavior**: All tests should produce same results after migration
4. **Remove serial crate**: Once all `#[serial]` attributes removed, remove `serial_test` dependency if unused

## Migration and Compatibility

- **Breaking Changes**: None for public APIs
- **Internal Changes**: Functions get `_with` variants
- **Test Changes**: Tests refactored but behavior unchanged
- **Dependency**: May remove `serial_test` crate if no longer needed
