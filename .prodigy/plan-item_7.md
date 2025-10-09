# Implementation Plan: Add Tests and Extract Pure Functions for analyze_retention_targets

## Problem Summary

**Location**: ./src/cli/events/mod.rs:analyze_retention_targets:934
**Priority Score**: 31.55
**Debt Type**: TestingGap (0% direct coverage, 33.3% transitive)

**Current Metrics**:
- Lines of Code: 42
- Cyclomatic Complexity: 15
- Cognitive Complexity: 45
- Direct Coverage: 0%
- Transitive Coverage: 33.3%
- Nesting Depth: 5
- Uncovered Lines: 23 lines across multiple ranges (934, 941, 943-947, 949-950, 952-960, 966-969, 973)

**Issue**: Complex business logic with 100% coverage gap. This function has 15 decision branches requiring at least 15 test cases for full path coverage. It orchestrates retention analysis across global and local storage with complex nested loops and conditional logic. The high cognitive complexity (45) makes it difficult to understand and maintain.

**Rationale**: Testing before refactoring ensures no regressions. After extracting 9 functions, each will need only 3-5 tests instead of 15 tests for the monolithic function.

## Target State

**Expected Impact**:
- Complexity Reduction: 4.5 (from 15 to ~10-11 cyclomatic)
- Coverage Improvement: 50% (from 0% to 50%+)
- Risk Reduction: 13.251 points

**Success Criteria**:
- [ ] Direct test coverage increases from 0% to 80%+
- [ ] Extract 9 pure functions with complexity ≤3 each
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with cargo fmt
- [ ] Each extracted function has 3-5 unit tests

## Implementation Phases

### Phase 1: Add Comprehensive Test Coverage

**Goal**: Establish baseline test coverage for all execution paths before refactoring

**Changes**:
- Create test module `analyze_retention_targets_tests` with mock setup
- Use existing `MockFileSystem` pattern from codebase
- Write 8 integration tests covering all major branches:
  1. `test_analyze_global_with_all_jobs_flag()` - all_jobs=true with existing directories
  2. `test_analyze_global_with_specific_job_id()` - job_id=Some("job-123")
  3. `test_analyze_global_nonexistent_directory()` - global_events_dir doesn't exist
  4. `test_analyze_global_empty_job_dirs()` - no job directories found
  5. `test_analyze_local_file_exists()` - neither flag set, local file exists
  6. `test_analyze_local_file_missing()` - neither flag set, no local file
  7. `test_analyze_multiple_event_files_aggregation()` - multiple files across jobs
  8. `test_analyze_with_archive_policy()` - archive_old_events=true vs false

**Testing**:
- Run `cargo test analyze_retention_targets` to verify new tests
- Ensure tests cover all uncovered lines using mock filesystem
- Verify aggregation logic with multiple RetentionAnalysis results

**Success Criteria**:
- [ ] 8+ integration tests written
- [ ] Tests use MockFileSystem for isolation
- [ ] All tests pass
- [ ] Coverage for target function reaches 50%+
- [ ] Ready to commit

### Phase 2: Extract Pure Decision Functions

**Goal**: Extract decision logic into pure, testable functions

**Changes**:
Extract these 3 pure functions at module level:
1. `should_analyze_global_storage(all_jobs: bool, job_id: Option<&str>) -> bool`
   - Simple decision: returns true if all_jobs OR job_id.is_some()
   - Complexity: 1 (single OR condition)

2. `calculate_archive_count(events_to_archive: usize, archive_enabled: bool) -> usize`
   - Returns events_to_archive if enabled, else 0
   - Complexity: 1 (single if statement)

3. `build_global_events_path(repo_name: &str) -> Result<PathBuf>`
   - Pure path construction from repo name
   - Complexity: 2 (error handling)

**Testing**:
- Write 3-5 unit tests per function (12 tests total)
- Test edge cases: empty strings, None values, error conditions
- Run `cargo test --lib` to verify all tests pass

**Success Criteria**:
- [ ] 3 pure functions extracted
- [ ] Each function has complexity ≤2
- [ ] 12 unit tests added
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Aggregation Logic Functions

**Goal**: Extract analysis aggregation into pure, testable functions

**Changes**:
Extract these 3 pure functions:
1. `aggregate_retention_analysis(total: &mut RetentionAnalysis, new: &RetentionAnalysis, archive_enabled: bool)`
   - Aggregates events_to_remove and space_to_save
   - Conditionally adds events_to_archive
   - Complexity: 2 (if statement for archive)

2. `create_default_analysis() -> RetentionAnalysis`
   - Factory function for default analysis
   - Complexity: 1

3. `aggregate_file_analyses(analyses: Vec<RetentionAnalysis>, archive_enabled: bool) -> RetentionAnalysis`
   - Folds multiple analyses into one
   - Uses `aggregate_retention_analysis`
   - Complexity: 2 (fold + conditional)

**Testing**:
- Write unit tests for aggregation with edge cases (9 tests total):
  - Empty input lists
  - Single analysis
  - Multiple analyses with various counts
  - Archive enabled/disabled scenarios
- Run `cargo test --lib` to verify all tests pass

