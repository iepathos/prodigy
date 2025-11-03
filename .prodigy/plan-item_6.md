# Implementation Plan: Extract Test Code from validation.rs God Module

## Problem Summary

**Location**: ./src/cook/workflow/executor/validation.rs:file:0
**Priority Score**: 69.44
**Debt Type**: God Object (File-level)
**Current Metrics**:
- Lines of Code: 2,038
- Functions: 112 (92 test functions, 20 implementation functions)
- Cyclomatic Complexity: 209 total (avg 1.87 per function)
- Coverage: 0.0% (test file, not production code)
- God Object Score: 1.0 (maximum)
- Responsibilities: 7 distinct domains mixed together

**Issue**: This file has become a massive "God Module" with 2,038 lines containing both implementation code (validation logic) and extensive test code (78+ test functions). The file violates the single responsibility principle by mixing production validation logic with test infrastructure and test cases. The recommended action is to split by data flow: 1) Input/parsing functions 2) Core logic/transformation 3) Output/formatting 4) Extract test code to separate test module.

**Root Cause**: Test code has grown organically within the same file as implementation code, violating the convention of separating tests into dedicated test modules. The debtmap analysis identifies this as having 92 private test functions and test helper functions mixed with 20 implementation functions.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 41.8 points (moving test code to separate module)
- Maintainability Improvement: 6.94 points
- Test Effort: 203.8 lines of test code to reorganize

**Success Criteria**:
- [ ] Test code extracted to `src/cook/workflow/executor/validation_tests.rs`
- [ ] Implementation code remains in `validation.rs` with only production logic
- [ ] All 92 tests continue to pass without modification
- [ ] File size reduced from 2,038 lines to ~250 lines for implementation
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`
- [ ] Clear module boundary with appropriate visibility (`pub(super)`)

## Implementation Phases

### Phase 1: Create Dedicated Test Module File

**Goal**: Set up the infrastructure for separate test module and verify the approach works

**Changes**:
1. Create new file `src/cook/workflow/executor/validation_tests.rs`
2. Add module declaration in `src/cook/workflow/executor/mod.rs` with `#[cfg(test)]`
3. Set up basic test module structure with necessary imports
4. Move ONE simple test function (e.g., `test_should_continue_retry_boundary_conditions`) as proof of concept
5. Verify test still compiles and runs

**Testing**:
```bash
cargo test --lib validation_tests::test_should_continue_retry_boundary_conditions
cargo test --lib -- validation  # Run all validation tests
```

**Success Criteria**:
- [ ] New test file created with proper module structure
- [ ] Module properly declared in parent `mod.rs`
- [ ] Proof-of-concept test passes in new location
- [ ] Original test in validation.rs still exists (not deleted yet)
- [ ] Ready to commit

**Estimated Lines**: Create ~50 lines (new file setup + 1 test)

### Phase 2: Move Pure Function Tests (Decision Logic)

**Goal**: Extract tests for pure decision functions that have no side effects

**Changes**:
1. Move all tests for pure decision functions to `validation_tests.rs`:
   - `test_should_continue_retry_*` (4 tests)
   - `test_determine_handler_type_*` (4 tests)
   - `test_should_fail_workflow_*` (4 tests)
   - `test_calculate_retry_progress_*` (4 tests)
   - `test_determine_validation_execution_mode_*` (6 tests)
   - `test_should_read_result_file_after_commands_*` (3 tests)
   - `test_should_use_result_file_*` (2 tests)

2. Remove these tests from `validation.rs`
3. Ensure all imports are correct in both files

**Testing**:
```bash
cargo test --lib -- validation  # All validation tests should pass
cargo test --lib validation_tests  # New module tests should pass
```

**Success Criteria**:
- [ ] 27 pure function tests moved to separate module
- [ ] All tests pass in new location
- [ ] validation.rs reduced by ~540 lines
- [ ] No duplicate test code between files
- [ ] Ready to commit

**Estimated Lines**: Move ~540 lines from validation.rs to validation_tests.rs

### Phase 3: Move Result Construction Tests

**Goal**: Extract tests for result/context construction functions

**Changes**:
1. Move all tests for construction functions to `validation_tests.rs`:
   - `test_create_command_step_failure_result*` (2 tests)
   - `test_create_file_read_error_result*` (2 tests)
   - `test_create_command_execution_failure_result*` (2 tests)
   - `test_create_validation_execution_context_*` (3 tests)
   - `test_create_validation_timeout_result_*` (3 tests)

2. Remove these tests from `validation.rs`
3. Verify helper functions are accessible (may need `pub(super)` visibility)

**Testing**:
```bash
cargo test --lib -- validation  # All validation tests should pass
cargo clippy --tests  # Check for unused imports or visibility issues
```

