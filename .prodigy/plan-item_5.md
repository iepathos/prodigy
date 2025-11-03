# Implementation Plan: Extract Validation and Utilities from WorktreeManager God Class

## Problem Summary

**Location**: ./src/worktree/manager.rs:file:0
**Priority Score**: 101.40
**Debt Type**: God Object (File-level) - God Class Detection
**Current Metrics**:
- Lines of Code: 2283
- Functions: 71 total (39 impl methods, 32 other functions/tests)
- Cyclomatic Complexity: 259 total, avg 3.65, max 18
- Coverage: 0% (TestCode file type)
- God Object Score: 1.0 (definite God Class)
- Responsibilities: 6 distinct (Filtering & Selection, Data Access, Utilities, Processing, Persistence, Validation)
- Methods in WorktreeManager impl: 39
- Struct fields: 6

**Issue**: WorktreeManager is a massive God Class with 2283 lines and 39 methods handling far too many responsibilities. The debtmap analysis recommends splitting into focused modules:

1. **manager_utilities** - 29 methods (Utilities responsibility)
2. **manager_validation** - 6 methods (Validation responsibility)

The current structure violates the Single Responsibility Principle and makes the code difficult to test, understand, and maintain.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 51.8 points
- Maintainability Improvement: 10.14 points
- Test Effort: 228.3 (currently very difficult to test due to size)

**Success Criteria**:
- [ ] Extract validation functions to dedicated `manager_validation.rs` module (6 methods)
- [ ] Extract utility functions to dedicated `manager_utilities.rs` module (29 methods)
- [ ] Reduce WorktreeManager impl to core responsibilities (session creation, worktree operations)
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with cargo fmt
- [ ] Each new module is under 400 lines
- [ ] Functions remain under 20 lines where possible
- [ ] Clear module boundaries and dependencies

## Implementation Phases

### Phase 1: Extract Validation Functions

**Goal**: Create `manager_validation.rs` module with 6 pure validation functions identified by debtmap

**Changes**:
- Create new file `src/worktree/manager_validation.rs`
- Extract these 6 methods as pure functions:
  1. `validate_merge_preconditions` → `validate_merge_preconditions()`
  2. `validate_claude_result` → `validate_claude_result()`
  3. `verify_merge_completion` → `verify_merge_completion()`
  4. `validate_merge_success` → `validate_merge_success()`
  5. `is_permission_denied` → `is_permission_denied()`
  6. `is_branch_merged` → `check_if_branch_merged()` (rename for clarity)
- Make them pure functions where possible (no self parameter)
- Add module documentation explaining validation responsibilities
- Update `src/worktree/mod.rs` to export the new module
- Update imports in `manager.rs` to use the new validation module
- Update method calls in WorktreeManager to use `manager_validation::` functions

**Testing**:
- Run `cargo test --lib` to verify all tests pass
- Run `cargo clippy` to ensure no warnings
- Verify that the validation functions are properly called from manager.rs

**Success Criteria**:
- [ ] `manager_validation.rs` created with 6 validation functions
- [ ] Functions are pure where possible (input → output, no side effects)
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code compiles successfully
- [ ] Ready to commit

### Phase 2: Extract First Batch of Utility Functions (Session Management - 10 methods)

**Goal**: Extract session-related utility functions to `manager_utilities.rs`

**Changes**:
- Create new file `src/worktree/manager_utilities.rs`
- Extract session management utilities (10 methods):
  1. `update_session_state` → `update_session_state()`
  2. `list_sessions` → Keep as method (needs async + self)
  3. `list_git_worktree_sessions` → Keep as method (needs async + subprocess)
  4. `list_detailed` → Keep as method (needs async + complex logic)
  5. `list_metadata_sessions` → `list_metadata_sessions()`
  6. `update_session_state_after_merge` → `update_session_state_after_merge()`
  7. `restore_session` → `restore_session_from_state()`
  8. `mark_session_abandoned` → `mark_session_abandoned()`
  9. `update_checkpoint` → `update_checkpoint_in_session()`
  10. `get_last_successful_command` → `get_last_successful_command()`
