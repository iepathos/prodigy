# Implementation Plan: Extract Core Operations from WorktreeManager God Object

## Problem Summary

**Location**: ./src/worktree/manager.rs:WorktreeManager:44
**Priority Score**: 106.19
**Debt Type**: God Object / High Complexity
**Current Metrics**:
- Lines of Code: 2547
- Functions: 92
- Cyclomatic Complexity: 309 (avg 3.36 per function, max 18)
- Coverage: 0.36% (1633 uncovered lines)
- God Object Score: 1.0 (84 methods, 6 fields, 6 responsibilities)

**Issue**: URGENT: The WorktreeManager is a classic god object with 2547 lines and 92 functions handling 6 distinct responsibilities: Construction, Core Operations, Validation, Communication, Data Access, and Persistence. The debtmap analysis recommends splitting by data flow into 3 focused modules with <30 functions each.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 61.8 points
- Maintainability Improvement: 10.62 points
- Test Effort: 163.3 (reduced testing burden through pure functions)

**Success Criteria**:
- [ ] Split WorktreeManager into 3-4 focused modules with single responsibilities
- [ ] Extract pure functions for data transformations (at least 15-20 functions)
- [ ] Separate I/O operations from business logic
- [ ] Each module has <400 lines and <30 functions
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Coverage improves to >70% for extracted pure functions

## Implementation Phases

This refactoring follows functional programming principles: extract pure logic, separate I/O from business logic, and build small, testable modules.

### Phase 1: Extract Pure Data Functions (Manager Data Access)

**Goal**: Extract all pure data access and query functions into a new `manager_queries.rs` module. These functions have no side effects and are easily testable.

**Changes**:
- Create `src/worktree/manager_queries.rs`
- Extract pure query functions (~12 functions):
  - `filter_sessions_by_status` (already pure!)
  - `load_state_from_file` (already pure!)
  - `collect_all_states` (pure I/O wrapper)
  - `get_session_state` (data access)
  - `get_parent_branch` (git query)
  - `get_current_branch` (git query)
  - `get_merge_target` (decision logic)
  - `get_commit_count_between_branches` (git query)
  - `get_merged_branches` (git query)
  - `get_git_root_path` (git query)
  - `get_worktree_for_branch` (git query)
  - `get_cleanup_config` (config access)
- Keep functions pure where possible (return Result, no mutations)
- Re-export from `manager.rs` with delegation pattern
- Move any helper functions used only by these queries

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings
- Verify all query functions still work through delegation

**Success Criteria**:
- [ ] New module created with ~12 functions (~240 lines estimated)
- [ ] All query functions extracted and working
- [ ] All tests pass without modification
- [ ] No clippy warnings
- [ ] Code compiles and runs
- [ ] Ready to commit

### Phase 2: Extract Construction and Configuration (Manager Construction)

**Goal**: Extract all construction, initialization, and test setup functions into a new `manager_construction.rs` module. This separates object lifecycle from operations.

**Changes**:
- Create `src/worktree/manager_construction.rs`
- Extract construction functions (~6 functions):
  - `new` (constructor)
  - `with_config` (builder pattern)
  - `create_checkpoint` (initialization)
  - Test helpers:
    - `create_test_worktree_state_with_checkpoint`
    - `create_test_session_state`
    - `create_mock_worktree_dirs`
    - `create_test_worktree_with_session_state`
    - `setup_test_git_repo`
    - `setup_test_worktree_manager`
- Keep WorktreeManager struct in `manager.rs` but move impl blocks
- Create associated functions and constants
- Ensure all test fixtures are properly organized

**Testing**:
- Run `cargo test --lib` to verify test helpers work
- Verify construction patterns still work
- Check that all tests can still create WorktreeManager instances

**Success Criteria**:
- [ ] New module created with ~10 functions (~120 lines estimated)
- [ ] All construction logic extracted
- [ ] Test helpers accessible and working
- [ ] All tests pass without modification
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 3: Extract Session Lifecycle Operations (Manager Operations)

**Goal**: Extract high-level session lifecycle operations into a new `manager_operations.rs` module. This separates orchestration from low-level operations.

