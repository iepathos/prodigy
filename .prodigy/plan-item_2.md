# Implementation Plan: Test and Refactor calculate_step_changes

## Problem Summary

**Location**: ./src/cook/workflow/git_context.rs:GitChangeTracker::calculate_step_changes:205
**Priority Score**: 20.615111111111112
**Debt Type**: TestingGap { cognitive: 110, coverage: 0.28888888888888886, cyclomatic: 32 }

**Current Metrics**:
- Lines of Code: 120
- Cyclomatic Complexity: 32
- Cognitive Complexity: 110
- Coverage: 28.89% (32 uncovered lines out of ~45 executable lines)
- Nesting Depth: 7

**Issue**: Complex business logic with 72% coverage gap. Cyclomatic complexity of 32 requires at least 32 test cases for full path coverage. The function handles multiple responsibilities:
1. Detecting uncommitted file changes (WT_NEW, WT_MODIFIED, WT_DELETED)
2. Detecting staged file changes (INDEX_NEW, INDEX_MODIFIED, INDEX_DELETED)
3. Walking commit history between last and current commits
4. Calculating diff statistics (insertions, deletions)
5. Processing diff deltas to track file changes from commits
6. Deduplicating and sorting file lists

**Uncovered Lines**: 221, 228, 230-231, 233, 241-242, 245-247, 249-250, 255-256, 258, 261, 267, 273-274, 278-279, 283-284, 288-289, 292, 295, 315-319

## Target State

**Expected Impact**:
- Complexity Reduction: 9.6 (from 32 to ~22)
- Coverage Improvement: 35.56% (from 28.89% to ~64%)
- Risk Reduction: 8.66

**Success Criteria**:
- [ ] Test coverage increased from 28.89% to at least 64%
- [ ] All 32 uncovered lines have test coverage
- [ ] Cyclomatic complexity reduced from 32 to ≤22
- [ ] Cognitive complexity reduced from 110 to ≤70
- [ ] At least 22 pure functions extracted with complexity ≤3 each
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Tests for Uncommitted Changes Detection

**Goal**: Cover lines 221, 228, 230-231, 233 - the basic file status detection paths

**Changes**:
- Add test for files with None path (line 221)
- Add test for WT_MODIFIED status detection (line 228, 230)
- Add test for WT_DELETED status detection (line 231, 233)
- Add test for INDEX_MODIFIED status detection
- Add test for INDEX_DELETED status detection
- Add test for mixed working tree and index changes

**Testing**:
```bash
cargo test --lib git_context_tests
cargo tarpaulin --out Stdout --packages prodigy --lib -- git_context_tests
```

**Success Criteria**:
- [ ] 6 new tests covering uncommitted change detection
- [ ] Lines 221, 228, 230-231, 233 now covered
- [ ] Coverage increases to ~35%
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Add Tests for Commit History Walking

**Goal**: Cover lines 241-242, 245-247, 249-250 - the commit walking and tracking logic

**Changes**:
- Add test for new commits detected (last != current)
- Add test for revwalk between commits
- Add test for multiple commits between checkpoints
- Add test for commit SHA collection
- Add test for error handling in OID parsing
- Add helper function to create commits in test repo

**Testing**:
```bash
cargo test --lib git_context_tests::test_commit_detection
cargo tarpaulin --out Stdout --packages prodigy --lib -- git_context_tests
```

**Success Criteria**:
- [ ] 5 new tests covering commit history walking
- [ ] Lines 241-242, 245-247, 249-250 now covered
- [ ] Coverage increases to ~50%
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Add Tests for Diff Statistics and File Changes

**Goal**: Cover lines 255-256, 258, 261, 267, 273-274, 278-279, 283-284, 288-289, 292, 295 - diff processing logic

