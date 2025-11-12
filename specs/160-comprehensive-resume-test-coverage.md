---
number: 160
title: Comprehensive Resume Test Coverage
category: testing
priority: high
status: draft
dependencies: [134, 159]
created: 2025-01-11
---

# Specification 160: Comprehensive Resume Test Coverage

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: Spec 134 (MapReduce Checkpoint and Resume), Spec 159 (MapReduce Resume CLI)

## Context

The Phase 1 investigation revealed significant gaps in test coverage for checkpoint and resume functionality. While basic checkpoint creation and loading are well-tested, critical failure scenarios and edge cases are not adequately covered. This leads to bugs that only surface in production, such as the workflow_path hardcoding bug fixed in Phase 1.

Current test coverage analysis:
- ✅ Happy path checkpoint/resume: Well covered
- ⚠️ Failure recovery: Partially covered
- ❌ Edge cases: Minimal coverage
- ❌ Environment changes: Not covered
- ❌ End-to-end CLI integration: Incomplete

## Objective

Achieve comprehensive test coverage for checkpoint and resume functionality to prevent regression and catch edge cases before production deployment. Ensure all failure scenarios, edge cases, and environment changes are thoroughly tested with clear, maintainable test cases.

## Requirements

### Functional Requirements

1. **Failure Scenario Tests**
   - Workflow fails mid-execution → checkpoint saved → resume succeeds
   - Resume with modified workflow file (detect incompatibility)
   - Resume with missing worktree (create new or error gracefully)
   - Resume with corrupted checkpoint file (detect and error clearly)
   - Resume with missing workflow file (error with helpful message)

2. **Edge Case Tests**
   - Checkpoint version incompatibility (old checkpoint, new code)
   - Workflow hash mismatch (workflow changed between checkpoint/resume)
   - Environment variable changes between checkpoint and resume
   - Git branch changes between checkpoint and resume
   - Multiple resume attempts on same checkpoint (idempotency)

3. **MapReduce-Specific Tests**
   - Resume with all work items failed (DLQ only)
   - Resume from reduce phase with no map results (error)
   - Resume with agent cleanup failures (orphaned worktrees)
   - Resume with modified max_parallel setting
   - Resume with DLQ items exceeding max_retries

4. **CLI Integration Tests**
   - End-to-end `prodigy resume` with standard workflow failure
   - End-to-end `prodigy resume-job` with MapReduce interruption
   - Resume command auto-detection (session vs job ID)
   - Resume with various verbosity levels (-v, -vv, -vvv)
   - Resume error messaging validation (clear, actionable)

5. **State Management Tests**
   - Session and checkpoint state divergence detection
   - Transactional state updates (rollback on failure)
   - Concurrent resume prevention (locking mechanism)
   - Stale checkpoint cleanup and retention

### Non-Functional Requirements

1. **Test Execution Performance**
   - Full test suite should complete in <5 minutes
   - Individual integration tests should complete in <30 seconds
   - No flaky tests (must be deterministic and reliable)

2. **Test Maintainability**
   - Tests should be self-documenting with clear descriptions
   - Use helper functions to reduce duplication
   - Mock complex dependencies appropriately
   - Maintain test data fixtures for reproducibility

3. **Coverage Metrics**
   - Achieve >90% code coverage for checkpoint/resume modules
   - Cover all error paths and failure scenarios
   - Test all public APIs and CLI commands
   - Validate all acceptance criteria from Spec 134 and 159

4. **Test Organization**
   - Group related tests logically
   - Use descriptive test names following conventions
   - Separate unit, integration, and E2E tests clearly
   - Document test purpose and expected behavior

## Acceptance Criteria

- [ ] All failure scenario tests pass (workflow failure, missing files, corruption)
- [ ] All edge case tests pass (version mismatch, environment changes, hash mismatch)
- [ ] All MapReduce-specific tests pass (all-failed, no results, cleanup failures)
- [ ] All CLI integration tests pass (E2E resume for standard and MapReduce)
- [ ] All state management tests pass (divergence, transactions, locking)
- [ ] Test coverage for checkpoint/resume modules exceeds 90%
- [ ] No flaky tests (100% pass rate over 10 consecutive runs)
- [ ] All tests have clear, descriptive names and documentation
- [ ] Test execution time is <5 minutes for full suite
- [ ] Code review confirms test quality and maintainability

## Technical Details

### Implementation Approach

1. **Test File Organization**
   ```
   tests/
   ├── checkpoint_resume/
   │   ├── failure_scenarios_test.rs       // Workflow failures and errors
   │   ├── edge_cases_test.rs              // Version, hash, env changes
   │   ├── mapreduce_specific_test.rs      // MapReduce edge cases
   │   ├── cli_integration_test.rs         // End-to-end CLI tests
   │   └── state_management_test.rs        // State consistency tests
   ```