- Start with pure utility functions first
- Add module documentation
- Update imports in `manager.rs`

**Testing**:
- Run `cargo test --lib` after each function extraction
- Verify session management functionality still works
- Check for any broken tests

**Success Criteria**:
- [ ] First 10 utility functions extracted
- [ ] Clear separation between I/O orchestration (stays in manager) and pure logic (moves to utilities)
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 3: Extract Second Batch of Utility Functions (Cleanup & Merge - 10 methods)

**Goal**: Extract cleanup and merge-related utility functions

**Changes**:
- Add to `manager_utilities.rs`:
  11. `cleanup_session` → Keep as method (needs async + subprocess)
  12. `cleanup_all_sessions` → Keep as method (orchestration)
  13. `cleanup_session_after_merge` → `cleanup_session_after_merge_impl()` (extract logic)
  14. `detect_mergeable_sessions` → `detect_mergeable_sessions_impl()`
  15. `cleanup_merged_sessions` → Keep as method (orchestration)
  16. `perform_auto_cleanup` → `perform_auto_cleanup_impl()`
  17. `show_cleanup_diagnostics` → Keep as method (I/O)
  18. `show_manual_cleanup_message` → `format_cleanup_message()` (pure)
  19. `finalize_merge_session` → Keep as method (orchestration)
  20. `handle_auto_cleanup_if_enabled` → `should_auto_cleanup()` (pure logic)
- Focus on extracting pure logic and decision functions
- Leave I/O orchestration in WorktreeManager
- Update method calls

**Testing**:
- Run `cargo test --lib`
- Test cleanup and merge workflows
- Verify auto-cleanup logic works correctly

**Success Criteria**:
- [ ] Next 10 utility functions extracted
- [ ] Cleanup and merge logic properly separated
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 4: Extract Third Batch of Utility Functions (Merge Workflow - 9 methods)

**Goal**: Extract merge workflow execution utilities

**Changes**:
- Add to `manager_utilities.rs`:
  21. `execute_merge_workflow` → Keep as method (orchestration)
  22. `execute_claude_merge` → Keep as method (I/O)
  23. `execute_custom_merge_workflow` → Keep as method (orchestration)
  24. `init_merge_variables` → `init_merge_variables_impl()` (extract pure logic)
  25. `execute_merge_shell_command` → Keep as method (I/O)
  26. `execute_merge_claude_command` → Keep as method (I/O)
  27. `interpolate_merge_variables` → `interpolate_variables()` (pure)
  28. `log_execution_context` → `format_execution_context()` (pure)
  29. `log_claude_execution_details` → `format_claude_details()` (pure)
  30. `save_merge_checkpoint` → Keep as method (I/O)

**Note**: Many of these are orchestration/I/O functions that should stay in WorktreeManager. Focus on extracting:
- Variable interpolation (pure)
- Message formatting (pure)
- Decision logic (pure)

**Testing**:
- Run `cargo test --lib`
- Test merge workflow execution
- Verify variable interpolation works

**Success Criteria**:
- [ ] Pure merge workflow utilities extracted
- [ ] Variable interpolation logic separated
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Final Cleanup and Documentation

**Goal**: Clean up WorktreeManager, add documentation, verify improvements

**Changes**:
- Review WorktreeManager impl - should now have ~10-15 core methods
- Add module-level documentation to each new module explaining:
  - Purpose and responsibilities
  - When to use functions in this module
  - Dependencies and relationships
- Add comprehensive doc comments to public functions
- Ensure proper error context in all functions
- Run final quality checks:
  - `cargo fmt` - format all code
  - `cargo clippy` - check for warnings
  - `cargo test --lib` - verify all tests pass
  - Count lines in each module (should be <400 per module)
  - Check cyclomatic complexity (should be <5 per function where possible)

**Testing**:
- Full test suite: `cargo test --lib`
- Clippy: `cargo clippy --all-targets`
- Format check: `cargo fmt --check`
- Manual verification: Review the code structure