**Changes**:
- Add test for diff tree-to-tree calculation
- Add test for insertions/deletions statistics
- Add test for Delta::Added file tracking (lines 278-279)
- Add test for Delta::Modified file tracking (lines 283-284)
- Add test for Delta::Deleted file tracking (lines 288-289)
- Add test for deduplication of files (lines 307-312)
- Add test for unknown delta status (line 292)
- Add helper to create committed file changes

**Testing**:
```bash
cargo test --lib git_context_tests::test_diff
cargo tarpaulin --out Stdout --packages prodigy --lib -- git_context_tests
```

**Success Criteria**:
- [ ] 7 new tests covering diff processing
- [ ] Lines 255-295 now covered
- [ ] Coverage increases to ~64%
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Pure Functions for Status Detection

**Goal**: Extract pure functions for file status classification, reducing complexity

**Changes**:
- Extract `classify_file_status(status: Status) -> FileChangeType` (complexity ≤3)
- Extract `should_track_as_added(status: Status) -> bool` (complexity ≤2)
- Extract `should_track_as_modified(status: Status) -> bool` (complexity ≤2)
- Extract `should_track_as_deleted(status: Status) -> bool` (complexity ≤2)
- Create enum `FileChangeType { Added, Modified, Deleted, Unknown }`
- Update lines 225-234 to use extracted functions
- Add unit tests for each extracted function (3 tests each = 12 tests)

**Testing**:
```bash
cargo test --lib git_context_tests::test_classify
cargo clippy
```

**Success Criteria**:
- [ ] 4 pure functions extracted
- [ ] 12 new unit tests for extracted functions
- [ ] Cyclomatic complexity reduced by ~6 (from 32 to ~26)
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Extract Pure Functions for Diff Processing

**Goal**: Extract pure functions for diff delta handling, further reducing complexity

**Changes**:
- Extract `classify_delta_status(delta: git2::Delta) -> FileChangeType` (complexity ≤3)
- Extract `extract_file_path(delta: &DiffDelta) -> Option<String>` (complexity ≤2)
- Extract `should_add_to_list(list: &[String], path: &str) -> bool` (complexity ≤2)
- Extract `add_unique_file(list: &mut Vec<String>, path: String)` (complexity ≤2)
- Update diff.foreach callback (lines 272-296) to use extracted functions
- Add unit tests for each extracted function (3 tests each = 12 tests)

**Testing**:
```bash
cargo test --lib git_context_tests::test_delta
cargo clippy
```

**Success Criteria**:
- [ ] 4 pure functions extracted
- [ ] 12 new unit tests for extracted functions
- [ ] Cyclomatic complexity reduced by ~6 (from ~26 to ~20)
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 6: Extract Pure Functions for Commit Walking and Result Processing

**Goal**: Extract remaining pure functions to reach target complexity

**Changes**:
- Extract `collect_commits_between(repo: &Repository, from: Oid, to: Oid) -> Result<Vec<String>>` (complexity ≤4)
- Extract `calculate_diff_stats(repo: &Repository, from: Oid, to: Oid) -> Result<(usize, usize)>` (complexity ≤5)
- Extract `normalize_file_lists(changes: &mut StepChanges)` for sort/dedup (complexity ≤2)
- Extract `merge_file_changes(dest: &mut Vec<String>, source: Vec<String>)` (complexity ≤2)
- Update calculate_step_changes to use extracted functions
- Add unit tests for each extracted function (3-4 tests each = 14 tests)

**Testing**:
```bash
cargo test --lib git_context_tests::test_commit_walking
cargo test --lib git_context_tests::test_normalization
cargo tarpaulin --out Stdout --packages prodigy --lib -- git_context_tests
```

**Success Criteria**:
- [ ] 4 pure functions extracted
- [ ] 14 new unit tests for extracted functions
- [ ] Cyclomatic complexity reduced to ≤20
- [ ] Cognitive complexity reduced to ≤70
- [ ] Coverage reaches or exceeds 64%
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 7: Final Integration and Validation

**Goal**: Ensure all changes integrate correctly and meet success criteria

