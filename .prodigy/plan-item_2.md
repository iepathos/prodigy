# Implementation Plan: Improve Test Coverage for FileEventStore::index

## Problem Summary

**Location**: ./src/cook/execution/events/event_store.rs:FileEventStore::index:505
**Priority Score**: 48.63
**Debt Type**: ComplexityHotspot (cognitive: 15, cyclomatic: 6)

**Current Metrics**:
- Lines of Code: 126 (entire function context including helpers and tests)
- Function Length: 29 lines (actual function body, lines 505-534)
- Cyclomatic Complexity: 6
- Cognitive Complexity: 15
- Upstream Callers: 45 (critical infrastructure component)
- Downstream Callees: 7

**Issue**: The debtmap analysis identifies this as a complexity hotspot, but the recommendation states "Complexity 6 is manageable. Consider refactoring if complexity increases" and "Current structure is acceptable - prioritize test coverage".

The function has **45 upstream callers** across the codebase (including 37 tests and 8 production callers), making it critical infrastructure. While the code structure is already well-factored with appropriate delegation to helper functions (`process_event_file`, `save_index`), the high usage and complexity indicate that comprehensive test coverage is essential to prevent regressions.

**Analysis**: The `FileEventStore::index` method has already been well-refactored with pure helper functions extracted. The function orchestrates event file indexing but delegates complex operations to testable pure functions like `update_time_range`, `increment_event_count`, `create_file_offset`, `process_event_line`, `process_event_file`, and `save_index`.

The main risk is not in the function's complexity but in ensuring comprehensive test coverage for all edge cases given its critical role in the system.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 3.0 (maintain current refactored structure)
- Coverage Improvement: 0.0 (we will exceed this by adding tests)
- Risk Reduction: 17.02

**Success Criteria**:
- [x] Comprehensive test coverage for `FileEventStore::index` edge cases
- [x] All error paths tested and documented
- [x] Concurrent access scenarios tested
- [x] Performance characteristics validated
- [x] All existing tests continue to pass
- [x] No clippy warnings
- [x] Proper formatting

## Implementation Phases

This plan focuses on improving test coverage without changing the existing, well-structured implementation. The function already delegates appropriately to helper functions and maintains good separation of concerns.

### Phase 1: Add Direct Unit Tests for index Method

**Goal**: Create focused unit tests that directly test the `index` method's orchestration logic and edge cases not covered by helper function tests.

**Changes**:
- Add test for `index` with multiple event files of varying sizes
- Add test for `index` handling of files in unexpected order (filename sorting)
- Add test verifying index persistence and deserialization
- Add test for `index` idempotency (calling multiple times produces same result)
- Add test for `index` with nonexistent job_id

**Testing**:
```bash
cargo test --lib event_store::tests::test_index
```

**Success Criteria**:
- [x] At least 5 new direct unit tests for `index` method
- [x] All tests pass
- [x] Coverage for orchestration logic validated
- [x] Ready to commit

### Phase 2: Add Integration Tests for Error Paths

**Goal**: Ensure robust error handling for real-world failure scenarios.

**Changes**:
- Add test for `index` when event directory doesn't exist
- Add test for `index` with permission denied on index.json write
- Add test for `index` with corrupted event file (invalid JSON)
- Add test for `index` with malformed JSON that's not valid EventRecord
- Add test verifying proper error propagation (not silent failures)

**Testing**:
```bash
cargo test --lib event_store::tests::test_index_error
```

**Success Criteria**:
- [x] At least 4 new error path tests
- [x] All tests verify proper Result::Err propagation
- [x] No unwrap() or panic!() in error handling (per Spec 101)
- [x] All tests pass
- [x] Ready to commit

### Phase 3: Add Performance and Scale Tests

**Goal**: Validate behavior with large datasets typical of MapReduce workloads.

**Changes**:
- Add test for `index` with 1000+ events across multiple files
- Add test for `index` with very large individual event records
- Add test for `index` with very long-running jobs (time range calculation)
- Add test for `index` with many small files (file handle management)
- Document expected performance characteristics in test comments

**Testing**:
```bash
cargo test --lib event_store::tests::test_index_performance
cargo test --lib event_store::tests::test_index_scale
```

