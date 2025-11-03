# Implementation Plan: Refactor Git Context Module

## Problem Summary

**Location**: ./src/cook/workflow/git_context.rs:file:0
**Priority Score**: 223.88
**Debt Type**: God Object (God Module)
**Current Metrics**:
- Lines of Code: 1695
- Functions: 81 (64 private, 0 public functions; 15 impl methods)
- Cyclomatic Complexity: 485 total, 5.99 average, 33 max
- Coverage: 0% (test code itself)
- Module Components: 16 (enums, structs, impl blocks)
- God Object Score: 1.0 (maximum)

**Issue**: This test-heavy module contains 1695 lines with 81 functions crammed into a single file. The code mixes:
1. Pure utility functions (file status classification, list normalization)
2. Git I/O operations (repository access, diff processing)
3. Core domain logic (change tracking, step management)
4. Test infrastructure (527 lines of test setup code)

The debtmap analysis recommends splitting by data flow into 3 focused modules with <30 functions each.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 97.0 points
- Maintainability Improvement: 22.39 points
- Test Effort: 169.5 lines

**Success Criteria**:
- [ ] Production code split into 3-4 focused modules (<400 lines each)
- [ ] Test code separated into test-only module
- [ ] Pure functions extracted and independently testable
- [ ] Average function complexity reduced below 4.0
- [ ] All 81 existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with cargo fmt

## Implementation Phases

### Phase 1: Extract Pure Utility Functions

**Goal**: Extract all pure utility functions (file status classification, list operations) into a dedicated module

**Changes**:
- Create `src/cook/workflow/git_utils.rs` module
- Extract 11 pure functions:
  - `classify_file_status`, `should_track_as_added`, `should_track_as_modified`, `should_track_as_deleted`
  - `classify_delta_status`, `extract_file_path`
  - `should_add_to_list`, `add_unique_file`
  - `normalize_file_list`, `normalize_file_lists`
  - Plus `FileChangeType` enum
- Update `git_context.rs` to use re-exported functions
- Move 31 related pure function tests to `git_utils.rs` tests

**Testing**:
- Run `cargo test --lib git_utils` to verify extracted functions
- Run `cargo test --lib git_context` to ensure no regressions
- All 31 pure function tests should pass in new module

**Success Criteria**:
- [ ] 11 pure functions extracted to `git_utils.rs` (~120 lines)
- [ ] 31 tests moved and passing (~350 lines)
- [ ] No git2 or I/O dependencies in utils module
- [ ] All tests pass with `cargo test`
- [ ] No clippy warnings

### Phase 2: Extract Git Operations Module

**Goal**: Separate low-level git I/O operations from domain logic

**Changes**:
- Create `src/cook/workflow/git_ops.rs` module
- Extract git I/O functions:
  - `get_head_commit` (currently private in GitChangeTracker)
  - Functions for reading git status
  - Functions for walking commit history
  - Functions for calculating diffs
- These should be focused, single-purpose functions
- Move integration tests that directly test git operations (25 tests) to `git_ops.rs`

**Testing**:
- Run `cargo test --lib git_ops` for git operation tests
- Verify all integration tests pass (those using `init_test_repo`)
- Ensure no test behavior changes

**Success Criteria**:
- [ ] Git I/O operations isolated in `git_ops.rs` (~200 lines)
- [ ] 25 integration tests moved and passing (~750 lines)
- [ ] Clear separation: git_ops handles I/O, git_context handles domain logic
- [ ] All tests pass with `cargo test`
- [ ] No clippy warnings

### Phase 3: Simplify Core Domain Logic

**Goal**: Streamline GitChangeTracker to focus on domain logic, using extracted utilities and operations

**Changes**:
- Refactor `GitChangeTracker` to use functions from `git_utils` and `git_ops`
- Simplify `calculate_step_changes` by delegating to git_ops functions
- Reduce impl method complexity (currently 11 methods)
- Keep only domain logic: step tracking, change aggregation, variable resolution
- Update remaining tests to use new structure

