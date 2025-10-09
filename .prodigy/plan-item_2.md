# Implementation Plan: Improve Test Coverage for FileEventStore::index

## Problem Summary

**Location**: ./src/cook/execution/events/event_store.rs:FileEventStore::index:389
**Priority Score**: 48.3875
**Debt Type**: ComplexityHotspot (cognitive: 15, cyclomatic: 6)
**Current Metrics**:
- Lines of Code: 30
- Cyclomatic Complexity: 6
- Cognitive Complexity: 15
- Upstream Dependencies: 31 (heavily used throughout codebase)
- Downstream Dependencies: 7
- Function Role: PureLogic

**Issue**: While the cyclomatic complexity (6) is within acceptable limits, the high cognitive complexity (15) and extensive dependency network (31 upstream callers) indicate this is a critical function that needs comprehensive test coverage. The debtmap analysis recommends: "Current structure is acceptable - prioritize test coverage" and "Complexity 6 is manageable. Focus on maintaining simplicity".

**Analysis**: The `FileEventStore::index` method has already been well-refactored with pure helper functions extracted (lines 186-287). The function orchestrates event file indexing but delegates complex operations to testable pure functions like `update_time_range`, `increment_event_count`, `create_file_offset`, `process_event_line`, `process_event_file`, and `save_index`. All helper functions already have unit tests (lines 1116-1455).

The main risk is not in the function's complexity but in ensuring comprehensive test coverage for all edge cases given its critical role in the system.

## Target State

**Expected Impact**:
- Complexity Reduction: 3.0 (maintain current refactored structure)
- Coverage Improvement: 0.0 (focus on comprehensive testing, not structural changes)
- Risk Reduction: 16.94

**Success Criteria**:
- [ ] Comprehensive test coverage for all edge cases of `FileEventStore::index`
- [ ] Tests verify error handling paths (I/O failures, invalid paths)
- [ ] Tests validate integration between `index` and its helper functions
- [ ] All existing tests continue to pass (31 existing tests)
- [ ] No clippy warnings
- [ ] Proper formatting (cargo fmt)
- [ ] Maintain existing pure function structure (no unnecessary refactoring)

## Implementation Phases

This plan focuses on improving test coverage without changing the well-structured implementation. The function already follows functional programming principles with extracted pure functions.

### Phase 1: Add Direct Unit Tests for index Method

**Goal**: Create focused unit tests that directly test the `index` method's orchestration logic and edge cases not covered by helper function tests.

**Changes**:
- Add test for `index` with multiple event files of varying sizes
- Add test for `index` handling of concurrent file system changes (defensive)
- Add test for `index` with files in unexpected order (filename sorting)
- Add test verifying index persistence after creation
- Add test for `index` idempotency (calling multiple times produces same result)

**Testing**:
```bash
cargo test --lib event_store::tests::test_index
cargo test --lib FileEventStore::index
```

**Success Criteria**:
- [ ] At least 5 new direct unit tests for `index` method
- [ ] All tests pass
- [ ] Coverage for orchestration logic validated
- [ ] Ready to commit

### Phase 2: Add Integration Tests for Error Paths

**Goal**: Ensure robust error handling for real-world failure scenarios.

**Changes**:
- Add test for `index` when event file is deleted mid-read (I/O error)
- Add test for `index` with permission denied on index.json write
- Add test for `index` with disk full scenario (simulated via temp filesystem limits)
- Add test for `index` with corrupted event file (partial line read)
- Add test verifying proper error propagation (not silent failures)

**Testing**:
```bash
cargo test --lib event_store::tests::test_index_error
```

**Success Criteria**:
- [ ] At least 4 new error path tests
- [ ] All tests verify proper Result::Err propagation
- [ ] No unwrap() or panic!() in error handling (per Spec 101)
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Add Performance and Scale Tests

**Goal**: Validate behavior with large datasets typical of MapReduce workloads.

