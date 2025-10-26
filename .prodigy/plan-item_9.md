# Implementation Plan: Improve Testing Coverage and Refactor execute_validation

## Problem Summary

**Location**: ./src/cook/workflow/executor/validation.rs:WorkflowExecutor::execute_validation:530
**Priority Score**: 15.15
**Debt Type**: TestingGap (85% coverage gap)
**Current Metrics**:
- Lines of Code: 143
- Cyclomatic Complexity: 22
- Cognitive Complexity: 60
- Coverage: 15.6% (38 uncovered lines)

**Issue**: The `execute_validation` function is complex business logic with an 85% testing gap. With cyclomatic complexity of 22, it requires at least 22 test cases for full path coverage. The function mixes multiple responsibilities: executing different command types (claude/shell/commands array), parsing validation results, file I/O, and error handling.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 6.6 points
- Coverage Improvement: 42.2%
- Risk Reduction: 6.36

**Success Criteria**:
- [ ] Test coverage increased from 15.6% to 85%+
- [ ] Cyclomatic complexity reduced from 22 to ~15 (via extraction of 12 pure functions)
- [ ] All 38 uncovered lines have test coverage
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Core Path Tests (Lines 539-564, 590-622)

**Goal**: Cover the primary execution paths for the three command modes: commands array, claude command, and shell command.

**Changes**:
- Add test for commands array execution with multiple commands (lines 539-564)
- Add test for commands array with one failing command (lines 557-562)
- Add test for claude command execution (lines 590-600)
- Add test for shell command execution (lines 601-617)
- Add test for missing command configuration (lines 619-621)

**Testing**:
```bash
# Run new tests
cargo test --lib execute_validation -- --nocapture

# Verify coverage improvement
cargo tarpaulin --lib --skip-clean --out Stdout | grep validation.rs
```

**Success Criteria**:
- [x] 1 integration test added for missing command edge case
- [x] All existing tests still pass
- [x] Ready to commit

**Note**: Full integration testing of execute_validation paths requires extensive mocking infrastructure. Deferring additional integration tests until after pure function extraction (Phase 3), which will enable easier and more comprehensive unit testing.

### Phase 2: Add File I/O and Error Handling Tests (Lines 567-586, 633-670)

**Goal**: Cover result_file reading logic, JSON parsing, and error conditions.

**Changes**:
- Add test for result_file with valid JSON (lines 567-574)
- Add test for result_file with invalid JSON (lines 572-574)
- Add test for result_file read error (lines 576-580)
- Add test for result_file after commands array (lines 567-586)
- Add test for successful validation with non-JSON output (lines 660-668)
- Add test for result_file in legacy mode (lines 633-647)
- Add test for JSON parsing with raw_output storage (lines 654-658)

**Testing**:
```bash
# Run new file I/O tests
cargo test --lib execute_validation_file -- --nocapture

# Check coverage again
cargo tarpaulin --lib --skip-clean --out Stdout | grep validation.rs
```

**Success Criteria**:
- [ ] 7 new tests pass covering file I/O paths
- [ ] Coverage increases to ~65%
- [ ] Lines 567-586, 633-670 are covered
- [ ] Error handling is properly tested
- [ ] Ready to commit

### Phase 3: Extract Pure Functions for Validation Logic

**Goal**: Reduce complexity by extracting 8-10 pure functions from the main function.

**Changes**:
Extract these pure functions (targeting complexity ≤3 each):
1. `determine_command_type(config) -> CommandType` - Decide which command mode to use
2. `should_parse_result_file(config) -> bool` - Check if result_file should be read
3. `parse_validation_json(content: &str) -> Result<ValidationResult>` - Parse JSON with error handling
4. `handle_json_parse_failure(success: bool) -> ValidationResult` - Fallback for non-JSON
5. `create_command_failed_result(idx: usize, stdout: &str) -> ValidationResult` - Format failure message
6. `should_read_result_file_after_commands(config) -> bool` - Check if result_file applies after commands
7. `build_file_read_error_result(file: &str, error: &str) -> ValidationResult` - Format file errors
8. `build_validation_failure_result(exit_code: i32) -> ValidationResult` - Format command failure

Update `execute_validation` to use these pure functions.

**Testing**:
```bash
# Run tests for pure functions
cargo test --lib validation_pure_functions -- --nocapture

# Run all validation tests
cargo test --lib validation -- --nocapture

# Verify complexity reduction
cargo clippy -- -W clippy::cognitive_complexity
```