**Success Criteria**:
- [ ] WorktreeManager reduced to core responsibilities (~300-400 lines)
- [ ] `manager_validation.rs` complete (~120-200 lines)
- [ ] `manager_utilities.rs` complete (~300-400 lines)
- [ ] All modules have comprehensive documentation
- [ ] All functions have clear doc comments
- [ ] Zero clippy warnings
- [ ] All tests passing
- [ ] Code properly formatted
- [ ] Clear module boundaries maintained
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` immediately after changes
2. Run `cargo clippy` to catch potential issues
3. Manually test affected functionality if possible
4. Verify compilation with `cargo check`

**Final verification**:
1. `cargo test --lib` - All tests must pass
2. `cargo clippy --all-targets` - Zero warnings
3. `cargo fmt --check` - Verify formatting
4. Manual code review:
   - Check module sizes (lines of code)
   - Verify function complexity
   - Ensure clear responsibilities
   - Check for proper error handling

**What NOT to test**:
- Don't add new tests during refactoring
- Don't modify test behavior
- Don't change test expectations
- Focus on preserving existing functionality

## Rollback Plan

If a phase fails:
1. **Identify the failure point**: Compilation error, test failure, or clippy warning
2. **Revert the phase**: `git reset --hard HEAD~1`
3. **Review the failure**: Understand what went wrong
4. **Adjust approach**:
   - Split the phase into smaller chunks
   - Re-examine which functions should be extracted
   - Check for missing dependencies or circular references
5. **Retry with refined approach**: Implement smaller, more focused changes

**Common issues and solutions**:
- **Circular dependency**: Extract to separate trait or use dependency injection
- **Test failures**: Ensure all imports are updated in test module
- **Borrow checker errors**: Review lifetime requirements and ownership
- **Missing methods**: Check if method was moved to utilities but calls weren't updated

## Important Guidelines

### Functional Programming Principles

**DO**:
- Extract pure functions that take inputs and return outputs
- Separate I/O (file operations, git commands) from pure logic
- Use pure functions for validation, formatting, and decision logic
- Keep complex orchestration in WorktreeManager methods
- Make utilities testable without requiring WorktreeManager instance

**DON'T**:
- Move I/O operations to utilities (keep in manager methods)
- Break up legitimate patterns (async/await orchestration)
- Extract methods that need mutable self access
- Create helper methods only used in tests

### Module Organization Principles

**manager_validation.rs** should contain:
- Pure validation functions (input → bool/Result)
- Error checking logic
- Precondition verification
- State consistency checks

**manager_utilities.rs** should contain:
- Pure utility functions (transformations, formatting)
- Helper logic extracted from complex methods
- Reusable functions for session/merge operations
- Decision functions that don't require I/O

**manager.rs** should retain:
- WorktreeManager struct and core impl methods
- Async orchestration methods
- Methods requiring subprocess execution
- Methods requiring mutable self access
- Git worktree operations
- Session creation and management coordination

### Extraction Strategy

For each method being extracted:
1. **Identify if it's pure logic or I/O**: Pure → extract, I/O → keep
2. **Check dependencies**: Can it work without `self`?
3. **Extract the logic**: Create standalone function
4. **Update original method**: Call the extracted function
5. **Test immediately**: Verify compilation and tests

## Notes

**Critical Observations**:
- This file contains 71 functions total but only 39 are in WorktreeManager impl
- The remaining 32 are test helper functions (lines 1324-2283)
- Debtmap correctly identified that WorktreeManager has 6 distinct responsibilities
- Many methods are already well-structured for extraction (pure validation, formatting)
- Some methods like `interpolate_merge_variables` are already pure and easy to extract
- Test code should remain in this file - we're only refactoring production code

**Key Success Factors**:
- Focus on extracting logic, not moving complexity
- Keep orchestration methods in WorktreeManager
- Maintain clear module boundaries
- Test after each extraction
- Commit working code incrementally

**Risks to Watch**:
- Circular dependencies between modules
- Breaking async orchestration by over-extraction
- Missing imports after extraction
- Test failures due to visibility changes

**Estimated Outcome**:
- `manager.rs`: ~800 lines (WorktreeManager + tests)
- `manager_validation.rs`: ~150 lines
- `manager_utilities.rs`: ~350 lines
- Total: ~1300 lines (reduction of ~1000 lines through better organization)