**Success Criteria**:
- [ ] 3 aggregation functions extracted
- [ ] Each function has complexity ≤2
- [ ] 9 unit tests added
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Async I/O Coordination

**Goal**: Separate async I/O coordination from pure logic

**Changes**:
Extract these 3 async functions as private module functions:
1. `async fn analyze_global_job_directories(job_dirs: Vec<PathBuf>, policy: &RetentionPolicy) -> Result<RetentionAnalysis>`
   - Iterates job_dirs, finds event files, analyzes each
   - Uses pure aggregation functions from Phase 3
   - Complexity: 3 (nested loops + async)

2. `async fn analyze_global_events(global_events_dir: &Path, job_id: Option<&str>, policy: &RetentionPolicy) -> Result<RetentionAnalysis>`
   - Gets job directories, delegates to analyze_global_job_directories
   - Complexity: 2 (error handling)

3. `async fn analyze_local_events(policy: &RetentionPolicy) -> Result<RetentionAnalysis>`
   - Checks local file, creates RetentionManager, analyzes
   - Complexity: 2 (file existence check)

**Refactor main function**:
- `analyze_retention_targets` becomes thin orchestrator
- Uses `should_analyze_global_storage()` to decide path
- Delegates to either `analyze_global_events()` or `analyze_local_events()`
- Reduced complexity from 15 to ~5

**Testing**:
- Update integration tests to verify async behavior
- Add tests for error handling in I/O functions
- Test main function orchestration logic

**Success Criteria**:
- [ ] 3 async I/O functions extracted
- [ ] Main function complexity reduced to ≤5
- [ ] All async operations tested
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Final Validation and Cleanup

**Goal**: Achieve 80%+ coverage and ensure code quality

**Changes**:
- Add final edge case tests:
  1. Test with filesystem errors (permission denied, etc.)
  2. Test with corrupted event files
  3. Test aggregation with large numbers (overflow scenarios)
- Run full coverage analysis: `cargo tarpaulin --lib`
- Fix any clippy warnings: `cargo clippy`
- Format code: `cargo fmt`
- Update function documentation with examples
- Add module-level documentation explaining the refactoring

**Testing**:
- `cargo test --lib` - All tests pass
- `cargo clippy` - No warnings
- `cargo fmt --check` - Properly formatted
- `cargo tarpaulin --lib` - Coverage ≥80% for target function

**Success Criteria**:
- [ ] Coverage for `analyze_retention_targets` ≥80%
- [ ] Total of 9 extracted functions
- [ ] Each extracted function complexity ≤3
- [ ] 29+ new tests added (8 integration + 21 unit)
- [ ] No clippy warnings
- [ ] Properly formatted
- [ ] Documentation updated
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Write tests BEFORE extracting (Phase 1) to establish baseline
2. Extract functions while tests are passing (Phases 2-4)
3. Add unit tests for each extracted function (Phases 2-4)
4. Run `cargo test --lib` after each extraction
5. Verify no regressions in existing tests

**Test Patterns**:
- Use `MockFileSystem` for all file operations
- Mock `RetentionManager::analyze_retention()` to return controlled results
- Test both success and error paths
- Cover edge cases: empty inputs, missing files, permission errors

**Final verification**:
1. `cargo test --lib` - All tests pass
2. `cargo clippy` - No warnings
3. `cargo tarpaulin --lib` - Coverage ≥80%
4. `debtmap analyze` - Verify complexity reduction

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure:
   - Check test output for specific errors
   - Run `cargo clippy` to identify issues
   - Verify mock setup is correct
3. Adjust the plan:
   - If tests fail: Fix edge cases or mock setup
   - If complexity too high: Split function further
   - If integration breaks: Verify async boundary
4. Retry the phase with adjustments

**Common failure scenarios**:
- **Mock setup issues**: Ensure MockFileSystem properly implements required traits
- **Async errors**: Verify .await placement and Result propagation
- **Coverage gaps**: Add tests for uncovered branches
- **Type mismatches**: Ensure extracted functions have correct signatures

## Notes

**Key Considerations**:
- Function uses `RetentionManager::analyze_retention()` which is async - maintain boundary
- MockFileSystem from `src/testing/mocks/fs.rs:88` is already used in codebase
- Archive logic depends on `policy.archive_old_events` flag - test both states
- Multiple nested loops create complexity - extract inner logic to reduce nesting
- Path construction is platform-agnostic using PathBuf - no special Windows/Unix handling

**Extraction Strategy**:
1. Pure decision logic first (no I/O, no state)
2. Pure aggregation logic next (data transformation)
3. Async I/O coordination last (maintains async boundary)
4. Main function becomes thin orchestrator using extracted pieces

**Testing Focus**:
- **Branch coverage**: All conditional paths (15 branches total)
- **Edge cases**: Empty collections, missing files, permission errors
- **Policy variations**: Archive enabled/disabled, different retention settings
- **Aggregation correctness**: Multiple files, multiple jobs, sum calculations

**Expected Outcome**:
After all phases:
- 1 simplified orchestration function (~20 lines, complexity ~5)
- 9 extracted functions (pure and async I/O, each ≤3 complexity)
- 29+ comprehensive tests
- 80%+ test coverage
- Significantly improved maintainability and readability
