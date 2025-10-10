# Implementation Plan: Test and Refactor GitOperations::merge_agent_to_parent

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/resources/git.rs:GitOperations::merge_agent_to_parent:58
**Priority Score**: 27.76
**Debt Type**: TestingGap (0% coverage, complexity 12)

**Current Metrics**:
- Lines of Code: 91
- Functions: 1 (merge_agent_to_parent)
- Cyclomatic Complexity: 12
- Coverage: 0.0%
- Cognitive Complexity: 40
- Nesting Depth: 4

**Issue**: Add 8 tests for 100% coverage gap, then refactor complexity 12 into 8 functions

**Rationale**: Complex business logic with 100% gap. Cyclomatic complexity of 12 requires at least 12 test cases for full path coverage. After extracting 8 functions, each will need only 3-5 tests. Testing before refactoring ensures no regressions.

## Target State

**Expected Impact**:
- Complexity Reduction: 3.6 (from 12 to ~8.4)
- Coverage Improvement: 50.0% (from 0% to 50%+)
- Risk Reduction: 11.66

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 12 to ≤8
- [ ] Test coverage increased from 0% to ≥50%
- [ ] Function extracted into 8 smaller functions (complexity ≤3 each)
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Comprehensive Tests for Current Implementation

**Goal**: Achieve ≥50% test coverage on the existing `merge_agent_to_parent` function before any refactoring.

**Changes**:
- Create test module `tests/git_operations_tests.rs` or add to existing test file
- Write 8-12 test cases covering all branches:
  1. Success case: merge with valid agent branch in worktree context
  2. Error case: not in worktree context (env.worktree_name is None)
  3. Clean merge: no existing MERGE_HEAD
  4. Merge recovery: MERGE_HEAD exists with staged changes → commit succeeds
  5. Merge recovery: MERGE_HEAD exists with staged changes → commit fails → abort
  6. Merge recovery: MERGE_HEAD exists with no staged changes → abort
  7. Merge failure: git merge command fails
  8. Git status failure during merge recovery
- Use test fixtures for git repository setup
- Mock or use temporary git repositories for testing

**Testing**:
```bash
cargo test git_operations::merge_agent_to_parent
cargo tarpaulin --lib -- git_operations::merge_agent_to_parent
```

**Success Criteria**:
- [ ] 8+ test cases written and passing
- [ ] Coverage reaches ≥50% on merge_agent_to_parent
- [ ] Tests are deterministic and don't flake
- [ ] Tests follow existing project patterns
- [ ] All tests pass: `cargo test`
- [ ] Ready to commit

### Phase 2: Extract Pure Decision Logic Functions

**Goal**: Extract pure functions for decision-making logic (no I/O), reducing cognitive complexity.

**Changes**:
- Extract validation logic:
  ```rust
  fn validate_worktree_context(env: &ExecutionEnvironment) -> Result<(), String>
  ```
  - Checks if `env.worktree_name.is_some()`
  - Returns error message if not in worktree context
  - Complexity: ≤2

- Extract merge state detection:
  ```rust
  fn has_incomplete_merge(parent_path: &Path) -> bool
  ```
  - Checks if `.git/MERGE_HEAD` exists
  - Pure file existence check
  - Complexity: 1

- Extract status analysis:
  ```rust
  fn parse_git_status(status_output: &str) -> MergeRecoveryAction
  ```
  - Analyzes porcelain status output
  - Returns enum: `CommitStagedChanges | AbortMerge | NoAction`
  - Complexity: ≤2

**Testing**:
- Write 3-4 unit tests per extracted function
- Test edge cases (empty strings, special characters, etc.)
- Verify all original tests still pass

**Success Criteria**:
- [ ] 3 pure functions extracted with ≤2 complexity each
- [ ] 10+ unit tests for extracted functions
- [ ] Original merge_agent_to_parent tests still pass
- [ ] No regression in coverage
- [ ] All tests pass: `cargo test`
- [ ] Ready to commit

### Phase 3: Extract I/O Operations into Focused Functions

