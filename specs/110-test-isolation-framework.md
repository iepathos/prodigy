---
number: 110
title: Test Isolation Framework
category: testing
priority: high
status: draft
dependencies: []
created: 2025-09-30
---

# Specification 110: Test Isolation Framework

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The current test suite has significant isolation issues that cause tests to fail when run together but pass when run individually. This is due to:

1. **Global State Mutations**: Tests modify environment variables (`PRODIGY_AUTO_MERGE`, `PRODIGY_AUTO_CONFIRM`, etc.) that persist across test runs
2. **Current Directory Changes**: Tests use `std::env::set_current_dir()` which affects other tests running in parallel
3. **Git Repository State**: Tests create git repos, branches, and worktrees that interfere with each other
4. **Shared File System Resources**: Tests use non-unique temporary directories

### Evidence of Test Pollution

- 4 tests fail when run in full suite but pass individually:
  - `cook::execution::state::tests::test_list_resumable_jobs`
  - `cook::input::tests::test_file_pattern_glob_expansion`
  - `worktree::tests::test_original_branch_deleted`
  - `worktree::tests::test_worktree_tracks_feature_branch`
- With `--test-threads=1`, failures reduce to 3 (less parallelism = less interference)
- Tests that modify CWD or environment variables cause cascading failures

## Objective

Implement a comprehensive test isolation framework that ensures all tests can run safely in parallel without interfering with each other, eliminating test pollution and flakiness.

## Requirements

### Functional Requirements

1. **Environment Isolation**
   - Provide test fixtures that save and restore environment variables
   - Ensure env var changes don't leak between tests
   - Support scoped environment variable modifications

2. **Working Directory Isolation**
   - Provide safe CWD manipulation utilities
   - Automatically restore original CWD after test completion
   - Prevent CWD changes from affecting parallel tests

3. **Git Repository Isolation**
   - Ensure each test gets a completely isolated git repository
   - Use unique temporary directories with random suffixes
   - Clean up git resources after test completion

4. **Test Fixture System**
   - Create reusable test fixture utilities
   - Support setup and teardown patterns
   - Provide composable fixtures for common scenarios

### Non-Functional Requirements

- **Performance**: Test isolation overhead should be < 5% of test execution time
- **Compatibility**: Work with existing `#[tokio::test]` and `#[test]` macros
- **Ergonomics**: Easy to use with minimal boilerplate
- **Reliability**: 100% cleanup even when tests panic

## Acceptance Criteria

- [ ] All 4 currently failing tests pass when run in full test suite
- [ ] Test suite passes with `--test-threads=16` (high parallelism)
- [ ] No test failures when run 10 times consecutively
- [ ] Environment variable test fixture available and documented
- [ ] Working directory test fixture available and documented
- [ ] Git repository test fixture with unique directories
- [ ] Automatic cleanup on test panic or failure
- [ ] Migration guide for converting existing tests
- [ ] At least 10 existing tests migrated to use new fixtures
- [ ] Documentation added to testing guidelines

## Technical Details

### Implementation Approach

#### 1. Environment Variable Isolation

Create a `TestEnv` fixture that uses RAII pattern:

```rust
pub struct TestEnv {
    saved_vars: HashMap<String, Option<String>>,
}

impl TestEnv {
    pub fn new() -> Self {
        TestEnv {
            saved_vars: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        // Save original value if not already saved
        if !self.saved_vars.contains_key(key) {
            self.saved_vars.insert(
                key.to_string(),
                std::env::var(key).ok()
            );
        }
        std::env::set_var(key, value);
    }

    pub fn remove(&mut self, key: &str) {
        if !self.saved_vars.contains_key(key) {
            self.saved_vars.insert(
                key.to_string(),
                std::env::var(key).ok()
            );
        }
        std::env::remove_var(key);
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        // Restore all env vars
        for (key, value) in &self.saved_vars {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }
}
```

#### 2. Working Directory Isolation

Create a `TestWorkingDir` fixture:

```rust
pub struct TestWorkingDir {
    original_dir: PathBuf,
}

impl TestWorkingDir {
    pub fn new() -> Result<Self> {
        let original_dir = std::env::current_dir()?;
        Ok(TestWorkingDir { original_dir })
    }

    pub fn change_to(&self, path: &Path) -> Result<()> {
        std::env::set_current_dir(path)
    }
}

impl Drop for TestWorkingDir {
    fn drop(&mut self) {
        // Always restore original directory
        let _ = std::env::set_current_dir(&self.original_dir);
    }
}
```

#### 3. Git Repository Test Fixture

Create an isolated git repository fixture:

```rust
pub struct TestGitRepo {
    temp_dir: TempDir,
    path: PathBuf,
}

impl TestGitRepo {
    pub fn new() -> Result<Self> {
        // Use unique suffix to avoid collisions
        let suffix = format!("{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let temp_dir = TempDir::with_prefix(&format!("prodigy-test-{}", suffix))?;
        let path = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git")
            .current_dir(&path)
            .args(["init"])
            .output()?;

        // Configure git user
        Command::new("git")
            .current_dir(&path)
            .args(["config", "user.email", "test@test.com"])
            .output()?;

        Command::new("git")
            .current_dir(&path)
            .args(["config", "user.name", "Test User"])
            .output()?;

        Ok(TestGitRepo { temp_dir, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn commit(&self, message: &str) -> Result<()> {
        // Helper to create commits
        Command::new("git")
            .current_dir(&self.path)
            .args(["commit", "--allow-empty", "-m", message])
            .output()?;
        Ok(())
    }
}

// TempDir cleanup happens automatically
```

#### 4. Composite Test Fixture

Create a `TestContext` that combines all fixtures:

