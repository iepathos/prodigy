# Implementation Plan: Refactor TokioProcessRunner::run_streaming for Better Testability

## Problem Summary

**Location**: ./src/subprocess/runner.rs:TokioProcessRunner::run_streaming:435
**Priority Score**: 45.73
**Debt Type**: ComplexityHotspot (cognitive: 16, cyclomatic: 6)
**Current Metrics**:
- Lines of Code: 53
- Function Length: 53 lines
- Cyclomatic Complexity: 6
- Cognitive Complexity: 16
- Coverage: Not directly covered (only through integration tests)

**Issue**: Function has moderate complexity and mixes I/O orchestration with logic. While complexity 6 is manageable, the cognitive complexity of 16 suggests the function could benefit from extraction of pure logic and better separation of concerns for improved testability.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 3.0
- Coverage Improvement: 0.0 (but we'll improve testability)
- Risk Reduction: 16.01

**Success Criteria**:
- [ ] Extract stream creation logic into pure, testable functions
- [ ] Reduce cognitive complexity below 10
- [ ] Maintain cyclomatic complexity at 6 or below
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Stream Creation Logic

**Goal**: Extract the stdout/stderr stream creation into a separate, reusable function

**Changes**:
- Extract lines 468-472 into a helper method `create_output_streams`
- This consolidates the repeated pattern of taking and creating streams
- Reduces duplication and cognitive load

**Testing**:
- Run `cargo test --lib subprocess` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] Stream creation logic extracted
- [ ] No code duplication for stdout/stderr handling
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Process Setup Logic

**Goal**: Separate the process spawn and configuration from stream handling

**Changes**:
- Extract lines 442-451 into a `spawn_configured_process` method
- This includes command configuration, spawning, and stdin writing
- Returns a configured child process ready for stream extraction

**Testing**:
- Run `cargo test --lib subprocess` to verify existing tests
- Test with streaming examples in the test suite

**Success Criteria**:
- [ ] Process setup logic separated
- [ ] Clear separation between spawn and stream handling
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Simplify Error Handling Patterns

**Goal**: Reduce cognitive complexity by improving error handling clarity

**Changes**:
- Replace the `.ok_or_else` patterns with a helper function for stream extraction
- Create `extract_stream` helper that handles the Option -> Result conversion
- Consolidate error message construction

**Testing**:
- Run full test suite: `cargo test`
- Verify error cases are properly handled

**Success Criteria**:
- [ ] Error handling patterns simplified
- [ ] Cognitive complexity reduced
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Add Unit Tests for Extracted Components

**Goal**: Improve test coverage by testing the extracted pure functions

**Changes**:
- Add unit tests for the new helper functions
- Test edge cases and error conditions
- Focus on testing the logic, not the I/O

**Testing**:
- Run `cargo test --lib subprocess` with new tests
- Check coverage improvement with `cargo tarpaulin`

**Success Criteria**:
- [ ] New unit tests added
- [ ] Edge cases covered
- [ ] Coverage metrics improved
- [ ] All tests pass
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib subprocess` to verify existing tests pass
2. Run `cargo clippy -- -W clippy::all` to check for warnings
3. Run `cargo fmt --check` to verify formatting

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib --out Html` - Regenerate coverage report
3. Re-run debtmap to verify improvement in metrics

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure - check if it's a test issue or logic issue
3. Adjust the plan based on findings
4. Retry with modified approach

## Notes

- The function's complexity of 6 is already at a reasonable level, but the cognitive complexity of 16 indicates nested conditions and error handling that could be simplified
- The function orchestrates I/O operations, so we'll focus on extracting the pure logic portions while keeping the I/O coordination intact
- Many callers are test functions, which provides good integration test coverage already
- The main improvement will be in code clarity and maintainability rather than dramatic complexity reduction
