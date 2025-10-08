# Implementation Plan: Test and Refactor handle_incomplete_validation

## Problem Summary

**Location**: ./src/cook/workflow/executor/validation.rs:WorkflowExecutor::handle_incomplete_validation:224
**Priority Score**: 31.59
**Debt Type**: TestingGap (0% coverage, cyclomatic complexity 20)

**Current Metrics**:
- Lines of Code: 112
- Cyclomatic Complexity: 20
- Cognitive Complexity: 56
- Coverage: 0% (direct), 18.2% (transitive)
- Nesting Depth: 4
- Downstream Dependencies: 11

**Issue**: Complex business logic with 100% coverage gap. Cyclomatic complexity of 20 requires at least 20 test cases for full path coverage. After extracting 11 functions, each will need only 3-5 tests. Testing before refactoring ensures no regressions.

## Target State

**Expected Impact**:
- Complexity Reduction: 6.0
- Coverage Improvement: 50%
- Risk Reduction: 13.27

**Success Criteria**:
- [ ] 80%+ test coverage for handle_incomplete_validation
- [ ] Cyclomatic complexity reduced from 20 to ≤14
- [ ] All 35 uncovered lines have test coverage
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Integration Tests for Core Paths

**Goal**: Achieve 50%+ coverage by testing the main execution paths through handle_incomplete_validation without modifying the implementation.

**Changes**:
- Add test for retry loop with commands array execution
- Add test for retry loop with single claude command
- Add test for retry loop with single shell command
- Add test for validation passing after retry
- Add test for validation failing with fail_workflow=true
- Add test for prompt handling after max attempts

**Testing**:
```bash
cargo test --lib handle_incomplete_validation
cargo tarpaulin --out Stdout --include-tests --exclude-tests -- handle_incomplete_validation
```

**Success Criteria**:
- [ ] 6 new integration tests added and passing
- [ ] Coverage for lines 224-334 increases to 50%+
- [ ] All branch points have at least one test
- [ ] Tests cover: commands array, claude/shell handlers, retry logic, fail_workflow

### Phase 2: Extract Retry Loop Logic

**Goal**: Extract the retry loop condition and state management into testable pure functions to reduce complexity by ~3 points.

**Changes**:
- Extract `determine_retry_continuation(attempts, max_attempts, is_complete) -> bool`
  - Replaces inline condition at line 235-236
  - Cyclomatic complexity: 2
- Extract `create_retry_status_message(attempt, max, percentage, threshold) -> String`
  - Replaces formatting at lines 240-243, 302-305
  - Cyclomatic complexity: 1
- Add unit tests for both new functions (6 tests total)

**Testing**:
```bash
# Test the extracted functions
cargo test --lib test_determine_retry_continuation
cargo test --lib test_create_retry_status_message

# Verify integration still works
cargo test --lib handle_incomplete_validation

# Check coverage improvement
cargo tarpaulin --out Stdout --include-tests --exclude-tests -- validation
```

**Success Criteria**:
- [ ] 2 new pure functions extracted
- [ ] 6 new unit tests added and passing
- [ ] Cyclomatic complexity of handle_incomplete_validation reduced to ≤17
- [ ] All existing tests still pass
- [ ] No clippy warnings

### Phase 3: Extract Command Execution Logic

**Goal**: Extract the command execution logic into separate testable functions to reduce complexity by ~4 points.

**Changes**:
- Extract `execute_recovery_commands_array(commands, idx_context) -> Result<bool>`
  - Handles lines 246-271 (commands array execution)
  - Cyclomatic complexity: 3
- Extract `execute_single_recovery_command(handler_step) -> Result<bool>`
  - Handles lines 272-279 (single command execution)
  - Cyclomatic complexity: 2
- Extract `determine_recovery_strategy(on_incomplete) -> RecoveryStrategy`
  - Determines which execution path to take (lines 246, 272, 280)
  - Cyclomatic complexity: 2
  - Returns enum: MultiCommand | SingleCommand | NoHandler
- Add unit tests for all new functions (9 tests total)

**Testing**:
```bash
# Test the extracted functions
cargo test --lib execute_recovery_commands
cargo test --lib determine_recovery_strategy

# Verify integration
cargo test --lib handle_incomplete_validation

# Check coverage and complexity
cargo tarpaulin --out Stdout --include-tests --exclude-tests -- validation
```

**Success Criteria**:
- [ ] 3 new functions extracted with clear responsibilities
- [ ] 9 new unit tests added and passing
- [ ] Cyclomatic complexity of handle_incomplete_validation reduced to ≤13
- [ ] Recovery logic is now testable in isolation
- [ ] All existing tests still pass

### Phase 4: Extract Validation Re-execution Logic

**Goal**: Extract validation re-execution and result interpretation logic to reduce final complexity points.

**Changes**:
- Extract `interpret_validation_result(result, config, is_complete) -> ValidationInterpretation`
  - Handles lines 294-306 (validation result interpretation)
  - Returns struct with: message, should_continue, display_level
  - Cyclomatic complexity: 2
- Extract `should_fail_after_validation(result, config, attempts) -> (bool, String)`
  - Handles lines 324-331 (final failure check)
  - Returns: (should_fail, error_message)
  - Cyclomatic complexity: 2
- Add unit tests for both functions (8 tests total)

