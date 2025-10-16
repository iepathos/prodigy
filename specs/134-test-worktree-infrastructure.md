---
number: 134
title: Test Worktree Infrastructure
category: testing
priority: high
status: draft
dependencies: []
created: 2025-10-16
---

# Specification 134: Test Worktree Infrastructure

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The resume integration tests are currently failing because they create mock checkpoint data but not actual git worktrees that the resume command expects. The tests were disabled with `#[ignore]` attribute after commit 458239b8, which identified three core issues:

1. Test IDs were not using the 'session-' prefix, causing incorrect detection as MapReduce jobs
2. Tests create mock worktree directories but not actual git worktrees
3. Resume command expects fully initialized git worktrees to exist

While issue #1 was fixed by updating test IDs, issues #2 and #3 require proper test infrastructure to create and manage git worktrees. Currently, 9 resume integration tests are disabled and cannot validate the resume functionality.

The existing `CliTest` helper does create git repositories in temporary directories, but the resume tests need actual worktrees created through Prodigy's `WorktreeManager` to properly simulate interrupted workflows.

## Objective

Create comprehensive test infrastructure that enables integration tests to properly create and manage git worktrees, allowing the 9 disabled resume integration tests to be re-enabled and validate resume functionality end-to-end.

## Requirements

### Functional Requirements

- **Test Worktree Creation**: Provide helper functions to create proper git worktrees using `WorktreeManager`
- **Checkpoint Integration**: Ensure created worktrees are properly linked to test checkpoints
- **Session State Setup**: Create complete session state that matches real workflow execution
- **Environment Isolation**: Maintain test isolation while creating worktrees in temporary directories
- **Cleanup Management**: Ensure worktrees are properly cleaned up after tests complete

### Non-Functional Requirements

- **Test Performance**: Worktree creation should not significantly slow down test execution
- **Resource Management**: Tests should not leak worktrees or repository state
- **Cross-Platform**: Infrastructure must work on Linux, macOS, and Windows
- **Maintainability**: Helper functions should be reusable across multiple test files

## Acceptance Criteria

- [ ] Test helper function `create_test_worktree()` creates actual git worktrees using `WorktreeManager`
- [ ] Test helper properly initializes worktree with git configuration (user.name, user.email)
- [ ] Worktree creation integrates with existing `setup_test_prodigy_home()` isolation
- [ ] Enhanced `create_test_checkpoint()` helper links checkpoints to actual worktrees
- [ ] All 9 disabled resume integration tests pass when re-enabled:
  - `test_resume_from_early_interruption`
  - `test_resume_from_middle_interruption`
  - `test_resume_with_variable_preservation`
  - `test_resume_with_retry_state`
  - `test_resume_with_force_restart`
  - `test_resume_parallel_workflow`
  - `test_resume_with_checkpoint_cleanup`
  - `test_resume_workflow_with_on_failure_handlers`
  - `test_end_to_end_error_handler_execution_after_resume`
- [ ] No worktree leaks detected after test suite execution
- [ ] Test execution time increases by less than 20% compared to current skipped tests
- [ ] Documentation added to test helper functions explaining worktree setup pattern

## Technical Details

### Implementation Approach

The test infrastructure should provide a layered approach to worktree creation:

1. **Base Repository Setup**: Use existing `CliTest::new()` git repository initialization
2. **Worktree Manager Integration**: Import and use `WorktreeManager` from production code
3. **Session Simulation**: Create complete session state including worktree metadata
4. **Checkpoint Coordination**: Ensure checkpoint references match created worktree paths

### Test Helper API Design

