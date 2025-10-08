# Implementation Plan: Add Test Coverage for FileEventStore::index

## Problem Summary

**Location**: ./src/cook/execution/events/event_store.rs:FileEventStore::index:389
**Priority Score**: 48.3875
**Debt Type**: ComplexityHotspot (Cognitive: 15, Cyclomatic: 6)
**Current Metrics**:
- Lines of Code: 30
- Cyclomatic Complexity: 6
- Cognitive Complexity: 15
- Coverage: 0% (according to debtmap)
- Upstream Dependencies: 31 callers
- Downstream Dependencies: 7 callees

**Issue**: Add 6 tests for 100% coverage gap. NO refactoring needed (complexity 6 is acceptable)

**Rationale**: Complexity 6 is manageable. Coverage at 0%. Focus on test coverage, not refactoring. Current structure is acceptable - prioritize test coverage.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 3.0
- Coverage Improvement: 0.0
- Risk Reduction: 16.94

**Success Criteria**:
- [ ] Verify existing tests are running and passing
- [ ] Identify any uncovered code paths in the `index` method
- [ ] Add targeted tests for any missing branches
- [ ] Achieve 100% line and branch coverage for `FileEventStore::index`
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Investigate Coverage Discrepancy

**Goal**: Understand why debtmap reports 0% coverage when extensive tests already exist

**Changes**:
- Run existing tests to verify they pass
- Run `cargo tarpaulin` to get actual coverage metrics for `event_store.rs`
- Identify any genuinely uncovered lines or branches in the `index` method
- Review test configuration to ensure async tests are being counted

**Testing**:
- Run: `cargo test --lib event_store::tests`
- Run: `cargo tarpaulin --out Stdout --skip-clean -- event_store`
- Review coverage report for `FileEventStore::index` specifically

**Success Criteria**:
- [ ] All existing tests pass
- [ ] Actual coverage metrics obtained
- [ ] Gap analysis completed
- [ ] Root cause of 0% coverage identified

### Phase 2: Add Missing Tests for Uncovered Paths

**Goal**: Write focused tests for any genuinely uncovered code paths discovered in Phase 1

**Changes**:
Based on Phase 1 findings, add tests for any missing branches. The function has these potential paths:
- Empty file list → already tested
- Files with events → already tested
- Error when `find_event_files` fails → may need test
- Error when `process_event_file` fails → may need test
- Time range tuple decomposition with no events → already tested
- Error when `save_index` fails → may need test

Each new test should:
- Be < 15 lines
- Test ONE specific path
- Follow existing test patterns in the module
- Use descriptive names like `test_index_handles_xxx_error`

**Testing**:
- Run each new test individually: `cargo test <test_name>`
- Verify it covers the intended branch
- Run full suite: `cargo test event_store::tests`

**Success Criteria**:
- [ ] All new tests pass
- [ ] Each test covers a specific branch
- [ ] Tests follow existing patterns
- [ ] No test is longer than 15 lines
- [ ] Ready to commit

### Phase 3: Verify Complete Coverage

**Goal**: Confirm 100% coverage of the `index` method

**Changes**:
- Run full test suite
- Generate updated coverage report
- Verify all branches of `index` method are covered
- Ensure logging statements are exercised

**Testing**:
- Run: `cargo test --lib`
- Run: `cargo tarpaulin --out Html`
- Review HTML report to verify coverage
- Run: `cargo clippy`
- Run: `cargo fmt --check`

**Success Criteria**:
- [ ] Coverage report shows 100% for `FileEventStore::index`
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code is properly formatted
- [ ] Ready to commit

### Phase 4: Final Validation

**Goal**: Run full CI checks and verify the debt item is resolved

**Changes**:
- Run `just ci` to ensure all checks pass
- Verify the debtmap score has improved

**Testing**:
- Run `just ci` for full CI validation
- Run `cargo tarpaulin` to regenerate coverage report
- Run `debtmap analyze` to verify improvement in debt score

**Success Criteria**:
- [ ] All CI checks pass
- [ ] Coverage report shows improvement
- [ ] Debtmap analysis shows reduced debt score
- [ ] Code is ready for final commit

## Testing Strategy

**For Phase 1**:
1. Run existing tests: `cargo test --lib event_store::tests`
2. Generate coverage: `cargo tarpaulin --skip-clean --out Html`
3. Analyze gaps in coverage report

**For Phase 2**:
1. Write one test at a time
2. Run `cargo test <test_name>` after each
3. Verify it covers the intended branch
4. Move to next gap

**For Phase 3**:
1. Run full test suite: `cargo test --lib`
2. Generate coverage report: `cargo tarpaulin --out Html`
3. Run quality checks: `cargo clippy && cargo fmt --check`
4. Verify metrics improved

**Final verification**:
1. `just ci` - Full CI checks
2. Review coverage report - Confirm 100% coverage for `index` method
3. Run debtmap again to verify score improvement

**Test Structure Pattern** (following existing async patterns in the module):
```rust
#[tokio::test]
async fn test_index_<scenario>() {
    // Setup: Create test directory and store
    // Action: Call index() with specific scenario
    // Assert: Verify expected behavior
}
```

## Rollback Plan

If a phase fails:
1. Identify the failing test or check
2. Review the error message carefully
3. If test is flaky or incorrect, remove it
4. If underlying code has issues, investigate further
5. Can always revert with `git reset --hard HEAD~1`

## Notes

**Important Context**:
- The `index` method is part of the `EventStore` trait implementation
- It's async and uses file I/O operations
- There are already 15+ comprehensive tests for this method
- The "0% coverage" may be a measurement artifact rather than actual lack of tests
- Focus is on verifying coverage, not refactoring (complexity 6 is acceptable)

**Existing Test Coverage** (already present):
- `test_index_creates_index_for_job_with_events` - Happy path
- `test_index_with_no_event_files` - Empty directory
- `test_index_with_nonexistent_job` - Error case
- `test_index_aggregates_multiple_event_files` - Multiple files
- `test_index_calculates_correct_time_range` - Time range logic
- `test_index_handles_malformed_json` - Error handling
- `test_index_persists_and_deserializes_correctly` - Serialization
- `test_index_with_empty_directory_creates_empty_index` - Edge case
- `test_index_fails_when_save_directory_missing` - Error case
- `test_index_with_only_invalid_json_events` - Invalid data
- `test_index_time_range_with_single_event` - Single event
- `test_index_time_range_with_multiple_events` - Multiple events
- `test_index_default_time_range_no_valid_events` - No valid events

**Potential Gaps** (to investigate in Phase 1):
- Async execution paths may not be traced by coverage tools
- Error propagation via `?` operator
- Logging statements (info! macro)
- Early returns vs normal flow

**References**:
- Function location: `src/cook/execution/events/event_store.rs:389`
- Test module: `src/cook/execution/events/event_store.rs::tests`
- Recommendation: Add 6 tests for 100% coverage gap

**NO REFACTORING**: The function has complexity 6, which is acceptable. Focus ONLY on adding tests if needed.
