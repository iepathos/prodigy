# Implementation Plan: Improve Test Coverage and Reduce Complexity in resume_workflow

## Problem Summary

**Location**: ./src/cook/orchestrator/execution_pipeline.rs:ExecutionPipeline::resume_workflow:280
**Priority Score**: 15.23
**Debt Type**: TestingGap (cognitive: 78, cyclomatic: 23, coverage: 31.5%)

**Current Metrics**:
- Function Length: 155 lines
- Cyclomatic Complexity: 23
- Cognitive Complexity: 78
- Test Coverage: 31.5% (direct coverage)
- Uncovered Lines: 37 lines across multiple code paths

**Issue**: Complex business logic with significant testing gaps (69% coverage gap). The function has 23 decision branches requiring comprehensive test coverage. High cognitive complexity (78) indicates the function is doing too much and needs refactoring into smaller, focused functions.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 6.9 (reduce from 23 to ~16)
- Coverage Improvement: 34.3% (from 31.5% to ~65.8%)
- Risk Reduction: 6.4

**Success Criteria**:
- [ ] Test coverage increases from 31.5% to at least 65%
- [ ] Cyclomatic complexity reduces from 23 to 16 or lower
- [ ] All uncovered error paths have test coverage
- [ ] Extract at least 5-7 pure functions from complex logic
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Tests for Uncovered Error Paths

**Goal**: Increase test coverage by adding tests for critical uncovered error paths, focusing on error handling branches.

**Changes**:
- Add test for session not found in unified storage AND worktree file missing (lines 290-299)
- Add test for non-resumable session status (lines 311-314)
- Add test for workflow hash mismatch (lines 323-325)
- Add test for missing workflow_state (lines 426-428)
- Add test for session interrupted during resume (lines 385-396)
- Add test for session failure during resume (lines 398-406)

**Testing**:
```bash
# Run tests to verify new test cases pass
cargo test --lib resume_workflow

# Verify coverage improved
cargo tarpaulin --out Stdout --packages prodigy --lib -- resume_workflow
```

**Success Criteria**:
- [ ] 6 new test cases added covering error paths
- [ ] Coverage increases to at least 50%
- [ ] All existing tests still pass
- [ ] Tests are in `tests/cli_integration/resume_integration_tests.rs`

### Phase 2: Extract Session Loading Logic

**Goal**: Extract session loading and fallback logic into a separate pure function to reduce complexity.

**Changes**:
- Create new function `load_session_with_fallback(session_id, config, session_manager)`
- Extract lines 283-307 (session loading with worktree fallback)
- Function should return `Result<SessionState>`
- Reduces cyclomatic complexity by ~3

**Testing**:
```bash
# Run tests to verify refactoring didn't break anything
cargo test --lib resume_workflow

# Verify no clippy warnings
cargo clippy --package prodigy
```

**Success Criteria**:
- [ ] New function `load_session_with_fallback` created
- [ ] Original function calls new function
- [ ] All existing tests pass
- [ ] Complexity reduced (verify with metrics)
- [ ] No clippy warnings

### Phase 3: Extract Validation Logic

**Goal**: Extract session validation logic into separate pure functions.

**Changes**:
- Create `validate_session_resumable(state)` - lines 310-316
- Create `validate_workflow_unchanged(state, config)` - lines 319-328
- Each function returns `Result<()>`
- Reduces cyclomatic complexity by ~2-3

**Testing**:
```bash
# Add unit tests for new validation functions
cargo test --lib validate_session_resumable
cargo test --lib validate_workflow_unchanged

# Verify integration still works
cargo test --lib resume_workflow
```

**Success Criteria**:
- [ ] 2 new validation functions created
- [ ] Unit tests added for each validation function
- [ ] Original function calls validation functions
- [ ] All tests pass
- [ ] Complexity further reduced

### Phase 4: Extract Result Handling Logic

**Goal**: Extract complex result handling logic into a separate function to reduce nesting and complexity.

**Changes**:
- Create `handle_resume_result(result, session_manager, user_interaction, config, session_id)`
- Extract lines 375-408 (result handling with error recovery)
- Function should return `Result<()>`
- Reduces cyclomatic complexity by ~4-5

**Testing**:
```bash
# Add unit tests for result handling scenarios
cargo test --lib handle_resume_result

# Verify all integration tests pass
cargo test --lib resume_workflow
```

**Success Criteria**:
- [ ] New function `handle_resume_result` created
- [ ] Tests cover success, interruption, and failure paths
- [ ] Original function simplified
- [ ] All tests pass
- [ ] Cyclomatic complexity now at or below 16

### Phase 5: Add Tests for Remaining Uncovered Lines and Final Verification

**Goal**: Achieve target coverage of 65%+ and verify all improvements.

**Changes**:
- Add tests for edge cases in environment restoration (lines 336, 345, 354)
- Add tests for execution context restoration (lines 354-361)
- Add tests for session completion and summary display (lines 419-423)
- Verify all extracted functions have adequate test coverage

**Testing**:
```bash
# Run full test suite
cargo test --lib

# Generate coverage report
cargo tarpaulin --out Stdout --packages prodigy

# Run clippy and formatting checks
cargo clippy --package prodigy
cargo fmt --check

# Run full CI checks
just ci
```

**Success Criteria**:
- [ ] Coverage at or above 65%
- [ ] All uncovered lines now have tests
- [ ] Cyclomatic complexity at or below 16
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Ready to commit and complete

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib resume_workflow` to verify existing tests pass
2. Run `cargo clippy --package prodigy` to check for warnings
3. Run phase-specific tests as outlined above
4. Commit after each successful phase

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --packages prodigy --lib` - Verify coverage improvement
3. Compare metrics before/after to confirm improvements

## Rollback Plan

If a phase fails:
1. Review the error messages and test failures
2. Use `git diff` to review changes
3. If needed, revert with `git checkout -- <file>`
4. Adjust the approach based on what failed
5. Retry the phase with fixes

For test failures:
- First verify the test is correct (not testing implementation details)
- Then fix the code or test as appropriate
- Never disable or skip tests - fix them

## Notes

**Key Considerations**:
- The function deals with critical resume logic - changes must be thoroughly tested
- Existing integration tests in `tests/cli_integration/resume_integration_tests.rs` provide good coverage patterns to follow
- Focus on extracting pure functions that can be unit tested independently
- Keep I/O operations (session loading, file operations) at the boundaries
- Error handling logic should be explicit and testable

**Patterns to Follow**:
- Session loading already has good patterns in existing tests
- Use the `create_test_checkpoint_with_worktree` helper for test setup
- Follow existing test structure for consistency

**Dependencies**:
- `SessionManager` interface for session operations
- `UserInteraction` interface for user feedback
- Git worktree structure for session storage

**Complexity Sources**:
- Multiple error paths and fallbacks (session loading, validation)
- Result handling with three different outcomes (success, interruption, failure)
- Nested conditionals for state restoration
- Integration with multiple subsystems (session manager, environment, workflow executor)