```rust
/// Enhanced test checkpoint creation that includes worktree setup
pub fn create_test_checkpoint_with_worktree(
    prodigy_home: &Path,
    project_root: &Path,
    workflow_id: &str,
    commands_executed: usize,
    total_commands: usize,
    variables: serde_json::Value,
) -> anyhow::Result<PathBuf> {
    // 1. Create actual worktree using WorktreeManager
    // 2. Initialize git config in worktree
    // 3. Create checkpoint referencing worktree
    // 4. Create session state in UnifiedSessionManager location
    // 5. Return worktree path for test validation
}

/// Create a proper test worktree using production WorktreeManager
pub fn create_test_worktree(
    prodigy_home: &Path,
    project_root: &Path,
    worktree_name: &str,
) -> anyhow::Result<PathBuf> {
    // 1. Initialize WorktreeManager with test paths
    // 2. Create worktree branch
    // 3. Initialize worktree with git config
    // 4. Return worktree path
}

/// Cleanup test worktrees after test completion
pub fn cleanup_test_worktree(
    prodigy_home: &Path,
    project_root: &Path,
    worktree_name: &str,
) -> anyhow::Result<()> {
    // 1. Remove worktree using WorktreeManager
    // 2. Clean up any remaining state
}
```

### Integration with Existing Test Infrastructure

The new helpers should integrate with existing test patterns:

```rust
#[test]
fn test_resume_from_early_interruption() {
    // Use existing isolation
    let (_env, _prodigy_home) = setup_test_prodigy_home();

    // Use existing CliTest setup
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();

    // NEW: Create actual worktree with checkpoint
    let workflow_id = "session-resume-early-12345";
    let worktree_path = create_test_checkpoint_with_worktree(
        &PathBuf::from(std::env::var("PRODIGY_HOME").unwrap()),
        &test_dir,
        workflow_id,
        1, // commands_executed
        5, // total_commands
        json!({ "variable1": "test-value" }),
    ).unwrap();

    // Create workflow file in PROJECT ROOT (not worktree)
    let workflow_path = create_test_workflow(&test_dir, "test-resume-workflow.yaml");

    // Resume should now work with actual worktree
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}
```

### Worktree Manager Integration

The test infrastructure should use the production `WorktreeManager`:

```rust
use prodigy::git::worktree_manager::WorktreeManager;

pub fn create_test_worktree(
    prodigy_home: &Path,
    project_root: &Path,
    worktree_name: &str,
) -> anyhow::Result<PathBuf> {
    // Initialize manager with test paths
    let manager = WorktreeManager::new(project_root.to_path_buf());

    // Create worktree in test-isolated location
    let worktree_path = prodigy_home
        .join("worktrees")
        .join("prodigy")
        .join(worktree_name);

    // Create worktree through manager
    manager.create_worktree(worktree_name, &worktree_path)?;

    // Initialize git config
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&worktree_path)
        .output()?;

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&worktree_path)
        .output()?;

    Ok(worktree_path)
}
```

### Checkpoint and Session State Coordination

The enhanced checkpoint helper should create all necessary state:

```rust
pub fn create_test_checkpoint_with_worktree(
    prodigy_home: &Path,
    project_root: &Path,
    workflow_id: &str,
    commands_executed: usize,
    total_commands: usize,
    variables: serde_json::Value,
) -> anyhow::Result<PathBuf> {
    // 1. Create actual worktree
    let worktree_path = create_test_worktree(
        prodigy_home,
        project_root,
        workflow_id,
    )?;

    // 2. Create checkpoint in proper location
    let checkpoint_dir = prodigy_home
        .join("state")
        .join(workflow_id)
        .join("checkpoints");
    std::fs::create_dir_all(&checkpoint_dir)?;

    // 3. Create checkpoint JSON (same structure as current helper)
    let now = chrono::Utc::now();
    let checkpoint = json!({
        "workflow_id": workflow_id,
        "execution_state": { /* ... */ },
        "completed_steps": [ /* ... */ ],
        "variable_state": variables,
        // ... rest of checkpoint structure
    });

    let checkpoint_file = checkpoint_dir.join(format!("{}.checkpoint.json", workflow_id));
    std::fs::write(&checkpoint_file, serde_json::to_string_pretty(&checkpoint)?)?;

    // 4. Create UnifiedSession state
    let unified_session = json!({
        "id": workflow_id,
        "session_type": "Workflow",
        "status": "Paused",
        "workflow_data": {
            "workflow_id": workflow_id,
            "worktree_name": workflow_id,
            // ... rest of session data
        },
        // ...
    });

    let sessions_dir = prodigy_home.join("sessions");
    std::fs::create_dir_all(&sessions_dir)?;
    std::fs::write(
        sessions_dir.join(format!("{}.json", workflow_id)),
        serde_json::to_string_pretty(&unified_session)?,
    )?;

    Ok(worktree_path)
}
```