2. **Test Helpers and Utilities**
   ```rust
   // tests/helpers/checkpoint_helpers.rs
   pub struct CheckpointTestBuilder {
       workflow_id: String,
       workflow_path: PathBuf,
       failed_step: Option<usize>,
       completed_steps: Vec<CompletedStep>,
   }

   impl CheckpointTestBuilder {
       pub fn with_failure_at_step(mut self, step: usize) -> Self { ... }
       pub fn with_custom_workflow_path(mut self, path: PathBuf) -> Self { ... }
       pub fn build(self) -> WorkflowCheckpoint { ... }
   }

   // tests/helpers/workflow_helpers.rs
   pub async fn create_failing_workflow(
       dir: &Path,
       fail_step: usize,
   ) -> Result<PathBuf> { ... }

   pub async fn corrupt_checkpoint(
       checkpoint_path: &Path,
   ) -> Result<()> { ... }
   ```

3. **Test Coverage Strategy**

   **Unit Tests** (in src/ modules):
   - Test pure functions in checkpoint creation/validation
   - Test resume logic state transitions
   - Test work item deduplication algorithms
   - Test checkpoint version compatibility checks

   **Integration Tests** (in tests/ directory):
   - Test full checkpoint save → load → resume cycle
   - Test interaction between checkpoint manager and executor
   - Test session manager state updates during resume
   - Test MapReduce phase transitions on resume

   **E2E Tests** (CLI integration):
   - Test actual CLI command execution
   - Test user-visible output and error messages
   - Test workflow execution → interrupt → resume flow
   - Test MapReduce workflow lifecycle with resume

### Test Cases

#### 1. Failure Scenario Tests

**Test: Workflow fails mid-execution, checkpoint, resume**
```rust
#[tokio::test]
async fn test_workflow_failure_midexecution_resume() -> Result<()> {
    // 1. Create workflow that fails on step 2
    let workflow = create_failing_workflow(step: 2).await?;

    // 2. Execute and expect failure
    let result = execute_workflow(&workflow).await;
    assert!(result.is_err());

    // 3. Verify checkpoint was saved
    let checkpoint = load_checkpoint(&session_id).await?;
    assert_eq!(checkpoint.execution_state.current_step_index, 2);

    // 4. Verify session is resumable
    let session = load_session(&session_id).await?;
    assert!(session.is_resumable());

    // 5. Resume workflow
    let resume_result = resume_workflow(&session_id).await?;
    assert!(resume_result.success);

    Ok(())
}
```

**Test: Resume with missing workflow file**
```rust
#[tokio::test]
async fn test_resume_with_missing_workflow_file() -> Result<()> {
    // 1. Create checkpoint with workflow_path
    let checkpoint = create_checkpoint_with_path("workflow.yml").await?;

    // 2. Delete workflow file
    tokio::fs::remove_file("workflow.yml").await?;

    // 3. Attempt resume
    let result = resume_workflow(&session_id).await;

    // 4. Verify error message is helpful
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Workflow file not found"));
    assert!(error.to_string().contains("may have been moved or deleted"));

    Ok(())
}
```

#### 2. Edge Case Tests

**Test: Checkpoint version incompatibility**
```rust
#[tokio::test]
async fn test_checkpoint_version_mismatch() -> Result<()> {
    // 1. Create checkpoint with future version
    let mut checkpoint = create_checkpoint().await?;
    checkpoint.version = CHECKPOINT_VERSION + 10;
    save_checkpoint(&checkpoint).await?;

    // 2. Attempt to load checkpoint
    let result = load_checkpoint(&session_id).await;

    // 3. Verify version error
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("version"));
    assert!(error.to_string().contains("not supported"));

    Ok(())
}
```

**Test: Environment variable changes**
```rust
#[tokio::test]
async fn test_resume_with_changed_environment() -> Result<()> {
    // 1. Create checkpoint with env vars
    let env_vars = HashMap::from([
        ("API_KEY".to_string(), "secret123".to_string()),
    ]);
    let checkpoint = create_checkpoint_with_env(env_vars).await?;

    // 2. Change environment variable
    std::env::set_var("API_KEY", "different_value");

    // 3. Resume workflow
    let result = resume_workflow(&session_id).await;

    // 4. Verify environment is validated/warned
    // (Implementation may vary: error, warning, or allow)
    // This test documents expected behavior

    Ok(())
}
```

