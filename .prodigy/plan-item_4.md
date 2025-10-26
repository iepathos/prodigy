# Implementation Plan: Add Tests and Refactor CLIProgressViewer::display

## Problem Summary

**Location**: ./src/cook/execution/progress.rs:CLIProgressViewer::display:982
**Priority Score**: 18.655
**Debt Type**: TestingGap (0% coverage, complexity 14, cognitive complexity 70)
**Current Metrics**:
- Lines of Code: 44
- Cyclomatic Complexity: 14
- Cognitive Complexity: 70
- Coverage: 0%
- Nesting Depth: 4

**Issue**: Complex async business logic with 100% coverage gap. Cyclomatic complexity of 14 requires at least 14 test cases for full path coverage. The function has deeply nested conditionals (sampler presence check, should_sample logic, completion check) and mixes async control flow with rendering logic.

## Target State

**Expected Impact**:
- Complexity Reduction: 4.2 (from 14 to ~10)
- Coverage Improvement: 50% (from 0% to 50%+)
- Risk Reduction: 7.835

**Success Criteria**:
- [ ] Coverage of CLIProgressViewer::display reaches 50%+ (currently 0%)
- [ ] At least 8 tests covering major execution paths
- [ ] Complex conditional logic extracted into testable pure functions
- [ ] Complexity per function ≤5 (down from 14)
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Pure Decision Logic

**Goal**: Extract the complex conditional logic for testability without changing behavior

**Changes**:
- Extract `should_use_cached_render` function to determine if cached data should be used
- Extract `is_job_complete` function to check completion status
- Move logic into pure functions that can be easily unit tested
- Keep the async display loop intact but call extracted functions

**Testing**:
- Add unit tests for `should_use_cached_render` with various sampler states
- Add unit tests for `is_job_complete` with different metrics
- Run `cargo test --lib` to verify existing tests pass

**Success Criteria**:
- [ ] Pure functions extracted with complexity ≤3 each
- [ ] New functions have 100% test coverage
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Add Tests for Main Display Loop (Sampler Path)

**Goal**: Cover the sampler-enabled execution path with integration tests

**Changes**:
- Create test utilities for mocking ProgressSampler and EnhancedProgressTracker
- Add test: `test_display_with_sampler_should_sample` - covers lines 989-994
- Add test: `test_display_with_sampler_use_cached` - covers lines 997-1000
- Add test: `test_display_with_sampler_completion` - covers lines 1016-1020

**Testing**:
- Each test verifies correct rendering calls and completion detection
- Use tokio::time::pause for deterministic interval testing
- Run `cargo test --lib progress` to verify coverage

**Success Criteria**:
- [ ] 3+ tests for sampler path
- [ ] Lines 989-1000 covered
- [ ] All tests pass
- [ ] Coverage >30%

### Phase 3: Add Tests for Display Loop (Non-Sampler Path)

**Goal**: Cover the non-sampler execution path with integration tests

**Changes**:
- Add test: `test_display_without_sampler_renders_all` - covers lines 1004-1012
- Add test: `test_display_completion_check` - covers lines 1016-1020
- Add test: `test_display_interval_timing` - verifies update interval behavior

**Testing**:
- Verify render methods called correctly without sampler
- Confirm completion detection works
- Run `cargo test --lib progress` to verify coverage

**Success Criteria**:
- [ ] 3+ tests for non-sampler path
- [ ] Lines 1004-1012, 1016-1020 covered
- [ ] All tests pass
- [ ] Coverage >50%

### Phase 4: Extract Render Decision Logic

**Goal**: Further reduce complexity by extracting render decision logic

**Changes**:
- Extract `determine_render_strategy` function that returns enum: `RenderStrategy::Full | Cached | Skip`
- Move the nested if-else sampler logic into this pure function
- Simplify the main display loop to use the strategy pattern
- Add comprehensive tests for all render strategies

**Testing**:
- Test all combinations: sampler present/absent, should_sample true/false
- Verify strategy selection logic
- Run `cargo test --lib` to verify no regressions

**Success Criteria**:
- [ ] Render decision extracted to pure function
- [ ] Main loop simplified (complexity ≤8)
- [ ] New function has 100% coverage
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Add Edge Case and Error Path Tests

**Goal**: Reach target coverage and handle edge cases

**Changes**:
- Add test: `test_display_early_completion` - job complete on first tick
- Add test: `test_display_empty_metrics` - zero items scenario
- Add test: `test_display_render_errors` - handle rendering failures gracefully
- Add test: `test_display_sampler_cache_miss` - cached data unavailable

**Testing**:
- Cover edge cases and error paths
- Run `cargo tarpaulin --lib` to measure final coverage
- Verify target coverage achieved

**Success Criteria**:
- [ ] 4+ edge case tests added
- [ ] Error paths covered
- [ ] Total coverage ≥50%
- [ ] All tests pass
- [ ] Complexity ≤10 overall
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib progress` to verify new tests pass
2. Run `cargo test --lib` to ensure no regressions
3. Run `cargo clippy` to check for warnings
4. Run `cargo fmt` to ensure formatting

**Test organization**:
- Add tests to `src/cook/execution/progress_tests.rs`
- Follow existing test patterns with MockWriter and tokio::test
- Use `Arc<EnhancedProgressTracker>` for realistic async testing

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib -- progress` - Verify coverage improvement
3. Compare before/after metrics

**Coverage targets**:
- Phase 2: >30% coverage
- Phase 3: >50% coverage
- Phase 5: ≥50% coverage (final goal)

## Rollback Plan

If a phase fails:
1. Review test failures and error messages
2. Use `git diff` to review changes
3. If stuck after 2 attempts:
   - Document the failure
   - Consider alternative approach
   - Revert with `git reset --hard HEAD~1` if needed
4. Adjust plan and retry or move to next phase

## Notes

**Key Insights**:
- The display function is an async loop with complex conditional logic
- Main complexity sources: sampler presence check, should_sample logic, completion detection
- Nesting depth of 4 makes testing challenging without extraction
- Existing test patterns use MockWriter and tokio::test - follow these

**Testing Challenges**:
- Async function requires tokio runtime
- Need to mock ProgressSampler and EnhancedProgressTracker
- Interval timing requires tokio::time manipulation
- Completion detection needs careful metrics setup

**Refactoring Strategy**:
- Extract pure decision logic first (easier to test)
- Add integration tests for async loop behavior
- Keep I/O (rendering) separate from logic
- Use strategy pattern to simplify conditional complexity

**Dependencies to Mock**:
- `EnhancedProgressTracker` - provides metrics and snapshots
- `ProgressSampler` - controls sampling behavior
- `tokio::time::Interval` - controls update timing

**Lines to Cover (Priority Order)**:
1. Lines 982-983, 986 - Function start and interval tick
2. Lines 989-994 - Sampler should_sample path
3. Lines 997-1000, 1002 - Cached render path
4. Lines 1005, 1008-1012 - Non-sampler render path
5. Lines 1016-1019, 1023 - Completion check and exit