**Changes**:
- Create `src/worktree/manager_operations.rs`
- Extract session lifecycle functions (~15 functions):
  - `list_sessions` (orchestration)
  - `list_git_worktree_sessions` (git I/O)
  - `list_detailed` (formatting)
  - `list_metadata_sessions` (metadata I/O)
  - `update_session_state` (persistence)
  - `restore_session` (restoration logic)
  - `update_checkpoint` (checkpoint management)
  - `list_interrupted_sessions` (filtering)
  - `mark_session_abandoned` (state transition)
  - `get_last_successful_command` (query with logic)
- Keep these as methods on WorktreeManager but in separate module
- Ensure clean dependencies (operations use queries)

**Testing**:
- Run `cargo test --lib` to verify session operations work
- Test session listing, updating, and restoration
- Verify checkpoint management still works

**Success Criteria**:
- [ ] New module created with ~15 functions (~400 lines estimated)
- [ ] Session lifecycle operations extracted
- [ ] All tests pass without modification
- [ ] Dependencies flow correctly (operations → queries)
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 4: Extract Merge and Cleanup Operations (Manager Merge)

**Goal**: Extract merge workflow and cleanup operations into a new `manager_merge.rs` module. This is the most complex subsystem and deserves its own module.

**Changes**:
- Create `src/worktree/manager_merge.rs`
- Extract merge and cleanup functions (~25 functions):
  - Merge workflow:
    - `merge_session` (main orchestration)
    - `execute_merge_workflow` (workflow execution)
    - `execute_claude_merge` (Claude integration)
    - `execute_custom_merge_workflow` (custom workflow)
    - `execute_merge_shell_command` (shell command)
    - `execute_merge_claude_command` (Claude command)
    - `init_merge_variables` (variable setup)
    - `interpolate_merge_variables` (string interpolation)
  - Merge decision logic:
    - `find_session_by_name` (lookup)
    - `determine_default_branch` (branch logic)
    - `select_default_branch` (selection logic)
    - `should_proceed_with_merge` (decision)
    - `is_permission_denied` (error checking)
  - Cleanup operations:
    - `cleanup_session` (cleanup orchestration)
    - `cleanup_all_sessions` (batch cleanup)
    - `cleanup_session_after_merge` (post-merge cleanup)
    - `detect_mergeable_sessions` (detection)
    - `cleanup_merged_sessions` (batch merge cleanup)
    - `is_branch_merged` (merge detection)
    - `perform_auto_cleanup` (auto cleanup)
    - `show_cleanup_diagnostics` (diagnostics)
    - `show_manual_cleanup_message` (messaging)
  - Finalization:
    - `finalize_merge_session` (merge finalization)
    - `update_session_state_after_merge` (state update)
  - Logging:
    - `log_execution_context` (context logging)
    - `log_claude_execution_details` (Claude logging)
- Extract pure helper functions for merge variable interpolation
- Separate decision logic from I/O operations

**Testing**:
- Run `cargo test --lib` to verify merge operations work
- Test merge workflow execution
- Test cleanup detection and execution
- Verify all merge-related tests pass

**Success Criteria**:
- [ ] New module created with ~25 functions (~600 lines estimated)
- [ ] Merge and cleanup operations extracted
- [ ] Pure functions separated from I/O
- [ ] All tests pass without modification
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Reorganize Remaining Core Logic and Tests

**Goal**: Organize remaining core logic in `manager.rs` and move all test functions to appropriate test modules. Final cleanup and verification.

**Changes**:
- Keep only essential WorktreeManager struct and integration methods in `manager.rs`
- Move all test helper functions to test modules:
  - `test_*` functions → appropriate test files
  - `assert_*` functions → test utilities
- Create clean module structure:
  ```
  src/worktree/
  ├── manager.rs (struct, imports, basic delegation)
  ├── manager_queries.rs (pure data access)
  ├── manager_construction.rs (construction, test setup)
  ├── manager_operations.rs (session lifecycle)
  └── manager_merge.rs (merge and cleanup)
  ```
- Update `mod.rs` to export all modules
- Ensure clean dependency graph (no circular dependencies)
- Add module-level documentation

**Testing**:
- Run `cargo test --lib` for full test suite
- Run `cargo clippy` for lint checks
- Run `cargo fmt` for formatting
- Verify all 92 functions are accounted for
- Check that coverage improves