#### 3. MapReduce-Specific Tests

**Test: Resume with all items failed**
```rust
#[tokio::test]
async fn test_resume_mapreduce_all_items_failed() -> Result<()> {
    // 1. Create MapReduce job with 10 items
    let job = create_mapreduce_job(10).await?;

    // 2. Run job and fail all items
    fail_all_items(&job.id).await?;

    // 3. Verify all items in DLQ
    let dlq = load_dlq(&job.id).await?;
    assert_eq!(dlq.items.len(), 10);

    // 4. Resume with retry enabled
    let result = resume_job(&job.id, force_retry: true).await?;

    // 5. Verify items were retried
    assert_eq!(result.items_processed, 10);

    Ok(())
}
```

**Test: Resume with agent cleanup failures**
```rust
#[tokio::test]
async fn test_resume_with_orphaned_worktrees() -> Result<()> {
    // 1. Create MapReduce job
    let job = create_mapreduce_job(5).await?;

    // 2. Simulate cleanup failures (create orphaned worktrees)
    create_orphaned_worktrees(&job.id, 2).await?;

    // 3. Resume job
    let result = resume_job(&job.id).await;

    // 4. Verify resume handles orphaned worktrees gracefully
    assert!(result.is_ok());

    // 5. Verify orphaned worktrees are tracked
    let orphaned = load_orphaned_worktrees(&job.id).await?;
    assert_eq!(orphaned.len(), 2);

    Ok(())
}
```

#### 4. CLI Integration Tests

**Test: End-to-end CLI resume for standard workflow**
```rust
#[tokio::test]
async fn test_cli_resume_standard_workflow() -> Result<()> {
    // 1. Create and run workflow that fails
    let workflow_path = create_failing_workflow().await?;
    let output = run_cli(&["prodigy", "run", workflow_path.to_str().unwrap()]).await?;
    assert!(!output.status.success());

    // 2. Get session ID from output
    let session_id = extract_session_id(&output.stderr)?;

    // 3. Resume via CLI
    let resume_output = run_cli(&["prodigy", "resume", &session_id]).await?;

    // 4. Verify resume succeeded
    assert!(resume_output.status.success());
    assert!(resume_output.stdout.contains("Resume complete"));

    Ok(())
}
```

**Test: End-to-end CLI resume for MapReduce**
```rust
#[tokio::test]
async fn test_cli_resume_mapreduce_workflow() -> Result<()> {
    // 1. Start MapReduce workflow
    let workflow_path = create_mapreduce_workflow().await?;
    let process = start_workflow_async(&workflow_path).await?;

    // 2. Wait for 2 items to process
    wait_for_items_processed(2).await?;

    // 3. Interrupt workflow
    interrupt_process(&process).await?;

    // 4. Get job ID
    let job_id = get_job_id_from_session().await?;

    // 5. Resume via CLI
    let output = run_cli(&["prodigy", "resume-job", &job_id]).await?;

    // 6. Verify all items completed
    assert!(output.status.success());
    assert!(output.stdout.contains("items processed"));

    Ok(())
}
```

### Data Structures

**Test Fixtures**:
```rust
pub struct TestFixture {
    pub temp_dir: TempDir,
    pub workflow_path: PathBuf,
    pub checkpoint_dir: PathBuf,
    pub session_id: String,
}

impl TestFixture {
    pub async fn new() -> Result<Self> { ... }
    pub async fn create_checkpoint(&self) -> Result<WorkflowCheckpoint> { ... }
    pub async fn corrupt_checkpoint(&self) -> Result<()> { ... }
    pub async fn remove_workflow_file(&self) -> Result<()> { ... }
}
```

### Integration Points

1. **Checkpoint System**
   - Test all checkpoint creation paths (success, error, completion)
   - Test checkpoint validation and integrity checks
   - Test checkpoint version compatibility

2. **Session Management**
   - Test session state updates during resume
   - Test session-checkpoint state consistency
   - Test resumable state detection

3. **Resume Logic**
   - Test resume execution from various states
   - Test work item collection and deduplication
   - Test phase detection and transition

4. **CLI Commands**
   - Test CLI argument parsing
   - Test CLI output formatting
   - Test CLI error handling and messages

## Dependencies

### Prerequisites
- **Spec 134**: MapReduce Checkpoint and Resume (provides checkpoint functionality to test)
- **Spec 159**: MapReduce Resume CLI (provides CLI implementation to test)

### Affected Components
- `tests/` - All new test files
- `tests/helpers/` - Test utilities and fixtures
- Existing test files - May need updates for new test patterns