## Dependencies

**Prerequisites**: None - uses existing production code (`WorktreeManager`, `UnifiedSessionManager`)

**Affected Components**:
- `tests/cli_integration/resume_integration_tests.rs` - Will be updated to use new helpers
- `tests/cli_integration/test_utils.rs` - Will receive new helper functions

**External Dependencies**: None - all dependencies already present in project

## Testing Strategy

### Unit Tests

- Test `create_test_worktree()` creates valid git worktree
- Test worktree has proper git configuration
- Test worktree cleanup removes all artifacts
- Test checkpoint-worktree linkage is correct

### Integration Tests

- Re-enable all 9 disabled resume integration tests
- Verify each test passes with actual worktree infrastructure
- Test worktree isolation between concurrent tests
- Verify no worktree leaks after test suite completion

### Performance Tests

- Measure test execution time with worktree creation
- Ensure overhead is acceptable (< 20% increase)
- Profile worktree creation/cleanup operations

### Edge Case Testing

- Test behavior when worktree creation fails
- Test cleanup when tests are interrupted
- Test concurrent test execution doesn't conflict
- Test worktree state matches production workflow state

## Documentation Requirements

### Code Documentation

- Document all new test helper functions with examples
- Add inline comments explaining worktree setup steps
- Document worktree lifecycle in test infrastructure

### Test Documentation

- Update test file header comments with worktree requirements
- Document test helper usage patterns
- Explain worktree isolation strategy

### Architecture Updates

Not required - this is test infrastructure only

## Implementation Notes

### Key Design Decisions

1. **Use Production WorktreeManager**: Rather than mocking worktree creation, use the actual `WorktreeManager` to ensure tests validate real behavior
2. **Enhanced Helper Functions**: Extend existing `create_test_checkpoint()` rather than replacing it to minimize test changes
3. **Automatic Cleanup**: Use `Drop` trait or test cleanup hooks to ensure worktrees are removed even if tests panic

### Common Pitfalls to Avoid

- **Path Confusion**: Ensure workflow files are in project root, not worktree
- **Git Config**: Always initialize user.name and user.email in test worktrees
- **State Mismatch**: Ensure checkpoint, session, and worktree state are consistent
- **Resource Leaks**: Always clean up worktrees, even on test failure

### Performance Considerations

- Worktree creation is relatively fast (< 100ms per worktree)
- Can parallelize test execution since worktrees are isolated
- Consider caching base repository state for faster setup

### Testing Philosophy

These tests validate end-to-end resume functionality by simulating real workflow interruption scenarios. The worktree infrastructure should mirror production behavior as closely as possible while maintaining test isolation and repeatability.

## Migration and Compatibility

### Migration Path

1. **Phase 1**: Implement new test helper functions
2. **Phase 2**: Update one disabled test to use new infrastructure and verify it passes
3. **Phase 3**: Update remaining tests incrementally
4. **Phase 4**: Remove `#[ignore]` attributes once all tests pass
5. **Phase 5**: Add documentation and cleanup any temporary workarounds

### Breaking Changes

None - this is test infrastructure only and doesn't affect production code or APIs.

### Compatibility Considerations

- Test infrastructure must work across all supported platforms (Linux, macOS, Windows)
- Git version compatibility should be validated (minimum git version for worktree support)
- Temporary directory handling should respect platform-specific conventions

## Success Metrics

- All 9 disabled resume integration tests are re-enabled and passing
- No worktree leaks detected in CI/CD runs
- Test execution time remains acceptable
- Test helpers are reusable for future resume-related tests
- Production resume functionality is fully validated by integration tests