**Testing**:
- Run `cargo test --lib git_context` for domain logic tests
- Verify step tracking tests still pass
- Test variable resolution and change aggregation

**Success Criteria**:
- [ ] `git_context.rs` reduced to ~400 lines (from 1695)
- [ ] GitChangeTracker focused on domain logic only
- [ ] Average complexity per function < 4.0 (down from 5.99)
- [ ] 25 domain logic tests remain and pass
- [ ] All tests pass with `cargo test`
- [ ] No clippy warnings

### Phase 4: Module Integration and Structure

**Goal**: Ensure clean module boundaries and proper visibility controls

**Changes**:
- Update `src/cook/workflow/mod.rs` to declare new submodules
- Set proper `pub` visibility for exported functions/types
- Keep internal helpers private with `pub(crate)` where needed
- Ensure `git_context` is the public API, re-exporting what's needed
- Document module responsibilities in module-level docs

**Testing**:
- Run full test suite: `cargo test --lib`
- Verify no breaking changes to public API
- Test from external crates that all exports work

**Success Criteria**:
- [ ] Module structure properly declared in `mod.rs`
- [ ] Clean public API maintained
- [ ] Private helpers properly scoped
- [ ] All 81 tests pass unchanged
- [ ] Full test suite passes: `cargo test`
- [ ] No clippy warnings

### Phase 5: Final Verification and Documentation

**Goal**: Verify improvements and document the new structure

**Changes**:
- Run `just ci` to verify all CI checks pass
- Generate coverage report with `cargo tarpaulin` (if available)
- Update module-level documentation to reflect new structure
- Add inline docs explaining module responsibilities
- Verify all metrics have improved

**Testing**:
- `cargo test --all` - All tests pass
- `cargo clippy --all-targets` - No warnings
- `cargo fmt -- --check` - Properly formatted
- `just ci` - Full CI passes

**Success Criteria**:
- [ ] All 81 tests passing
- [ ] Coverage maintained or improved
- [ ] Module count: 4 (up from 1 god module)
- [ ] Average file size: ~400 lines (down from 1695)
- [ ] Average complexity: <4.0 per function (down from 5.99)
- [ ] Zero clippy warnings
- [ ] Documentation updated

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run phase-specific module tests
4. Commit with descriptive message

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin` - Verify coverage unchanged
3. Manual review of module structure and boundaries

## Module Structure (Target)

After completion, the git context system will have this structure:

```
src/cook/workflow/
├── git_context.rs        (~400 lines)  - Public API, domain logic, GitChangeTracker
├── git_utils.rs          (~120 lines)  - Pure utility functions
├── git_ops.rs            (~200 lines)  - Git I/O operations
└── mod.rs                (updated)     - Module declarations
```

With tests distributed:
- `git_context.rs` tests: ~200 lines (domain logic tests)
- `git_utils.rs` tests: ~350 lines (pure function tests)
- `git_ops.rs` tests: ~750 lines (integration tests)

Total: ~1600 lines across 3 focused modules vs 1695 lines in one god module

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures or errors
3. Adjust the extraction strategy
4. Ensure no circular dependencies between modules
5. Retry with smaller changes

## Notes

**Key Principles**:
- Pure functions first (easiest to extract and test)
- I/O operations next (clear boundary)
- Domain logic last (depends on other two)
- Never break existing tests - they verify correctness

**Potential Gotchas**:
- Test helper `init_test_repo` is used by many tests - keep it accessible
- Some functions are used in closures (e.g., in `diff.foreach`) - ensure they're accessible
- The `normalize_file_lists` function operates on `StepChanges` - may need to stay in git_context
- Private functions currently used only in tests should be reevaluated

**Performance Considerations**:
- No performance impact expected - only reorganizing code
- All functions remain inline-able by compiler
- No additional allocations or indirections

**Dependency Analysis**:
- `git_utils`: No external dependencies (pure functions)
- `git_ops`: Depends on git2, anyhow - I/O layer
- `git_context`: Depends on git_utils, git_ops - domain layer