### External Dependencies
- `tempfile` - For test directory creation
- `assert_cmd` - For CLI testing
- `predicates` - For output validation
- Existing test infrastructure (tokio, anyhow)

## Testing Strategy

### Test Development Process

1. **Phase 1: Test Infrastructure**
   - Create test helper modules
   - Build test fixtures and builders
   - Establish test patterns and conventions

2. **Phase 2: Failure Scenarios**
   - Implement workflow failure tests
   - Add missing file and corruption tests
   - Validate error messages

3. **Phase 3: Edge Cases**
   - Add version compatibility tests
   - Implement environment change tests
   - Test workflow hash mismatch

4. **Phase 4: MapReduce Tests**
   - Add all-failed scenario tests
   - Test orphaned worktree handling
   - Validate DLQ integration

5. **Phase 5: CLI Integration**
   - Implement E2E CLI tests
   - Validate output and error messages
   - Test auto-detection logic

6. **Phase 6: Coverage Analysis**
   - Run coverage tools (cargo-tarpaulin)
   - Identify coverage gaps
   - Add tests for uncovered code paths

### Coverage Measurement

```bash
# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage \
  --exclude-files 'tests/*' 'target/*' \
  --packages prodigy

# View coverage by module
cargo tarpaulin --print-summary --packages prodigy \
  --include-files 'src/cook/workflow/checkpoint*.rs' \
  'src/cook/workflow/resume.rs' \
  'src/cook/execution/mapreduce_resume.rs'
```

Target coverage levels:
- Checkpoint modules: >95%
- Resume modules: >90%
- CLI commands: >85%
- Integration paths: >80%

### Test Quality Assurance

**Code Review Checklist**:
- [ ] Test names clearly describe what is being tested
- [ ] Test documentation explains purpose and expected behavior
- [ ] Tests are deterministic (no random failures)
- [ ] Tests clean up resources (temp files, processes)
- [ ] Assertions have descriptive failure messages
- [ ] Tests use appropriate abstraction level
- [ ] No copy-paste code between tests (use helpers)
- [ ] Tests are focused (one concern per test)

## Documentation Requirements

### Code Documentation
- Document test helper functions with usage examples
- Add module-level documentation explaining test organization
- Comment complex test setup or validation logic
- Include references to specs being tested

### Test Documentation
- Create `tests/README.md` explaining test organization
- Document test naming conventions
- Provide examples of running specific test suites
- Explain test fixture usage

### Coverage Reports
- Generate and commit coverage reports
- Document coverage targets and current status
- Track coverage trends over time
- Identify modules needing additional tests

## Implementation Notes

### Test Isolation

Each test must be fully isolated:
- Use unique temporary directories
- Don't share state between tests
- Clean up resources in test teardown
- Use unique session/job IDs

### Test Data Management

Create reusable test fixtures:
```rust
pub mod fixtures {
    pub fn simple_workflow() -> &'static str { ... }
    pub fn failing_workflow(fail_step: usize) -> String { ... }
    pub fn mapreduce_workflow(items: usize) -> String { ... }
}
```

### Async Test Patterns

Use consistent async test patterns:
```rust
#[tokio::test]
async fn test_name() -> Result<()> {
    // Arrange
    let fixture = TestFixture::new().await?;

    // Act
    let result = operation_under_test(&fixture).await?;

    // Assert
    assert_eq!(result.expected_field, expected_value);

    Ok(())
}
```

### Error Testing Patterns

Validate error messages properly:
```rust
let result = operation_that_fails().await;
assert!(result.is_err(), "Operation should fail");

let error = result.unwrap_err();
assert!(
    error.to_string().contains("expected error message"),
    "Error message should be helpful: {}",
    error
);
```

## Migration and Compatibility

### Breaking Changes
- None - tests only, no production code changes

### Migration Requirements
- Update CI/CD to run new test suites
- Configure coverage reporting in CI
- Set minimum coverage thresholds

### Compatibility Considerations
- Tests should work on all supported platforms (macOS, Linux, Windows)
- Tests should not depend on specific system configuration
- Tests should be compatible with parallel test execution

## Success Metrics

- Test coverage for checkpoint/resume modules >90% (up from ~70%)
- Zero flaky tests (100% pass rate over 100 runs)
- All edge cases from investigation documented and tested
- Test execution time <5 minutes (currently ~2 minutes, allow for growth)
- Code review approval with no major test quality issues

## Future Enhancements (Out of Scope)

- Property-based testing with proptest/quickcheck
- Mutation testing to validate test effectiveness
- Performance benchmarking tests
- Chaos engineering tests (random failures, resource exhaustion)
- Cross-version compatibility testing (old checkpoints with new code)