**Goal**: Separate I/O operations from business logic, reducing complexity and improving testability.

**Changes**:
- Extract merge recovery operations:
  ```rust
  async fn recover_incomplete_merge(
      parent_path: &Path,
      agent_branch: &str
  ) -> MapReduceResult<()>
  ```
  - Handles the entire merge recovery flow (lines 73-121)
  - Uses extracted `has_incomplete_merge` and `parse_git_status`
  - Complexity: ≤4

- Extract merge execution:
  ```rust
  async fn execute_merge(
      parent_path: &Path,
      agent_branch: &str
  ) -> MapReduceResult<()>
  ```
  - Performs the actual git merge command (lines 124-140)
  - Single responsibility: execute merge
  - Complexity: ≤2

- Extract git status check:
  ```rust
  async fn check_git_status(parent_path: &Path) -> MapReduceResult<String>
  ```
  - Runs `git status --porcelain`
  - Returns status output string
  - Complexity: ≤2

- Extract git commit operation:
  ```rust
  async fn commit_staged_changes(parent_path: &Path) -> MapReduceResult<()>
  ```
  - Commits staged changes with `--no-edit`
  - Returns result
  - Complexity: ≤2

- Extract merge abort operation:
  ```rust
  async fn abort_merge(parent_path: &Path) -> MapReduceResult<()>
  ```
  - Aborts incomplete merge
  - Best-effort operation (ignores errors)
  - Complexity: ≤1

**Testing**:
- Write integration tests for each I/O function
- Use temporary git repos for testing
- Mock filesystem operations where appropriate
- Verify `merge_agent_to_parent` becomes a thin orchestrator

**Success Criteria**:
- [ ] 5 I/O functions extracted with ≤4 complexity each
- [ ] 15+ tests for extracted I/O functions
- [ ] `merge_agent_to_parent` reduced to orchestration logic (complexity ≤4)
- [ ] Coverage maintained or improved
- [ ] All tests pass: `cargo test`
- [ ] Ready to commit

### Phase 4: Refactor Main Function to Composition

**Goal**: Refactor `merge_agent_to_parent` to be a thin orchestrator composing extracted functions.

**Changes**:
- Rewrite `merge_agent_to_parent` to:
  ```rust
  pub async fn merge_agent_to_parent(
      &self,
      agent_branch: &str,
      env: &ExecutionEnvironment,
  ) -> MapReduceResult<()> {
      // Validate context (extracted function)
      validate_worktree_context(env)
          .map_err(|msg| self.create_git_error("merge_to_parent", &msg))?;

      let parent_path = &env.working_dir;

      // Recover from incomplete merge if needed (extracted function)
      if has_incomplete_merge(parent_path) {
          self.recover_incomplete_merge(parent_path, agent_branch).await?;
      }

      // Execute the merge (extracted function)
      self.execute_merge(parent_path, agent_branch).await?;

      info!("Successfully merged agent branch {} to parent", agent_branch);
      Ok(())
  }
  ```
- Target complexity: ≤3
- Clear separation: validation → recovery → execution
- Each step is independently testable

**Testing**:
- Verify all original integration tests pass
- Add end-to-end tests covering full workflow
- Verify extracted functions are properly composed

**Success Criteria**:
- [ ] `merge_agent_to_parent` complexity reduced to ≤3
- [ ] Function reads as high-level workflow
- [ ] No business logic remains in main function
- [ ] All 8+ extracted functions have ≤3 complexity
- [ ] All tests pass: `cargo test`
- [ ] Coverage ≥80% on entire module
- [ ] Ready to commit

### Phase 5: Final Verification and Cleanup

**Goal**: Verify all improvements, run full test suite, and ensure quality standards.

**Changes**:
- Run full CI pipeline
- Generate coverage report
- Run clippy with all lints
- Format code
- Review all extracted functions for:
  - Clear naming
  - Proper documentation
  - Consistent error handling
  - No code duplication

**Testing**:
```bash
# Full CI checks
just ci

# Coverage verification
cargo tarpaulin --lib --out Html --output-dir coverage

# Specific module coverage
cargo tarpaulin --lib -- git_operations

# Debtmap re-analysis
debtmap analyze
```