**Changes**:
- Review all extracted functions for consistency
- Add integration tests for full calculate_step_changes workflow
- Add property-based tests for edge cases (empty repos, no changes, etc.)
- Update documentation/comments for extracted functions
- Run full CI suite

**Testing**:
```bash
just ci
cargo tarpaulin --out Stdout --packages prodigy --lib
debtmap analyze --output .prodigy/debt-after-fix.json
```

**Success Criteria**:
- [ ] Full CI passes
- [ ] Coverage ≥64% confirmed
- [ ] Cyclomatic complexity ≤20 confirmed
- [ ] Cognitive complexity ≤70 confirmed
- [ ] All 23 new tests pass
- [ ] Debtmap shows improvement in unified score
- [ ] No regressions in existing functionality
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Write tests first to establish expected behavior
2. Run `cargo test --lib git_context_tests` to verify tests
3. For refactoring phases, ensure tests pass before AND after extraction
4. Run `cargo clippy` to check for warnings
5. Run `cargo fmt` to ensure formatting

**Coverage verification**:
```bash
# After each test-adding phase (1-3)
cargo tarpaulin --out Stdout --packages prodigy --lib -- git_context_tests

# After each refactoring phase (4-6)
cargo tarpaulin --out Stdout --packages prodigy --lib -- git_context_tests
cargo clippy
```

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --out Stdout --packages prodigy --lib` - Full coverage
3. Compare before/after debtmap results

## Rollback Plan

If a phase fails:
1. Review the test failure or compilation error
2. Use `git diff` to review changes
3. If the issue is fixable quickly (< 10 minutes), fix and retry
4. If the issue requires rethinking, use `git restore .` to revert working tree
5. Re-evaluate the approach for that phase
6. Adjust the plan if needed

For committed phases that introduce regressions:
1. `git revert HEAD` to undo the problematic commit
2. Analyze the regression
3. Fix the issue in a new commit
4. Continue with the plan

## Notes

**Key Insights**:
- This function mixes I/O (git operations) with business logic (status classification)
- The nesting depth of 7 comes from nested if-let and match statements
- Many branches are uncovered because tests don't create committed changes, only staged changes
- The function is actually doing 3 distinct things: detect uncommitted changes, detect committed changes, normalize results

**Testing Challenges**:
- Need to create actual git commits in tests (not just staged changes)
- Need to simulate revwalks between commits
- Need to handle None paths in status entries
- Tests must use real git2 Repository objects

**Refactoring Strategy**:
- Test first, refactor second (phases 1-3 add tests, phases 4-6 refactor)
- Extract pure functions that don't touch Repository objects when possible
- Keep git2-dependent code in thin wrapper functions
- Each extracted function should have ≤3 cyclomatic complexity
- Aim for ~22 functions total (current 1 → 22 = 21 extracted functions)

**Expected Function Breakdown** (after all phases):
1. `calculate_step_changes` - Main orchestrator (complexity ~8)
2. `classify_file_status` - Status classification (complexity 3)
3. `should_track_as_added` - Added check (complexity 2)
4. `should_track_as_modified` - Modified check (complexity 2)
5. `should_track_as_deleted` - Deleted check (complexity 2)
6. `classify_delta_status` - Delta classification (complexity 3)
7. `extract_file_path` - Path extraction (complexity 2)
8. `should_add_to_list` - Dedup check (complexity 2)
9. `add_unique_file` - Add with dedup (complexity 2)
10. `collect_commits_between` - Commit walking (complexity 4)
11. `calculate_diff_stats` - Diff statistics (complexity 5)
12. `normalize_file_lists` - Sort/dedup (complexity 2)
13. `merge_file_changes` - Merge lists (complexity 2)

Total complexity: 8+3+2+2+2+3+2+2+2+4+5+2+2 = 41 across 13 functions
Average per function: ~3.15
Main function: 8 (down from 32)

This achieves the goal of reducing complexity while maintaining functionality and dramatically improving testability.
