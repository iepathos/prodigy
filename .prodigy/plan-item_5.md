# Implementation Plan: Refactor GitHandler::execute for Reduced Complexity

## Problem Summary

**Location**: ./src/commands/handlers/git.rs:GitHandler::execute:246
**Priority Score**: 29.16
**Debt Type**: ComplexityHotspot (cognitive: 16, cyclomatic: 6)
**Current Metrics**:
- Lines of Code: 38
- Function Length: 38 lines
- Cyclomatic Complexity: 6
- Cognitive Complexity: 16
- Coverage: Not specified (transitive_coverage: null)

**Issue**: While the cyclomatic complexity of 6 is manageable, the cognitive complexity of 16 is higher, indicating nested control flow and mental overhead. The recommendation suggests extracting guard clauses for precondition checks and maintaining simplicity.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 3.0
- Coverage Improvement: 0.0
- Risk Reduction: 10.206

**Success Criteria**:
- [ ] Cognitive complexity reduced from 16 to under 13
- [ ] Cyclomatic complexity maintained at 6 or lower
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Precondition Validation

**Goal**: Extract all precondition checks into a separate pure function to reduce cognitive load in the main execute function.

**Changes**:
- Create a new pure function `validate_preconditions` that handles:
  - Operation validation
  - Argument building and validation
  - Returns a structured result with all needed data
- Simplify the execute function by calling this validation function

**Testing**:
- Run `cargo test --lib test_git` to verify git handler tests pass
- All existing tests should continue to pass

**Success Criteria**:
- [ ] New validation function is pure (no side effects)
- [ ] Execute function has fewer conditional branches
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Dry Run Logic

**Goal**: Move dry run handling into a dedicated function to reduce nesting and improve readability.

**Changes**:
- Create a function `handle_dry_run` that encapsulates:
  - Dry run check
  - Response building
  - Duration calculation
- Reduce conditional nesting in execute function

**Testing**:
- Run `cargo test --lib test_git_commit_dry_run` to verify dry run tests
- Run `cargo test --lib test_git` for all git tests

**Success Criteria**:
- [ ] Dry run logic is isolated
- [ ] Execute function has one less conditional branch
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Simplify Auto-staging Flow

**Goal**: Refactor auto-staging to reduce complexity and improve error handling flow.

**Changes**:
- Refactor `execute_auto_staging` to return early on non-commit operations
- Simplify the conditional flow in execute function
- Consider combining auto-staging check with preconditions if appropriate

**Testing**:
- Run `cargo test --lib test_git_commit_with_auto_stage`
- Run `cargo test --lib test_git_commit_auto_stage_failure`
- Run `cargo test --lib test_git_commit_auto_stage_custom_files`

**Success Criteria**:
- [ ] Auto-staging flow is clearer
- [ ] Error handling is more direct
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Final Optimization and Measurement

**Goal**: Apply final optimizations and verify complexity reduction goals are met.

**Changes**:
- Review the execute function for any remaining complexity
- Extract any remaining complex conditionals into named functions
- Add documentation to clarify the flow

**Testing**:
- Run full test suite: `cargo test --lib`
- Run clippy: `cargo clippy`
- Run formatter: `cargo fmt`

**Success Criteria**:
- [ ] Cognitive complexity reduced by at least 3 points
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code is properly formatted
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib test_git` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run specific test cases relevant to the phase

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin` - Regenerate coverage (if available)
3. `debtmap analyze` - Verify improvement in complexity scores

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure and understand what went wrong
3. Adjust the plan if necessary
4. Retry with the corrected approach

## Notes

- The current complexity of 6 is already manageable, but the cognitive complexity of 16 indicates nested conditionals that make the code harder to follow
- Focus on extracting guard clauses and early returns to flatten the control flow
- The function is an entry point with 87 upstream dependencies, so maintaining backward compatibility is crucial
- Many callers are tests, which provides good coverage for verification
- The recommendation emphasizes maintaining simplicity rather than aggressive refactoring