```rust
pub struct TestContext {
    pub env: TestEnv,
    pub working_dir: TestWorkingDir,
    pub git_repo: Option<TestGitRepo>,
}

impl TestContext {
    pub fn new() -> Result<Self> {
        Ok(TestContext {
            env: TestEnv::new(),
            working_dir: TestWorkingDir::new()?,
            git_repo: None,
        })
    }

    pub fn with_git_repo(mut self) -> Result<Self> {
        self.git_repo = Some(TestGitRepo::new()?);
        Ok(self)
    }
}
```

### Architecture Changes

1. **New Module**: Create `src/testing/fixtures.rs` for test fixture utilities
2. **Test Helper Module**: Create `src/testing/mod.rs` as parent module
3. **Integration**: Update `lib.rs` to include testing module with `#[cfg(test)]`

### Migration Strategy

1. **Phase 1**: Implement core fixtures (env, working_dir, git_repo)
2. **Phase 2**: Add `TestContext` composite fixture
3. **Phase 3**: Migrate failing tests to use fixtures
4. **Phase 4**: Document patterns and best practices
5. **Phase 5**: Gradually migrate remaining tests

### Example Usage

```rust
#[tokio::test]
async fn test_worktree_tracks_feature_branch() -> Result<()> {
    let mut ctx = TestContext::new()?.with_git_repo()?;
    let git_repo = ctx.git_repo.as_ref().unwrap();

    // Set environment variables with automatic cleanup
    ctx.env.set("PRODIGY_AUTO_MERGE", "true");

    // Change directory safely
    ctx.working_dir.change_to(git_repo.path())?;

    // Create feature branch
    Command::new("git")
        .current_dir(git_repo.path())
        .args(["checkout", "-b", "feature/my-feature"])
        .output()?;

    // Test logic here...

    // Cleanup happens automatically when ctx drops
    Ok(())
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/worktree/tests.rs` - Needs migration to use git repo fixtures
  - `src/cook/execution/state.rs` - Needs env var isolation
  - `src/cook/input/tests.rs` - Needs working dir isolation
  - `src/worktree/tracking_tests.rs` - Needs env var isolation
- **External Dependencies**:
  - `tempfile` crate (already in use)
  - `once_cell` for lazy static initialization (optional)

## Testing Strategy

### Unit Tests
- Test each fixture in isolation
- Verify cleanup happens even on panic
- Test nested fixture usage
- Verify no resource leaks

### Integration Tests
- Run the 4 currently failing tests with fixtures
- Verify tests pass with high parallelism (`--test-threads=16`)
- Run full test suite 10 times to ensure no flakiness

### Performance Tests
- Measure overhead of test fixtures
- Ensure < 5% performance impact
- Profile memory usage of isolated resources

### Compatibility Tests
- Verify works with `#[test]` and `#[tokio::test]`
- Test with various tempfile crate versions
- Ensure cleanup works across platforms

## Documentation Requirements

### Code Documentation
- Rustdoc for all public fixture types
- Examples for common usage patterns
- Explanation of RAII cleanup guarantees

### Testing Guidelines
- Add section on test isolation to `CONTRIBUTING.md`
- Document when to use each fixture type
- Provide migration examples for common patterns
- Best practices for writing isolated tests

### Migration Guide
- Step-by-step guide for converting existing tests
- Common pitfalls and how to avoid them
- Before/after examples

## Implementation Notes

### Panic Safety
All fixtures must use RAII pattern with `Drop` implementations to ensure cleanup happens even when tests panic. This is critical for preventing test pollution.

### Platform Considerations
- Ensure temporary directories work on Windows, Linux, and macOS
- Handle differences in environment variable behavior across platforms
- Account for git behavior differences (line endings, etc.)

### Performance Optimization
- Reuse git repo initialization code where possible
- Consider caching git configs for common test scenarios
- Use fast git operations (--allow-empty commits, etc.)

### Test Organization
```
src/
  testing/
    mod.rs          # Module declaration
    fixtures.rs     # Core fixture types
    git.rs          # Git-specific test utilities
    examples.rs     # Example usage patterns
```

## Migration and Compatibility

### Breaking Changes
None - this is additive functionality.

### Backward Compatibility
All existing tests continue to work. Migration to fixtures is optional but recommended.

### Migration Priority
1. **High Priority**: Migrate the 4 failing tests first
2. **Medium Priority**: Migrate tests that set env vars or change CWD
3. **Low Priority**: Gradually migrate remaining tests for consistency

### Rollout Plan
1. Implement fixtures in `src/testing/fixtures.rs`
2. Add unit tests for fixtures
3. Migrate 4 failing tests
4. Verify full test suite passes
5. Document patterns and best practices
6. Create migration guide
7. Gradually migrate remaining tests

## Success Metrics

- **Test Stability**: 0 test failures in 100 consecutive test runs
- **Performance**: < 5% overhead from isolation fixtures
- **Coverage**: At least 25% of tests use fixtures within 1 month
- **Documentation**: Migration guide and examples available
- **Developer Satisfaction**: Positive feedback from team on fixture usability

## Future Enhancements

### Phase 2 Features
- Database connection isolation fixtures
- Network request mocking utilities
- File system snapshot/restore utilities
- Parallel test execution optimizer

### Advanced Isolation
- Process-level isolation for extreme cases
- Container-based test isolation (optional)
- Test result caching based on code changes
- Automatic test dependency detection

## References

- Rust test best practices: https://doc.rust-lang.org/book/ch11-03-test-organization.html
- `serial_test` crate for inspiration: https://crates.io/crates/serial_test
- `tempfile` crate documentation: https://docs.rs/tempfile/