**Success Criteria**:
- [ ] 12 construction tests moved to separate module
- [ ] All tests pass in new location
- [ ] validation.rs reduced by ~240 lines (cumulative ~780 lines moved)
- [ ] Helper functions have appropriate visibility
- [ ] Ready to commit

**Estimated Lines**: Move ~240 lines from validation.rs to validation_tests.rs

### Phase 4: Move Formatting and Parsing Tests

**Goal**: Extract tests for formatting and parsing functions

**Changes**:
1. Move all tests for formatting/parsing functions to `validation_tests.rs`:
   - `test_format_validation_passed_message_*` (4 tests)
   - `test_format_validation_failed_message_*` (4 tests)
   - `test_format_failed_validation_detail_*` (3 tests)
   - `test_determine_step_name_*` (5 tests)
   - `test_parse_validation_result_with_fallback_*` (3 tests)
   - `test_parse_result_file_content_*` (4 tests)

2. Remove these tests from `validation.rs`
3. Ensure parsing/formatting functions have proper visibility

**Testing**:
```bash
cargo test --lib -- validation
cargo test --lib validation_tests
```

**Success Criteria**:
- [ ] 23 formatting/parsing tests moved to separate module
- [ ] All tests pass in new location
- [ ] validation.rs reduced by ~460 lines (cumulative ~1,240 lines moved)
- [ ] Ready to commit

**Estimated Lines**: Move ~460 lines from validation.rs to validation_tests.rs

### Phase 5: Move Integration and Async Tests

**Goal**: Extract remaining integration tests and test helper infrastructure

**Changes**:
1. Move test helper functions to `validation_tests.rs`:
   - `create_test_env()`
   - `create_test_executor_for_validation()`
   - `create_test_executor_with_mocks()`

2. Move all async integration tests to `validation_tests.rs`:
   - `test_handle_incomplete_validation_*` (5 tests)
   - `test_handle_step_validation_*` (3 tests)
   - `test_execute_validation_*` (1 test)

3. Remove the entire `mod tests` block from `validation.rs`
4. Final cleanup: remove unused imports from `validation.rs`
5. Update module documentation in both files

**Testing**:
```bash
cargo test --lib -- validation  # All 92 tests should still pass
cargo test --lib validation_tests  # All tests in new module pass
cargo clippy --lib  # No warnings
cargo fmt --check  # Proper formatting
```

**Success Criteria**:
- [ ] All remaining test code moved (9 tests + helpers)
- [ ] `mod tests` block completely removed from validation.rs
- [ ] validation.rs is now ~250 lines (pure implementation)
- [ ] validation_tests.rs contains all ~1,788 lines of test code
- [ ] All 92 tests pass without modification
- [ ] No clippy warnings
- [ ] Both files properly formatted
- [ ] Module documentation updated
- [ ] Ready to commit

**Estimated Lines**:
- Move final ~800 lines from validation.rs to validation_tests.rs
- validation.rs: 2,038 → ~250 lines (87% reduction)
- validation_tests.rs: 0 → ~1,788 lines (new test module)

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib -- validation` to verify all tests pass
2. Run `cargo clippy --tests` to check for warnings
3. Run `cargo fmt` to ensure proper formatting
4. Verify imports are minimal and correct in both files

**After Phase 2-4 (incremental verification)**:
- Tests should pass in both old and new locations during transition
- Remove old tests only after verifying new location works

**Final verification (Phase 5)**:
1. `cargo test --lib -- validation` - All 92 tests pass
2. `cargo clippy --lib` - No warnings
3. `cargo fmt --check` - Proper formatting
4. Verify validation.rs has only implementation code (~250 lines)
5. Verify validation_tests.rs has all test code (~1,788 lines)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure (likely import or visibility issue)
3. Adjust visibility modifiers (`pub(super)` for test-accessible functions)
4. Retry with corrected approach

**Common Issues**:
- **Visibility errors**: Add `pub(super)` to functions accessed by tests
- **Import errors**: Ensure `use super::*;` in test module for access to implementation
- **Module not found**: Verify `mod validation_tests;` in executor/mod.rs with `#[cfg(test)]`

## Notes

**Why This Approach**:
- Rust convention is to separate test code from implementation
- This is not a complexity refactoring - we're organizing code by purpose
- Tests remain unchanged - only their location changes
- This is a mechanical, low-risk refactoring with clear rollback

**File Organization Strategy**:
- `validation.rs`: Pure implementation code (~250 lines)
- `validation_tests.rs`: All test code (~1,788 lines)
- This matches Rust convention and improves maintainability

**Visibility Requirements**:
- Implementation functions may need `pub(super)` for test access
- Test module uses `#[cfg(test)]` to exclude from production builds
- Test module imports implementation with `use super::*;`

**This is File-Level Debt**:
- The entire file is the problem (not a specific function)
- Solution: Split file by production vs. test code
- This will reduce the God Object score and improve maintainability
