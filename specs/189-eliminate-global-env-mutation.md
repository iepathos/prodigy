---
number: 189
title: Eliminate Global Environment Mutation
category: testing
priority: high
status: draft
dependencies: [108]
created: 2025-11-28
---

# Specification 189: Eliminate Global Environment Mutation

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: [108 - Increase Functional Programming Adoption]

## Context

The codebase currently uses `std::env::set_var` and `std::env::remove_var` in both production code and tests. This violates Stillwater's "pure core, imperative shell" philosophy:

1. **Global mutable state** - Environment variables are process-global, making code impure
2. **Thread safety** - `std::env::set_var` is not thread-safe; tests cannot run in parallel safely
3. **Non-determinism** - Test order can affect results when tests mutate shared state
4. **Testability** - Code that reads `std::env::var` directly is hard to test without global mutation

Stillwater provides `MockEnv` for composable, thread-safe environment testing. The philosophy states:
- Pure functions should not depend on global state
- Effects (I/O, environment access) should flow through the effect system
- Tests should be deterministic and parallelizable

### Current State Analysis

**Production Code (1 file)**:
- `src/cook/orchestrator/argument_processing.rs` - Sets `PRODIGY_ARG` globally during workflow execution

**Test Code (10 files, ~40+ usages)**:
| File | Variables | Count |
|------|-----------|-------|
| `src/cook/workflow/executor_tests.rs` | `PRODIGY_TEST_MODE`, `PRODIGY_CLAUDE_STREAMING` | 9 |
| `src/cook/workflow/variable_checkpoint_tests.rs` | `TEST_VAR` | 4 |
| `src/cook/environment/path_resolver.rs` | `HOME`, `TEST_VAR`, `TEST_PATH`, `PROJECT` | 5 |
| `src/cook/environment/secret_store.rs` | `TEST_SECRET`, `CACHED_SECRET`, `DEFAULT_SECRET` | 4 |
| `src/cook/execution/variables.rs` | `TEST_VAR`, `TEST_SPECIAL_VAR` | 4 |
| `src/cook/execution/variables_test.rs` | `TEST_PRODIGY_VAR`, `USER` | 3 |
| `src/cook/input/tests.rs` | `PRODIGY_AUTOMATION`, `TEST_VAR_*`, `APP_*` | 8 |
| `src/subprocess/tests.rs` | `PRODIGY_TEST_BLOATED_VAR` | 2 |

## Objective

Eliminate all usage of `std::env::set_var` and `std::env::remove_var` by:
1. Refactoring production code to pass configuration through context/effects
2. Migrating tests to use `stillwater::MockEnv` for environment injection
3. Marking remaining tests that require true global env mutation with `#[serial]`

## Requirements

### Functional Requirements

1. **Environment Reader Trait**
   - Create `EnvReader` trait for abstracting environment variable access
   - Implement `RealEnvReader` for production using `std::env::var`
   - Use `stillwater::MockEnv` for testing

2. **Production Code Refactoring**
   - Remove global `PRODIGY_ARG` setting in argument_processing.rs
   - Pass argument value through `ExecutionContext` or workflow configuration
   - Ensure variable substitution works via context, not global env

3. **Test Migration**
   - Convert all tests using `std::env::set_var` to use `MockEnv`
   - Inject `EnvReader` into components that need environment access
   - Use builder pattern for test environment setup

4. **Exception Handling**
   - Tests that verify subprocess environment inheritance may need actual global env
   - Mark these tests with `#[serial]` from `serial_test` crate
   - Document why global mutation is necessary

### Non-Functional Requirements

- All tests must pass with the new approach
- Tests must be able to run in parallel (except `#[serial]` tests)
- No performance regression in production code
- Clear documentation on environment access patterns

## Acceptance Criteria

- [ ] Zero uses of `std::env::set_var` in production code
- [ ] Zero uses of `std::env::remove_var` in production code
- [ ] `EnvReader` trait created and implemented
- [ ] All env-dependent production code accepts `EnvReader`
- [ ] `argument_processing.rs` passes `PRODIGY_ARG` through context
- [ ] All test files migrated to `MockEnv` or marked `#[serial]`
- [ ] Tests run successfully in parallel with `cargo nextest`
- [ ] Documentation updated with environment access patterns
- [ ] Clippy passes with no warnings

## Technical Details

### Implementation Approach

#### Phase 1: Create Environment Abstraction

```rust
// src/cook/environment/env_reader.rs

/// Trait for reading environment variables
pub trait EnvReader: Send + Sync {
    fn var(&self, key: &str) -> Result<String, std::env::VarError>;
    fn var_os(&self, key: &str) -> Option<std::ffi::OsString>;
}

/// Production implementation using std::env
#[derive(Clone, Default)]
pub struct RealEnvReader;

impl EnvReader for RealEnvReader {
    fn var(&self, key: &str) -> Result<String, std::env::VarError> {
        std::env::var(key)
    }

    fn var_os(&self, key: &str) -> Option<std::ffi::OsString> {
        std::env::var_os(key)
    }
}

// For testing, use stillwater::MockEnv which implements similar interface
```

#### Phase 2: Refactor Argument Processing

**Before**:
```rust
// Sets global env - BAD
std::env::set_var("PRODIGY_ARG", input);
let result = executor.execute(&extended_workflow, env).await;
std::env::remove_var("PRODIGY_ARG");
```

**After**:
```rust
// Pass through context - GOOD
let context = WorkflowContext::new()
    .with_variable("ARG", input.clone())
    .with_variable("PRODIGY_ARG", input);

let result = executor
    .with_context(context)
    .execute(&extended_workflow, env)
    .await;
```

