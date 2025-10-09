# Implementation Plan: Add Tests and Refactor `analyze_retention_targets`

## Problem Summary

**Location**: ./src/cli/events/mod.rs:analyze_retention_targets:953
**Priority Score**: 31.25
**Debt Type**: TestingGap (0% coverage with complexity 15)
**Current Metrics**:
- Lines of Code: 41
- Cyclomatic Complexity: 15
- Cognitive Complexity: 44
- Coverage: 0% (22 uncovered lines)
- Dependencies: 5 downstream callees, 1 upstream caller

**Issue**: Complex async function with high cyclomatic complexity (15) and zero test coverage. The function orchestrates retention analysis across global and local storage with nested conditionals and loops. Despite already having some pure helper functions extracted (`should_analyze_global_storage`, `build_global_events_path`), the main function remains untested and complex.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 4.5
- Coverage Improvement: 50.0%
- Risk Reduction: 13.125

**Success Criteria**:
- [ ] 80%+ test coverage for `analyze_retention_targets` function
- [ ] Cyclomatic complexity reduced from 15 to ≤10
- [ ] All extracted pure functions have dedicated unit tests
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Tests for Existing Pure Helper Functions

**Goal**: Establish baseline test coverage for the two pure helper functions that already exist

**Changes**:
- Tests for `should_analyze_global_storage` are already present (lines 756-769)
- Tests for `build_global_events_path` are already present (lines 773-787)
- Verify these tests pass and provide good coverage
- Add edge case tests if needed

**Testing**:
- Run `cargo test test_should_analyze_global_storage`
- Run `cargo test test_build_global_events_path`
- Verify 100% coverage of these two pure functions

**Success Criteria**:
- [ ] All existing pure function tests pass
- [ ] Edge cases are covered
- [ ] No test gaps in helper functions

### Phase 2: Extract Pure Function for Job Processing Logic

**Goal**: Extract the nested loop logic into a testable pure function

**Changes**:
- Extract job directory iteration logic into pure function:
  ```rust
  /// Pure function: Aggregate retention analysis across job directories
  async fn aggregate_job_retention(
      job_dirs: Vec<PathBuf>,
      policy: &RetentionPolicy,
  ) -> Result<RetentionAnalysis>
  ```
- This reduces nesting depth and makes the logic independently testable
- Move lines 970-981 into this new function

**Testing**:
- Write unit tests with mock job directories
- Test empty job directory list
- Test single job directory
- Test multiple job directories
- Test aggregation of analysis results

**Success Criteria**:
- [ ] New pure function has 100% test coverage
- [ ] Nesting depth reduced by 1 level
- [ ] All tests pass
- [ ] Complexity reduced by ~3 points

### Phase 3: Create Integration Tests for Main Function Path

**Goal**: Add comprehensive integration tests for both global and local storage paths

**Changes**:
- Add async test for global storage path (all_jobs=true)
- Add async test for global storage with specific job_id
- Add async test for local storage path (all_jobs=false, job_id=None)
- Add async test for non-existent paths
- Use test fixtures with mock filesystem or temporary directories

**Testing**:
```rust
#[tokio::test]
async fn test_analyze_retention_targets_global_storage() {
    // Test with all_jobs=true
}

#[tokio::test]
async fn test_analyze_retention_targets_specific_job() {
    // Test with job_id=Some("test-job")
}

#[tokio::test]
async fn test_analyze_retention_targets_local_storage() {
    // Test local file path
}

#[tokio::test]
async fn test_analyze_retention_targets_nonexistent_paths() {
    // Test graceful handling of missing directories
}
```

**Success Criteria**:
- [ ] 4+ integration tests covering main branches
- [ ] Both global and local storage paths tested
- [ ] Edge cases handled (missing dirs, empty results)
- [ ] Coverage for main function reaches 70%+
- [ ] All tests pass

### Phase 4: Refactor Archive Handling Logic

**Goal**: Extract conditional archive logic into a pure helper function

**Changes**:
- Extract lines 977-979 (archive conditional) into pure function:
  ```rust
  /// Pure function: Calculate archived events count based on policy
  fn calculate_archive_count(
      events_to_archive: usize,
      should_archive: bool,
  ) -> usize {
      if should_archive {
          events_to_archive
      } else {
          0
      }
  }
  ```
- This simplifies the loop and makes the logic explicitly testable
- Add corresponding unit tests

**Testing**:
- Test with `should_archive = true`
- Test with `should_archive = false`
- Test with zero events
- Test with non-zero events

**Success Criteria**:
- [ ] Pure function has 100% test coverage
- [ ] Main function complexity reduced by ~1 point
- [ ] Logic is more explicit and readable
- [ ] All tests pass

### Phase 5: Final Coverage Push and Documentation

**Goal**: Achieve 80%+ coverage and add comprehensive documentation

**Changes**:
- Add property-based tests for edge cases
- Add tests for error paths (IO errors, invalid paths)
- Add doc comments explaining the retention analysis flow
- Update existing comments to reflect refactored structure
- Run `cargo tarpaulin` to verify final coverage

**Testing**:
- Run full test suite: `cargo test`
- Run coverage: `cargo tarpaulin --lib`
- Verify coverage >= 80% for `analyze_retention_targets`
- Run clippy: `cargo clippy`
- Run formatter: `cargo fmt --check`

**Success Criteria**:
- [ ] 80%+ coverage achieved
- [ ] All error paths tested
- [ ] Comprehensive documentation added
- [ ] No clippy warnings
- [ ] All formatting passes
- [ ] Ready for production

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run phase-specific tests to verify new functionality

**Final verification**:
1. `cargo test` - All tests pass
2. `cargo tarpaulin --lib --exclude-files 'src/testing/*'` - Verify 80%+ coverage
3. `cargo clippy` - No warnings
4. `cargo fmt --check` - Proper formatting

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure and error messages
3. Identify root cause (test logic, async handling, filesystem mocking)
4. Adjust approach:
   - Consider using `tempfile` crate for filesystem tests
   - Use `#[tokio::test]` for async tests
   - Mock filesystem operations if needed
5. Retry with corrected approach

## Notes

**Key Challenges**:
- Async testing requires `tokio::test` attribute
- Filesystem operations may need mocking or temp directories
- Integration with `RetentionManager` requires careful setup
- Must preserve existing behavior while adding tests

**Dependencies**:
- `tokio` for async test runtime
- Potentially `tempfile` for temporary test directories
- Existing test infrastructure in `src/testing/mocks/`

**Related Functions**:
- `get_job_directories` (line 995) - Already a helper function
- `find_event_files` (line 273) - Already a helper function
- `RetentionManager` - External dependency to mock/test with

**Coverage Baseline**:
- Current: 0% (22 uncovered lines: 953, 960, 962-965, 967-968, 970-978, 984-987, 991)
- Target: 80%+
- Gap: 22 lines need coverage