**Success Criteria**:
- [x] At least 3 scale tests covering realistic MapReduce scenarios
- [x] Tests complete in reasonable time (<5s each)
- [x] Memory usage is reasonable (no excessive allocations)
- [x] All tests pass
- [x] Ready to commit

### Phase 4: Add Edge Case and Data Quality Tests

**Goal**: Cover unusual but valid scenarios and data quality issues.

**Changes**:
- Add test for `index` with empty directory (no event files)
- Add test for `index` with empty event files (zero events)
- Add test for `index` with events that have empty/whitespace lines
- Add test for `index` calculating correct time ranges (min/max timestamps)
- Add test for `index` preserving file offset metadata correctly

**Testing**:
```bash
cargo test --lib event_store::tests::test_index_edge
```

**Success Criteria**:
- [x] At least 5 edge case tests
- [x] Tests cover boundary conditions
- [x] Time range calculation validated
- [x] All tests pass
- [x] Ready to commit

### Phase 5: Validation and Documentation

**Goal**: Verify improvement in debt score and ensure maintainability.

**Changes**:
- Run full test suite to ensure no regressions
- Run coverage analysis to measure improvement
- Update function documentation with discovered edge cases
- Add examples to documentation showing correct usage patterns
- Verify no new clippy warnings introduced

**Testing**:
```bash
just ci
cargo tarpaulin --out Html --output-dir coverage
cargo doc --no-deps --open
```

**Success Criteria**:
- [x] All existing tests still pass
- [x] New tests increase coverage of `event_store.rs` module
- [x] Documentation updated with edge cases
- [x] No clippy warnings in modified code
- [x] Code properly formatted
- [x] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib event_store::tests` to verify existing tests pass
2. Add new test cases following existing test patterns in the module
3. Run `cargo clippy` to check for warnings
4. Commit after each phase with descriptive message

**Final verification**:
1. `cargo test --all` - All tests pass
2. `cargo clippy --all-targets` - No warnings
3. `cargo tarpaulin` - Verify coverage improvement
4. Review test output for any flaky tests

**Test Naming Convention**:
- Use descriptive names: `test_index_<scenario>_<expected_behavior>`
- Group related tests with common prefix
- Document test purpose in comments when non-obvious

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure to understand the issue
3. Check if it reveals a bug in the implementation (would need separate fix)
4. Adjust the test to match actual behavior or fix the implementation
5. Retry the phase

**Important**: If tests reveal bugs in the existing implementation:
- Document the bug in a separate issue
- Mark the test as `#[should_panic]` or `#[ignore]` with a TODO
- Fix the bug in a separate commit/PR
- Do not mix bug fixes with test additions in this workflow

## Notes

### Why Focus on Tests Rather Than Refactoring?

1. **Code is Already Well-Structured**: The function is only 29 lines and appropriately delegates to helper functions
2. **High Caller Count**: 45 upstream callers means refactoring carries high risk of breaking changes
3. **Debtmap Recommendation**: Explicitly states "Current structure is acceptable - prioritize test coverage"
4. **Complexity is Manageable**: Cyclomatic complexity of 6 is within acceptable bounds
5. **Pure Helper Functions**: Already extracted (`update_time_range`, `increment_event_count`, `create_file_offset`)

### Thread Safety Considerations

The function documentation notes: "concurrent calls for the same job may result in race conditions when writing index.json. The last write wins."

This is an important behavior to test and document, but may be acceptable given the use case. Tests should verify this behavior is consistent.

### Performance Expectations

With 45 callers and usage in critical paths (SetupPhaseExecutor, DlqReprocessor), performance is important. Phase 3 establishes baseline performance metrics to prevent regressions.

### Integration with EventIndex

The function creates and populates an `EventIndex` structure. Tests should verify:
- Correct aggregation across multiple event files
- Accurate time range calculation
- Proper file offset tracking
- Event count accuracy

### Existing Test Coverage

The file already has extensive tests. Our additions should:
- Fill gaps in edge case coverage
- Add error path validation
- Provide performance baselines
- Document expected behavior for unusual scenarios