**Success Criteria**:
- [ ] All CI checks pass (build, test, clippy, fmt)
- [ ] Coverage ≥50% on merge_agent_to_parent and extracted functions
- [ ] Debtmap score reduced by ≥50%
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] All documentation updated
- [ ] Ready for final commit and PR

## Testing Strategy

### For Each Phase:

1. **Write tests first** (or alongside implementation)
2. **Run targeted tests**:
   ```bash
   cargo test git_operations::merge_agent_to_parent
   ```
3. **Verify no regressions**:
   ```bash
   cargo test --lib
   ```
4. **Check for warnings**:
   ```bash
   cargo clippy
   ```
5. **Measure coverage**:
   ```bash
   cargo tarpaulin --lib -- git_operations
   ```

### Final Verification:

1. **Full CI pipeline**:
   ```bash
   just ci
   ```
2. **Coverage report**:
   ```bash
   cargo tarpaulin --lib --out Html
   ```
3. **Debtmap re-analysis**:
   ```bash
   debtmap analyze
   ```
4. **Compare metrics**:
   - Before: complexity=12, coverage=0%
   - After: complexity≤8, coverage≥50%

## Test Fixtures and Utilities

### Git Test Helpers Needed:

```rust
// Helper to create temporary git repository
fn create_temp_git_repo() -> TempDir

// Helper to setup worktree context
fn create_test_worktree(parent: &Path) -> (PathBuf, String)

// Helper to create merge conflict state
fn setup_merge_conflict(repo: &Path, branch: &str)

// Helper to stage changes
fn stage_test_changes(repo: &Path, files: &[&str])

// Mock ExecutionEnvironment
fn mock_execution_env(worktree_name: Option<String>) -> ExecutionEnvironment
```

## Rollback Plan

If a phase fails:

1. **Revert the phase**:
   ```bash
   git reset --hard HEAD~1
   ```

2. **Review the failure**:
   - Check test output for specific failures
   - Review compiler errors or warnings
   - Verify test fixture setup

3. **Adjust the plan**:
   - Break phase into smaller steps if needed
   - Reconsider extraction boundaries
   - Simplify test cases

4. **Retry with adjustments**:
   - Implement smaller increment
   - Add more logging/tracing
   - Verify tests pass before refactoring

## Notes

### Key Insights from Code Analysis:

1. **Merge Recovery Logic** (lines 73-121):
   - Most complex part of the function
   - Handles edge case of incomplete merge state
   - Multiple nested conditionals
   - Perfect candidate for extraction

2. **Error Handling Pattern**:
   - Uses `create_git_error` helper consistently
   - Should maintain this pattern in extracted functions
   - Consider using `anyhow::Context` for better error messages

3. **Async I/O**:
   - All git operations are async via `tokio::process::Command`
   - Extracted functions must preserve async nature
   - Test helpers need async support

4. **Logging**:
   - Uses `tracing::info` and `tracing::warn`
   - Maintain logging in extracted functions
   - Consider adding debug logging for test troubleshooting

5. **Path Handling**:
   - Uses `&**parent_path` pattern for AsRef<Path> conversions
   - Keep consistent in extracted functions

### Testing Challenges:

1. **Git Operations**: Require actual git repository or extensive mocking
   - **Solution**: Use temporary directories and real git commands in integration tests

2. **Async Tests**: Need tokio runtime
   - **Solution**: Use `#[tokio::test]` macro

3. **File System State**: Tests may interfere with each other
   - **Solution**: Each test gets isolated temp directory

4. **Merge States**: Complex to reproduce
   - **Solution**: Create helper functions to setup specific states

### Important Patterns to Preserve:

1. **Deref Pattern**: `&**parent_path` for Path conversions
2. **Error Context**: Always include operation name in errors
3. **Logging**: Info for success, warn for recoverable issues
4. **Best-Effort Cleanup**: Some operations (like merge abort) ignore errors intentionally