#### Phase 3: Migrate Tests

**Before**:
```rust
#[tokio::test]
async fn test_env_secret_provider() {
    std::env::set_var("TEST_SECRET", "secret_value");

    let provider = EnvSecretProvider;
    let value = provider.get_secret("TEST_SECRET").await.unwrap();
    assert_eq!(value, "secret_value");
}
```

**After**:
```rust
#[tokio::test]
async fn test_env_secret_provider() {
    use stillwater::MockEnv;

    let env = MockEnv::new()
        .with_env("TEST_SECRET", "secret_value");

    let provider = EnvSecretProvider::with_env_reader(env);
    let value = provider.get_secret("TEST_SECRET").await.unwrap();
    assert_eq!(value, "secret_value");
}
```

#### Phase 4: Handle Special Cases

For `src/subprocess/tests.rs` which tests subprocess isolation:

```rust
use serial_test::serial;

#[tokio::test]
#[serial]  // Must run alone - modifies global env
async fn test_environment_not_inherited_from_parent() {
    // This test intentionally sets global env to verify subprocess doesn't inherit
    std::env::set_var("PRODIGY_TEST_BLOATED_VAR", "x".repeat(10000));

    // ... test subprocess behavior ...

    std::env::remove_var("PRODIGY_TEST_BLOATED_VAR");
}
```

### Files to Modify

**New Files**:
- `src/cook/environment/env_reader.rs` - EnvReader trait and RealEnvReader

**Production Code**:
- `src/cook/orchestrator/argument_processing.rs` - Remove global env mutation
- `src/cook/environment/path_resolver.rs` - Accept EnvReader
- `src/cook/environment/secret_store.rs` - Accept EnvReader
- `src/cook/execution/variables.rs` - Accept EnvReader for env.* interpolation

**Test Files**:
- `src/cook/workflow/executor_tests.rs`
- `src/cook/workflow/variable_checkpoint_tests.rs`
- `src/cook/environment/path_resolver.rs` (tests module)
- `src/cook/environment/secret_store.rs` (tests module)
- `src/cook/execution/variables.rs` (tests module)
- `src/cook/execution/variables_test.rs`
- `src/cook/input/tests.rs`
- `src/subprocess/tests.rs`

### Architecture Changes

```
Before:
┌─────────────────┐     ┌─────────────────────┐
│   Component     │────▶│  std::env::var()    │ (global, impure)
└─────────────────┘     └─────────────────────┘

After:
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────────┐
│   Component     │────▶│   EnvReader     │────▶│  std::env::var()    │
└─────────────────┘     │    (trait)      │     │   (production)      │
                        └─────────────────┘     └─────────────────────┘
                                │
                        ┌───────┴───────┐
                        ▼               ▼
               ┌─────────────┐  ┌─────────────────┐
               │ RealEnvReader│  │ MockEnv (test)  │
               └─────────────┘  └─────────────────┘
```

## Dependencies

- **Prerequisites**:
  - Spec 108 (Functional Programming Adoption) - provides context for pure/impure separation
  - `stillwater` crate with `MockEnv` support

- **Affected Components**:
  - Workflow execution system
  - Variable interpolation
  - Secret store
  - Path resolver
  - Input processing

- **External Dependencies**:
  - `serial_test` crate for `#[serial]` attribute (if not already present)
  - `stillwater::MockEnv` for test environment

## Testing Strategy

### Unit Tests
- Test `EnvReader` trait implementations
- Test components with mocked environment
- Verify MockEnv properly isolates tests

### Integration Tests
- Verify workflow execution with context-based variables
- Test variable substitution without global env mutation
- Verify subprocess isolation tests work with `#[serial]`

### Parallel Execution Tests
- Run full test suite with `cargo nextest run`
- Verify no race conditions or flaky tests
- Confirm `#[serial]` tests run correctly

### Regression Tests
- Ensure `PRODIGY_ARG` / `$ARG` substitution works as before
- Verify all existing behavior preserved

## Documentation Requirements

### Code Documentation
- Document `EnvReader` trait and its purpose
- Add examples for using `MockEnv` in tests
- Document when `#[serial]` is appropriate

### User Documentation
- Update CLAUDE.md with environment access patterns
- Add section on testing best practices

### Architecture Updates
- Update any architecture docs to reflect new pattern

## Implementation Notes

### Migration Order

Recommend implementing in this order to minimize risk:

1. **Add EnvReader trait** - Non-breaking addition
2. **Add EnvReader to new code paths** - Test with existing code
3. **Migrate tests file by file** - Each file independent
4. **Refactor production argument_processing.rs** - Most impactful change last
5. **Remove deprecated TestEnv** - Already done in previous commit

### Gotchas

1. **Default EnvReader**: Components should default to `RealEnvReader` to maintain backwards compatibility
2. **Async contexts**: `MockEnv` must be thread-safe for async tests
3. **Subprocess tests**: Cannot use MockEnv for tests that spawn actual processes
4. **Path expansion**: `expand_home_dir` reads `HOME` - must use EnvReader

### Performance Considerations

- `RealEnvReader` should be zero-cost (just delegates to std::env)
- Avoid excessive trait object indirection in hot paths
- Consider using generics instead of `dyn EnvReader` where possible

## Migration and Compatibility

### Breaking Changes
- None if implemented correctly with defaults

### Migration Path
1. Components gain optional `EnvReader` parameter
2. Tests migrate to using MockEnv
3. Old patterns deprecated but continue to work
4. Eventually remove deprecated patterns

### Backwards Compatibility
- All public APIs maintain existing signatures
- New methods added with `_with_env_reader` suffix or builder pattern
