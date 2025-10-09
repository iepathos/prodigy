# Implementation Plan: Refactor run_streaming with Functional Patterns

## Problem Summary

**Location**: ./src/subprocess/runner.rs:TokioProcessRunner::run_streaming:308
**Priority Score**: 45.13
**Debt Type**: ComplexityHotspot (Cognitive: 84, Cyclomatic: 29)
**Current Metrics**:
- Lines of Code: 175
- Cyclomatic Complexity: 29
- Cognitive Complexity: 84
- Nesting Depth: 3
- Function Role: PureLogic (but contains I/O mixing)

**Issue**: Apply functional patterns: 6 pure functions with Iterator chains. Moderate complexity (29), needs functional decomposition to reduce cognitive load and improve maintainability.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 14.5 (from 29 to ~14.5)
- Risk Reduction: 15.8
- Lines Reduction: ~50-70 lines through deduplication

**Success Criteria**:
- [ ] Cyclomatic complexity reduced to ≤15
- [ ] Cognitive complexity reduced to ≤40
- [ ] Code duplication eliminated (stdout/stderr streams)
- [ ] Pure functions extracted for stream creation and status handling
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Pure Stream Creation Functions

**Goal**: Extract pure functions for creating stdout/stderr streams, eliminating duplication.

**Changes**:
- Create `create_line_stream()` pure function that takes a `BufReader` and returns a configured stream
- Replace duplicated stdout/stderr stream creation code (lines 378-431) with calls to the new function
- Extract the line processing logic (removing newlines) into a pure `normalize_line()` function

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings
- Verify the streaming tests still work correctly

**Success Criteria**:
- [ ] ~50 lines of duplicated code eliminated
- [ ] `create_line_stream()` function exists and is reusable
- [ ] `normalize_line()` function handles line endings correctly
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Status Future Builder

**Goal**: Extract pure function for building the status future, eliminating timeout handling duplication.

**Changes**:
- Create `create_status_future()` function that takes:
  - `child: Child`
  - `timeout: Option<Duration>`
  - `program: String`
  - `args: Vec<String>`
- Returns: `Pin<Box<dyn Future<Output = Result<ExitStatus, ProcessError>> + Send>>`
- Replace status future creation code (lines 438-475) with a call to the new function
- Extract exit status conversion into `convert_exit_status()` pure function

**Testing**:
- Run `cargo test --lib`
- Run timeout-specific tests to verify timeout handling still works
- Run `cargo clippy`

**Success Criteria**:
- [ ] ~40 lines of code reduced through consolidation
- [ ] `create_status_future()` handles both timeout and non-timeout cases
- [ ] `convert_exit_status()` is a pure function
- [ ] Timeout behavior unchanged
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Command Configuration Builder

**Goal**: Unify command configuration logic between `run()` and `run_streaming()`.

**Changes**:
- Refactor `configure_command()` to support streaming mode via a parameter
- Create `configure_streaming_command()` that reuses common configuration
- Replace command configuration code in `run_streaming()` (lines 317-337) with the new function
- Ensure both `run()` and `run_streaming()` use consistent configuration logic

**Testing**:
- Run `cargo test --lib` to verify all tests pass
- Test both streaming and non-streaming execution paths
- Run `cargo clippy`

**Success Criteria**:
- [ ] Command configuration is consistent between `run()` and `run_streaming()`
- [ ] ~20 lines of duplicated configuration code eliminated
- [ ] Both execution modes work correctly
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Stdin Handling Function

**Goal**: Consolidate stdin handling logic to reduce duplication.

**Changes**:
- Refactor the existing `write_stdin()` to handle both execution modes
- Replace inline stdin handling in `run_streaming()` (lines 346-360) with call to the refactored function
- Use the same error handling pattern as `run()`

**Testing**:
- Run stdin-specific tests
- Verify both `run()` and `run_streaming()` handle stdin correctly
- Run `cargo test --lib`
- Run `cargo clippy`

**Success Criteria**:
- [ ] Stdin handling is unified and reusable
- [ ] ~15 lines of duplicated code eliminated
- [ ] Both execution modes handle stdin identically
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Final Cleanup and Validation

**Goal**: Ensure all improvements are complete and validate the refactoring.

**Changes**:
- Review `run_streaming()` function for any remaining complexity
- Add inline documentation for the new pure functions
- Ensure error types are consistent throughout
- Final code formatting and style checks

**Testing**:
- Run `just ci` - Full CI checks including all tests
- Run `cargo clippy -- -D warnings` to ensure no warnings
- Run `cargo fmt --check` to verify formatting
- Manually review complexity improvements

**Success Criteria**:
- [ ] `run_streaming()` is now ~100 lines instead of 175
- [ ] Cyclomatic complexity reduced to ≤15
- [ ] Cognitive complexity reduced to ≤40
- [ ] All pure functions are documented
- [ ] CI passes completely
- [ ] No clippy warnings
- [ ] Ready to commit final changes

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run specific tests related to the changed functionality
4. Ensure no behavioral changes (only refactoring)

**Phase-specific testing**:
- Phase 1: Focus on streaming output tests
- Phase 2: Focus on timeout and exit status tests
- Phase 3: Focus on command configuration tests
- Phase 4: Focus on stdin handling tests

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo test --all` - All tests including integration tests
3. Manual testing of streaming functionality
4. Review git diff to ensure only refactoring changes

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure messages and test output
3. Identify the root cause:
   - Type mismatch? Review function signatures
   - Test failure? Check behavioral change
   - Clippy warning? Review suggested fix
4. Adjust the implementation approach
5. Retry with corrected approach

If blocked after 3 attempts on a phase:
1. Document what failed and why
2. Consider breaking the phase into smaller sub-phases
3. Research alternative approaches in the codebase
4. Ask for guidance if fundamental design issue

## Notes

### Key Insights from Analysis

1. **Duplication Pattern**: The stdout/stderr stream creation code is nearly identical (lines 378-403 vs 406-431). This is the primary source of complexity.

2. **Timeout Handling**: The status future has duplicated logic for timeout vs non-timeout cases (lines 439-472). This can be consolidated.

3. **Command Configuration**: The command setup in `run_streaming()` duplicates logic from `configure_command()` but doesn't reuse it.

4. **Pure Function Candidates**:
   - `normalize_line()`: String → String (removes newlines)
   - `create_line_stream()`: BufReader → Stream (creates async stream)
   - `convert_exit_status()`: ExitStatus → ExitStatus (converts types)
   - `create_status_future()`: (Child, Option<Duration>) → Future (builds status future)

5. **Functional Patterns to Apply**:
   - Extract pure transformations (line normalization)
   - Use function composition for stream creation
   - Parameterize behavior instead of duplicating code
   - Separate pure logic from I/O coordination

### Complexity Reduction Strategy

- **Phase 1**: Reduces ~50 lines through stream deduplication
- **Phase 2**: Reduces ~40 lines through status future consolidation
- **Phase 3**: Reduces ~20 lines through command config unification
- **Phase 4**: Reduces ~15 lines through stdin handling unification
- **Total Expected Reduction**: ~125 lines → Target: ~100 lines (from 175)

This should bring cyclomatic complexity from 29 to ~14-15, achieving the 14.5 target from debtmap analysis.

### Error Handling Note

The function uses multiple `ProcessError` variants. Ensure consistency:
- `ProcessError::SpawnFailed` for spawn errors
- `ProcessError::IoError` for I/O errors
- `ProcessError::InternalError` for logical errors

All new functions should follow Spec 101 (no unwrap/panic in production code).
