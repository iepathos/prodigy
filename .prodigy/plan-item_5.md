# Implementation Plan: Add Test Coverage for JsonlEventWriter::write

## Problem Summary

**Location**: ./src/cook/execution/events/event_writer.rs:JsonlEventWriter::write:179
**Priority Score**: 32.44
**Debt Type**: ComplexityHotspot (Cognitive: 17, Cyclomatic: 5)

**Current Metrics**:
- Lines of Code: 19
- Cyclomatic Complexity: 5
- Cognitive Complexity: 17
- Coverage: 0% (no direct tests for this function)
- Function Role: IOWrapper

**Issue**: This function has 0% test coverage despite being called by 40 upstream callers. The complexity (5) is acceptable and the code is already well-structured with extracted helper functions. The primary action is to add 5 focused tests to achieve 100% coverage, NOT to refactor.

## Target State

**Expected Impact**:
- Complexity Reduction: 2.5 (minimal - structure is already good)
- Coverage Improvement: 0.0 → 100% (5 new focused tests)
- Risk Reduction: 11.36 (high-impact due to 40 upstream callers)

**Success Criteria**:
- [ ] 5 new focused tests covering all code paths in `write()` method
- [ ] Each test is <15 lines and tests ONE specific path
- [ ] 100% coverage for `JsonlEventWriter::write` function
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Test for Happy Path - Single Event Write

**Goal**: Test the main success path where events are serialized and written successfully

**Changes**:
- Add test `test_jsonl_writer_write_single_event` that:
  - Creates a JsonlEventWriter
  - Calls `write()` with one event
  - Verifies the file contains the serialized event
  - Verifies size counter is updated correctly

**Testing**:
- Run `cargo test test_jsonl_writer_write_single_event`
- Verify test passes
- Run `cargo tarpaulin --out Stdout -- --test test_jsonl_writer_write_single_event` to check coverage

**Success Criteria**:
- [ ] Test passes
- [ ] Covers the happy path through serialize → write → size update
- [ ] No warnings or errors

### Phase 2: Add Test for Empty Events Array

**Goal**: Test edge case where `write()` is called with an empty events array

**Changes**:
- Add test `test_jsonl_writer_write_empty_array` that:
  - Creates a JsonlEventWriter
  - Calls `write()` with empty array `&[]`
  - Verifies no bytes are written
  - Verifies size counter remains at 0

**Testing**:
- Run `cargo test test_jsonl_writer_write_empty_array`
- Verify empty array is handled gracefully

**Success Criteria**:
- [ ] Test passes
- [ ] Covers empty events branch
- [ ] No panics or errors with empty input

### Phase 3: Add Test for Multiple Events in Single Write

**Goal**: Test batching behavior where multiple events are written in one call

**Changes**:
- Add test `test_jsonl_writer_write_batch_events` that:
  - Creates a JsonlEventWriter
  - Calls `write()` with 10 events
  - Verifies all events are serialized and written
  - Verifies size counter reflects total bytes for all events
  - Verifies file has 10 lines

**Testing**:
- Run `cargo test test_jsonl_writer_write_batch_events`
- Verify batch processing works correctly

**Success Criteria**:
- [ ] Test passes
- [ ] Covers batch serialization path
- [ ] Size accumulation is correct

### Phase 4: Add Test for Rotation Trigger

**Goal**: Test the rotation path where file size exceeds rotation_size

**Changes**:
- Add test `test_jsonl_writer_rotation_on_write` that:
  - Creates a JsonlEventWriter with small rotation_size (e.g., 100 bytes)
  - Writes events that exceed the rotation threshold
  - Calls `write()` again
  - Verifies `rotate_if_needed()` was called (file rotated)
  - Verifies new file is created and size counter reset

**Testing**:
- Run `cargo test test_jsonl_writer_rotation_on_write`
- Verify rotation logic is triggered correctly

**Success Criteria**:
- [ ] Test passes
- [ ] Covers rotation path in `write()`
- [ ] File rotation behavior is verified

### Phase 5: Add Test for None Writer (Closed File)

**Goal**: Test edge case where writer has been closed (Option is None)

**Changes**:
- Add test `test_jsonl_writer_write_after_close` that:
  - Creates a JsonlEventWriter
  - Manually sets writer to None (simulating closed state)
  - Calls `write()` with events
  - Verifies no panic occurs
  - Verifies the function returns Ok (graceful no-op)

**Testing**:
- Run `cargo test test_jsonl_writer_write_after_close`
- Verify graceful handling of None writer

**Success Criteria**:
- [ ] Test passes
- [ ] Covers the `if let Some(writer)` branch with None
- [ ] No panics when writer is closed

## Testing Strategy

**For each phase**:
1. Run `cargo test <test_name>` to verify the new test
2. Run `cargo test` to ensure no regressions
3. Run `cargo clippy` to check for warnings
4. Commit the test with message: "test: add <test_name> for JsonlEventWriter::write coverage"

**Final verification**:
1. Run `cargo test --lib` - all tests pass
2. Run `cargo tarpaulin --out Stdout` - verify coverage increase
3. Run `cargo clippy` - no warnings
4. Run `cargo fmt --check` - proper formatting
5. Run `just ci` - full CI checks pass

## Rollback Plan

If a phase fails:
1. Review test failure output
2. If test design is wrong: Fix the test
3. If bug is found in production code: Document it but DO NOT fix (out of scope)
4. Revert with `git reset --hard HEAD~1` if needed
5. Adjust plan and retry

## Notes

**Important Context**:
- The `write()` function is already well-structured with extracted helpers
- There are 40 upstream callers, making test coverage critical
- Existing tests cover helper functions (serialize_events_to_jsonl, write_serialized_events, update_size_counter)
- This plan focuses ONLY on adding direct tests for the `write()` method itself
- NO refactoring is needed - complexity 5 is acceptable for an I/O wrapper

**Test Location**:
- All tests should be added to the existing `mod tests` section at line 302
- Follow existing test patterns (tempfile, EventRecord creation, etc.)
- Reuse helper functions from existing tests

**Coverage Gaps to Address**:
1. Happy path: serialize + write + size update (Phase 1)
2. Edge case: empty events array (Phase 2)
3. Batch processing: multiple events (Phase 3)
4. Rotation trigger: file size threshold (Phase 4)
5. Closed writer: None case (Phase 5)

These 5 tests will provide comprehensive coverage of all code paths in the `write()` method.