**Success Criteria**:
- [ ] All functions properly organized into 5 files
- [ ] `manager.rs` is <300 lines (struct + delegation)
- [ ] Each module is <600 lines
- [ ] All tests pass without modification
- [ ] No clippy warnings
- [ ] Proper module documentation
- [ ] Clean dependency graph
- [ ] Ready to commit

## Implementation Strategy

### Functional Programming Approach

For each phase, follow these principles:
1. **Extract Pure Functions First**: Identify functions with no side effects
2. **Separate I/O from Logic**: Move git operations, file I/O to wrappers
3. **Create Data Transformation Pipelines**: Chain operations clearly
4. **Use Result Types**: No unwrap() or panic() in production code
5. **Test Pure Functions Directly**: Much easier than testing I/O

### Incremental Refactoring Pattern

For each function being moved:
1. Copy function to new module
2. Add any dependencies it needs
3. Update original to delegate to new module
4. Run tests to verify behavior unchanged
5. Once all functions moved, remove delegation
6. Commit the working change

### Module Organization

```rust
// manager.rs - Main struct and coordination
pub struct WorktreeManager { /* fields */ }
impl WorktreeManager {
    // Delegation methods that call other modules
}

// manager_queries.rs - Pure data access
pub(crate) fn filter_sessions_by_status(...) -> Vec<...> { }
pub(crate) fn load_state_from_file(...) -> Option<...> { }
// etc.

// manager_construction.rs - Object lifecycle
impl WorktreeManager {
    pub fn new(...) -> Result<Self> { }
    pub fn with_config(...) -> Result<Self> { }
}

// manager_operations.rs - Session operations
impl WorktreeManager {
    pub async fn list_sessions(...) -> Result<...> { }
    pub fn update_session_state(...) -> Result<()> { }
}

// manager_merge.rs - Merge and cleanup
impl WorktreeManager {
    pub fn merge_session(...) -> Result<...> { }
    pub fn cleanup_session(...) -> Result<()> { }
}
```

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` before making changes (baseline)
2. After extracting functions, run tests again
3. Run `cargo clippy` to catch any issues
4. Use `cargo watch -x test` for continuous testing

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --workspace --out Xml` - Regenerate coverage
3. Compare coverage before/after (expect improvement in new modules)
4. Verify all 92 functions are accounted for

**Coverage targets**:
- Pure query functions: >90% coverage
- Construction functions: >80% coverage
- Operations with I/O: >60% coverage
- Merge operations: >60% coverage

## Rollback Plan

If a phase fails:
1. Review the error output carefully
2. Check if tests need updating (should NOT be necessary)
3. If compilation fails, check for missing imports
4. If tests fail, check delegation logic
5. Use `git diff` to review changes
6. If stuck after 3 attempts, revert phase: `git reset --hard HEAD~1`
7. Document the issue and adjust plan

## Notes

### Key Insights from Debtmap Analysis

The debtmap identified 3 recommended splits:
1. **Construction** (120 lines, 6 methods) - Phase 2
2. **Core Operations** (1140 lines, 57 methods) - Phases 3 & 4
3. **Data Access** (240 lines, 12 methods) - Phase 1

Our plan follows this guidance but splits "Core Operations" into two logical modules (Operations and Merge) for better separation of concerns.

### Gotchas to Watch For

1. **Test Functions**: Many functions are test helpers (`test_*`, `setup_*`) - move these to test modules, not production modules
2. **Subprocess Dependencies**: WorktreeManager has a SubprocessManager field - ensure it's accessible where needed
3. **Async Functions**: Some functions are async - maintain async boundaries correctly
4. **Git Worktree State**: Be careful with git operations - they have side effects
5. **Claude Executor**: Merge operations use ClaudeExecutor - ensure proper dependency injection

### Success Indicators

After completion:
- [ ] 5 modules instead of 1 god object
- [ ] Each module <600 lines
- [ ] Pure functions testable in isolation
- [ ] I/O separated from business logic
- [ ] Complexity reduced by ~62 points
- [ ] Coverage increased (especially for pure functions)
- [ ] All tests passing
- [ ] No clippy warnings

### Dependencies Between Phases

Phase 1 (Queries) → Used by Phase 3 (Operations) → Used by Phase 4 (Merge)
Phase 2 (Construction) → Independent
Phase 5 (Cleanup) → Depends on all previous phases

This ordering ensures we can build incrementally without breaking dependencies.