**Changes**:
- Add test for `index` with 1000+ events across 10+ files
- Add test for `index` with very large individual event records (>1MB)
- Add test for `index` with very long-running jobs (time range > 24 hours)
- Add benchmark for `index` operation (optional, for baseline)
- Document expected performance characteristics in test comments

**Testing**:
```bash
cargo test --lib event_store::tests::test_index_scale
cargo test --lib event_store::tests::test_index_performance
```

**Success Criteria**:
- [ ] At least 3 scale tests covering realistic MapReduce scenarios
- [ ] Tests complete in reasonable time (<5s each)
- [ ] Memory usage is reasonable (no excessive allocations)
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Add Documentation and Examples

**Goal**: Improve maintainability through clear documentation and usage examples.

**Changes**:
- Add comprehensive doc comments to `FileEventStore::index` method
- Document expected behavior for edge cases (empty directories, no events, etc.)
- Add usage examples in doc comments
- Document error conditions and return values
- Add links to related helper functions in documentation

**Testing**:
```bash
cargo doc --no-deps --open
cargo test --doc event_store
```

**Success Criteria**:
- [ ] Doc comments added with examples
- [ ] All doc examples compile and run
- [ ] Documentation covers all parameters and return values
- [ ] Links to helper functions included
- [ ] Ready to commit

### Phase 5: Validation and Metrics

**Goal**: Verify improvement in debt score and overall code quality.

**Changes**:
- Run full test suite to ensure no regressions
- Run coverage analysis to measure improvement
- Run debtmap to verify reduction in debt score
- Verify no new clippy warnings introduced
- Confirm all formatting standards met

**Testing**:
```bash
just ci
cargo tarpaulin --out Html --output-dir coverage
debtmap analyze --output /tmp/debtmap-after.json
```

**Success Criteria**:
- [ ] All 31+ existing tests still pass
- [ ] New tests increase coverage of `event_store.rs` module
- [ ] Debtmap shows improvement in unified score for this item
- [ ] No clippy warnings in modified code
- [ ] Code properly formatted
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib event_store` to verify new tests pass
2. Run `cargo test --lib` to ensure no regressions
3. Run `cargo clippy -- -D warnings` to check for warnings
4. Run `cargo fmt --check` to verify formatting
5. Review test output for clear failure messages

**Final verification**:
1. `just ci` - Full CI checks (build, test, clippy, format)
2. `cargo tarpaulin --out Html` - Verify coverage improvement
3. `debtmap analyze` - Verify debt score reduction
4. Review all test names for clarity and maintainability

**Test Naming Convention**:
- Use descriptive names: `test_index_<scenario>_<expected_behavior>`
- Group related tests with common prefix
- Document test purpose in comments when non-obvious

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure cause:
   - Test flakiness? Add stability improvements
   - Test design issue? Redesign test approach
   - Actual bug found? Fix bug separately, then return to testing
3. Adjust the plan based on learnings
4. Retry the phase with improvements

## Notes

**Important Observations**:
- The `FileEventStore::index` method is already well-refactored with pure helper functions
- All helper functions have comprehensive unit tests (11 tests for helpers, 20+ integration tests)
- The main gap is in testing the orchestration logic and error handling of `index` itself
- This is a critical function (31 upstream callers) requiring high reliability
- Focus should be on test coverage, not structural changes, per debtmap recommendation

**No Refactoring Needed**:
- The function follows functional programming principles (pure helpers, clear separation of concerns)
- Cyclomatic complexity of 6 is within acceptable limits
- The high cognitive complexity (15) is due to orchestration, not algorithmic complexity
- Changing the structure would risk breaking 31 dependent call sites

**Testing Focus Areas**:
1. Direct testing of `index` orchestration logic
2. Error handling and propagation
3. Edge cases (empty dirs, malformed files, I/O errors)
4. Scale and performance with realistic workloads
5. Documentation and examples for maintainability

**Success Definition**:
- Comprehensive test coverage for all edge cases
- Clear error handling validated through tests
- No structural changes to the well-designed implementation
- Improved confidence in this critical system component