**Testing**:
```bash
# Test the extracted functions
cargo test --lib interpret_validation_result
cargo test --lib should_fail_after_validation

# Full validation test suite
cargo test --lib validation

# Final coverage check
cargo tarpaulin --out Stdout --include-tests --exclude-tests -- validation
```

**Success Criteria**:
- [ ] 2 new pure functions extracted
- [ ] 8 new unit tests added and passing
- [ ] Cyclomatic complexity of handle_incomplete_validation reduced to ≤11
- [ ] 80%+ test coverage achieved
- [ ] All uncovered lines from debtmap are now covered

### Phase 5: Refactor Main Function and Final Cleanup

**Goal**: Simplify the main function to orchestrate the extracted pure functions and achieve target metrics.

**Changes**:
- Refactor handle_incomplete_validation to use extracted functions
  - Replace inline logic with calls to pure functions
  - Improve readability and reduce nesting
- Add final integration tests for edge cases
  - Test interaction between all components
  - Test error propagation
- Update documentation and add rustdoc examples

**Testing**:
```bash
# Full test suite
cargo test --lib

# Coverage verification
cargo tarpaulin --out Stdout --include-tests --exclude-tests

# Complexity check
cargo clippy -- -D warnings

# Format check
cargo fmt --check
```

**Success Criteria**:
- [ ] handle_incomplete_validation orchestrates pure functions clearly
- [ ] Nesting depth reduced from 4 to ≤2
- [ ] Final cyclomatic complexity ≤11 (target: 14, achieved better)
- [ ] 80%+ test coverage achieved
- [ ] All 31 tests passing (existing + new)
- [ ] No clippy warnings
- [ ] Code formatted correctly
- [ ] Risk score reduced by 13.27 points

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo tarpaulin --out Stdout --include-tests --exclude-tests` to measure coverage
4. Commit working code with descriptive message

**Coverage Targets by Phase**:
- Phase 1: 50%+ (baseline integration tests)
- Phase 2: 60%+ (retry logic tested)
- Phase 3: 70%+ (command execution tested)
- Phase 4: 80%+ (validation interpretation tested)
- Phase 5: 80%+ (final integration)

**Final verification**:
1. `cargo test --lib` - All tests pass
2. `cargo tarpaulin --out Stdout --include-tests --exclude-tests` - 80%+ coverage
3. `cargo clippy -- -D warnings` - No warnings
4. Verify complexity reduction with metrics tool

## Rollback Plan

If a phase fails:
1. Identify the failing test or check
2. Review the error message and stack trace
3. Revert the phase with `git reset --hard HEAD~1`
4. Analyze what went wrong
5. Adjust approach:
   - For test failures: Fix the logic or test expectations
   - For complexity issues: Extract smaller functions
   - For coverage gaps: Add more targeted tests
6. Retry the phase with adjusted approach

## Notes

### Key Insights from Code Analysis

1. **Function Structure**: The function has 3 main sections:
   - Retry loop (lines 235-311): Executes recovery commands and re-validates
   - Interactive prompt (lines 314-322): Prompts user if validation incomplete
   - Failure check (lines 324-331): Decides whether to fail workflow

2. **Complexity Sources**:
   - Nested conditionals for command type detection (lines 246, 272, 280)
   - Loop with multiple exit conditions
   - Validation result interpretation (lines 296-306)
   - Multiple error paths

3. **Extraction Opportunities**:
   - Retry loop logic is pure and can be extracted
   - Command execution strategy is pure decision logic
   - Validation result interpretation is pure formatting
   - Failure determination is pure logic

4. **Testing Challenges**:
   - Function is async and uses mutable state
   - Integration tests needed for full execution paths
   - Mock objects required for WorkflowExecutor dependencies
   - Pure function extraction enables easier unit testing

5. **Existing Test Infrastructure**:
   - File already has comprehensive tests for pure functions
   - MockUserInteraction available for testing
   - Test helpers (create_test_env) already present
   - Good foundation to build upon

### Function Dependencies

**Calls made by handle_incomplete_validation**:
- `self.user_interaction.display_info()` - Display progress
- `self.user_interaction.display_progress()` - Display command execution
- `self.user_interaction.display_error()` - Display errors
- `self.user_interaction.display_success()` - Display success
- `self.user_interaction.prompt_confirmation()` - Interactive prompts
- `self.convert_workflow_command_to_step()` - Convert command to step
- `self.get_step_display_name()` - Get display name
- `self.execute_step()` - Execute workflow step
- `self.create_validation_handler()` - Create handler step
- `self.execute_validation()` - Execute validation
- `ValidationConfig::is_complete()` - Check completion

**Required for testing**:
- Mock WorkflowExecutor with these methods
- Mock ExecutionEnvironment
- Mock WorkflowContext
- Mock ValidationConfig with is_complete implementation

### Refactoring Patterns

Follow functional programming principles:
- Pure functions for decision logic
- Separate I/O (execute_step) from pure logic (determine strategy)
- Immutable data flow (return new states, don't mutate)
- Small functions with single responsibility (≤20 lines)
- Clear function names that describe intent

### Expected Final Structure

After refactoring, handle_incomplete_validation will:
1. Initialize state
2. Loop while `determine_retry_continuation()` returns true
3. Use `determine_recovery_strategy()` to pick execution path
4. Execute commands via extracted execution functions
5. Re-validate and interpret result via `interpret_validation_result()`
6. Handle interactive prompt if needed
7. Check failure via `should_fail_after_validation()`

This structure is easier to understand, test, and maintain.
