# Implementation Plan: Improve Test Coverage for list_resumable_jobs_internal

## Problem Summary

**Location**: ./src/cook/execution/state.rs:DefaultJobStateManager::list_resumable_jobs_internal:884
**Priority Score**: 48.102062072615965
**Debt Type**: ComplexityHotspot (Cognitive: 56, Cyclomatic: 10)
**Current Metrics**:
- Lines of Code: 59
- Cyclomatic Complexity: 10
- Cognitive Complexity: 56
- Nesting Depth: 6
- Coverage: 0% (significant gap)

**Issue**: Add 10 tests for 100% coverage gap. NO refactoring needed (complexity 10 is acceptable)

The function `list_resumable_jobs_internal` is responsible for scanning checkpoint directories and identifying resumable MapReduce jobs. While existing tests cover many scenarios, the coverage analysis shows gaps that need to be addressed.

## Target State

**Expected Impact**:
- Complexity Reduction: 5.0 (minimal - focus is on testing, not refactoring)
- Coverage Improvement: 0.0 → 100%
- Risk Reduction: 16.8%

**Success Criteria**:
- [ ] 10 new focused tests added (each < 15 lines)
- [ ] Each test covers ONE specific code path/branch
- [ ] All existing tests continue to pass
- [ ] Test coverage reaches 100% for the function
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Tests are deterministic and fast

## Implementation Phases

### Phase 1: Add Tests for Entry Iteration Edge Cases

**Goal**: Cover edge cases in the directory entry iteration loop (lines 895-938)

**Changes**:
- Add test for multiple job directories with mixed states
- Add test for directory with special characters in name
- Add test for concurrent checkpoint modifications
- Add test for very large number of job directories

**Tests to Add**:
1. `test_list_resumable_multiple_mixed_jobs` - Mix of complete, incomplete, and invalid jobs
2. `test_list_resumable_special_chars_in_name` - Job IDs with hyphens, underscores
3. `test_list_resumable_many_jobs` - Verify performance with 50+ job directories

**Testing**:
- Run `cargo test test_list_resumable_multiple` to verify new tests pass
- Verify existing tests still pass with `cargo test list_resumable`

**Success Criteria**:
- [ ] 3 new tests added
- [ ] All tests pass
- [ ] Coverage increases for loop iteration paths
- [ ] Ready to commit

### Phase 2: Add Tests for Checkpoint State Variations

**Goal**: Cover different checkpoint states and version scenarios

**Changes**:
- Add test for job with no metadata file but valid checkpoint
- Add test for job with metadata but no checkpoint files
- Add test for job with checkpoints but failed list_checkpoints call
- Add test for jobs with different completion states

**Tests to Add**:
4. `test_list_resumable_metadata_missing` - Valid checkpoint but no metadata.json
5. `test_list_resumable_checkpoints_but_metadata_invalid` - Checkpoints exist but metadata is corrupted
6. `test_list_resumable_mixed_checkpoint_versions` - Jobs at different checkpoint versions

**Testing**:
- Run `cargo test test_list_resumable` to verify all variations
- Check that incomplete jobs with various checkpoint states are handled correctly

**Success Criteria**:
- [ ] 3 new tests added
- [ ] All tests pass
- [ ] Coverage increases for checkpoint loading paths
- [ ] Ready to commit

### Phase 3: Add Tests for Data Integrity and Edge Values

**Goal**: Cover edge cases in the data processing and aggregation

**Changes**:
- Add test for job with zero work items
- Add test for job with very high checkpoint version numbers
- Add test for job with partial failure records
- Add test for timestamp boundary conditions

**Tests to Add**:
7. `test_list_resumable_zero_items` - Job with empty work_items list
8. `test_list_resumable_high_checkpoint_version` - Checkpoint version near u32::MAX
9. `test_list_resumable_partial_failures` - Job with some failed agents
10. `test_list_resumable_recent_vs_old_jobs` - Jobs with different updated_at timestamps

**Testing**:
- Run `cargo test test_list_resumable` to verify all edge cases
- Ensure no panics or unexpected behavior with boundary values

**Success Criteria**:
- [ ] 4 new tests added (total 10 new tests)
- [ ] All tests pass
- [ ] Coverage reaches 100% for the function
- [ ] No edge cases cause panics or errors
- [ ] Ready to commit

### Phase 4: Verify Coverage and Finalize

**Goal**: Confirm 100% test coverage and clean up

**Changes**:
- Run coverage analysis with `cargo tarpaulin`
- Identify any remaining uncovered branches
- Add additional tests if needed
- Ensure all tests are documented

**Testing**:
- `cargo test list_resumable` - All tests pass
- `cargo tarpaulin --lib` - Verify 100% coverage of target function
- `cargo clippy` - No warnings
- `cargo fmt --check` - Proper formatting

**Success Criteria**:
- [ ] 100% coverage achieved for `list_resumable_jobs_internal`
- [ ] All 10+ tests passing consistently
- [ ] No clippy warnings
- [ ] Code formatted correctly
- [ ] Tests are fast (< 5s total)
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Write tests one at a time
2. Run `cargo test <test_name>` after each test
3. Verify test passes and covers intended branch
4. Run full test suite to ensure no regressions
5. Commit after each phase

**Test Design Principles**:
- Each test should be < 15 lines (excluding setup)
- Use helper functions `create_unique_temp_dir()` and `create_test_config()` (already in file)
- Test ONE specific branch or condition per test
- Use descriptive test names that explain what's being tested
- Follow existing test patterns in the file

**Coverage Verification**:
```bash
# After each phase, check coverage
cargo tarpaulin --lib --line --ignore-tests \
  --exclude-files 'src/main.rs' \
  --out Stdout | grep 'state.rs'
```

**Final verification**:
```bash
just ci                    # Full CI checks
cargo clippy -- -D warnings # No warnings allowed
cargo test --lib           # All tests pass
```

## Rollback Plan

If a phase fails:
1. Identify which test is failing
2. Review the test logic and the function behavior
3. Adjust the test to match actual function behavior
4. If the function has a bug, document it but DON'T fix it (this is test-only work)
5. If stuck after 3 attempts, commit what works and document the gap

**Important**: This plan focuses ONLY on adding tests. Do NOT refactor the function, even if you see opportunities to improve it.

## Notes

### Why No Refactoring?

The recommendation explicitly states: "NO refactoring needed (complexity 10 is acceptable)". The cyclomatic complexity of 10 is at the threshold of acceptability, and the function's logic is straightforward directory scanning. The high cognitive complexity (56) comes from deep nesting, but since the function is pure logic with clear intent, adding tests is more valuable than restructuring.

### Test Coverage Strategy

The 10 tests should focus on:
- **Boundary conditions**: Empty directories, missing files, edge values
- **Error paths**: Invalid checkpoints, missing metadata, I/O errors
- **State variations**: Complete vs incomplete jobs, different checkpoint versions
- **Data integrity**: Correct aggregation of job information

### Expected Outcomes

After completing this plan:
- 100% branch coverage for `list_resumable_jobs_internal`
- 10+ high-quality, focused tests
- Increased confidence in the function's correctness
- Clear documentation of expected behavior through tests
- Foundation for future refactoring if needed (but not in this plan)