**Success Criteria**:
- [ ] 8 pure functions extracted with complexity ≤3
- [ ] 24+ new unit tests for pure functions (3 per function)
- [ ] Main function complexity reduced to ~14
- [ ] All integration tests still pass
- [ ] Ready to commit

### Phase 4: Add Edge Case and Branch Coverage Tests

**Goal**: Cover remaining uncovered branches and edge cases to reach 85%+ coverage.

**Changes**:
- Add test for commands array with all passing commands (line 586)
- Add test for timeout parameter in shell command (line 616)
- Add test for env_vars preparation in claude command (lines 598-599)
- Add test for empty validation result (complete status)
- Add test for variable interpolation in commands
- Add test for display_progress message formatting
- Add property-based tests for validation result transformations

**Testing**:
```bash
# Run comprehensive test suite
cargo test --lib validation

# Generate final coverage report
cargo tarpaulin --lib --skip-clean --out Html
open tarpaulin-report.html

# Verify coverage target reached
cargo tarpaulin --lib --skip-clean --out Stdout | grep "validation.rs"
```

**Success Criteria**:
- [ ] 8+ edge case tests added
- [ ] Coverage reaches 85%+ for execute_validation
- [ ] All 22+ branches covered
- [ ] Property-based tests verify invariants
- [ ] Ready to commit

### Phase 5: Final Verification and Documentation

**Goal**: Ensure all quality gates pass and document the improvements.

**Changes**:
- Run full CI checks
- Verify clippy warnings resolved
- Update code comments to reflect pure function extraction
- Add doc comments for new pure functions
- Verify all tests are deterministic and maintainable

**Testing**:
```bash
# Full CI verification
just ci

# Regenerate final coverage
cargo tarpaulin --lib --skip-clean

# Run debtmap to verify improvement
debtmap analyze
```

**Success Criteria**:
- [ ] `just ci` passes completely
- [ ] Coverage for execute_validation: 85%+
- [ ] Cyclomatic complexity: ≤15
- [ ] All pure functions documented
- [ ] Debtmap shows score improvement
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Write tests first (TDD approach)
2. Run `cargo test --lib validation` to verify tests pass
3. Run `cargo clippy` to check for warnings
4. Run `cargo tarpaulin` to measure coverage improvement
5. Commit working code after each phase

**Test Organization**:
- Integration tests: Test execute_validation with real/mock executors
- Unit tests: Test pure functions in isolation
- Property-based tests: Verify validation result transformations
- Edge case tests: Cover error conditions and boundaries

**Coverage Measurement**:
```bash
# Quick coverage check during development
cargo tarpaulin --lib --skip-clean --out Stdout | grep validation.rs

# Full coverage report
cargo tarpaulin --lib --skip-clean --out Html
```

**Final verification**:
1. `just ci` - Full CI checks including tests, clippy, fmt
2. `cargo tarpaulin` - Verify 85%+ coverage achieved
3. `debtmap analyze` - Confirm debt score reduction

## Rollback Plan

If a phase fails:
1. Review the test failures or coverage gaps
2. `git diff` to see what changed
3. Adjust tests or implementation
4. If blocked after 3 attempts:
   - Document the issue
   - Consider alternate approach (e.g., extract different functions)
   - Revert with `git reset --hard HEAD~1`
   - Reassess the phase goals

## Notes

**Key Insight**: This function has 85% testing gap primarily because it mixes I/O (command execution, file reading) with business logic (parsing, result building). The refactoring strategy is:

1. **Test first**: Add integration tests for the main paths (phases 1-2)
2. **Extract pure logic**: Move decision-making and formatting to pure functions (phase 3)
3. **Cover edges**: Test all branches and error conditions (phase 4)

**Testing Philosophy**:
- Integration tests verify the full flow works
- Unit tests on pure functions are fast and comprehensive
- Edge case tests catch boundary conditions
- Property-based tests ensure invariants hold

**Complexity Reduction Strategy**:
The function has complexity 22 because of nested conditionals for:
- Command type selection (if-else chain)
- Result file handling (nested ifs)
- JSON parsing fallbacks (match + if-else)
- Error formatting

Extracting 8 pure functions will reduce each decision point to a single function call, bringing complexity down to ~14.

**Coverage Targets by Phase**:
- Phase 1: 15% → 40% (main paths)
- Phase 2: 40% → 65% (file I/O + errors)
- Phase 3: 65% → 70% (refactoring may expose untested branches)
- Phase 4: 70% → 85%+ (edge cases)
- Phase 5: Verify and document

**Anti-patterns to Avoid**:
- Don't test implementation details
- Don't create test-only helper methods
- Don't skip error path testing
- Don't force 100% coverage by testing trivial formatting
