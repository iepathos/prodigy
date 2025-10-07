# Implementation Plan: Refactor WorktreeManager God Object

## Problem Summary

**Location**: ./src/worktree/manager.rs:WorktreeManager:1
**Priority Score**: 130.40
**Debt Type**: God Object
**Current Metrics**:
- Lines of Code: 2837
- Functions: 107
- Cyclomatic Complexity: 343 (avg 3.2 per function, max 18)
- Coverage: 36.45% (1802 uncovered lines)
- God Object Score: 1.0 (confirmed god object)
- Responsibilities: 6 (Construction, Data Access, Core Operations, Persistence, Validation, Communication)

**Issue**: This is a critical god object with 2837 lines and 107 functions handling 6 distinct responsibilities. The file violates single responsibility principle, making it difficult to test, maintain, and reason about. The debtmap analysis recommends splitting into 4 focused modules with <30 functions each.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 68.6 points
- Maintainability Improvement: 13.04 points
- Test Effort Required: 180.2 points

**Success Criteria**:
- [ ] WorktreeManager reduced to <500 lines (facade/coordinator only)
- [ ] 3-4 focused modules created, each <400 lines
- [ ] Each module has a single, clear responsibility
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Coverage maintained or improved (target: >40%)

## Implementation Phases

### Phase 1: Extract Builder/Construction Module

**Goal**: Separate all construction and initialization logic into a dedicated builder module

**Changes**:
- Create `src/worktree/builder.rs` module
- Extract 15 construction-related methods (~300 lines):
  - `new`
  - `create_session`
  - `create_session_with_id`
  - `create_worktree_session`
  - `build_branch_check_command`
  - `build_commit_diff_command`
  - `build_claude_environment_variables`
  - `create_claude_executor`
  - `build_merge_check_command`
  - `create_checkpoint`
  - `create_merge_checkpoint_manager`
  - `create_test_worktree_state_with_checkpoint`
  - `create_test_session_state`
  - `create_mock_worktree_dirs`
  - `create_test_worktree_with_session_state`
- Create `WorktreeBuilder` struct
- Implement builder pattern for WorktreeManager construction
- Update `manager.rs` to use builder for construction
- Move test helper construction methods to test utilities

**Testing**:
- Run `cargo test --lib worktree` to verify all worktree tests pass
- Run `cargo test --lib` to verify no regressions in other modules
- Verify builder pattern works correctly in integration tests

**Success Criteria**:
- [ ] `src/worktree/builder.rs` exists with ~300 lines
- [ ] `WorktreeManager::new()` uses builder internally
- [ ] All construction logic removed from manager.rs
- [ ] All existing tests pass without modification
- [ ] No clippy warnings
- [ ] Code compiles successfully

### Phase 2: Extract Query/Data Access Module

**Goal**: Separate all read-only query operations into a dedicated query module

**Changes**:
- Create `src/worktree/queries.rs` module
- Extract 12 data access methods (~240 lines):
  - `get_session_state`
  - `get_parent_branch`
  - `get_current_branch`
  - `get_merge_target`
  - `get_commit_count_between_branches`
  - `get_merged_branches`
  - `get_git_root_path`
  - `get_worktree_for_branch`
  - `get_last_successful_command`
  - `get_cleanup_config`
  - `setup_test_git_repo` (test helper)
  - `setup_test_worktree_manager` (test helper)
- Create pure functions that take `&WorktreeManager` as first parameter
- Separate git queries from state queries
- Move test setup helpers to test utilities module

**Testing**:
- Run `cargo test --lib worktree` to verify worktree tests pass
- Run `cargo test --lib` for full test suite
- Verify query methods work correctly through facade

**Success Criteria**:
- [ ] `src/worktree/queries.rs` exists with ~240 lines
- [ ] All query methods are pure (no mutation)
- [ ] Clear separation between git and state queries
- [ ] All tests pass without modification
- [ ] No clippy warnings
- [ ] manager.rs reduced by ~240 lines

### Phase 3: Extract Session Operations Module (Part 1)

**Goal**: Move session lifecycle operations (list, filter, update) to dedicated module

**Changes**:
- Create `src/worktree/session_ops.rs` module
- Extract session management methods (~380 lines):
  - `filter_sessions_by_status` (already pure!)
  - `collect_all_states` (already pure!)
  - `load_state_from_file` (already pure!)
  - `with_config`
  - `update_session_state`
  - `list_sessions`
  - `list_git_worktree_sessions`
  - `list_detailed`
  - `list_metadata_sessions`
  - `find_session_by_name`
  - `list_interrupted_sessions`
  - `update_checkpoint`
  - `restore_session`
  - `mark_session_abandoned`
- Organize into logical groups:
  - Session listing/filtering
  - Session state management
  - Checkpoint operations
- Extract pure logic where possible

**Testing**:
- Run `cargo test --lib worktree::session` tests
- Verify session listing and filtering works
- Test checkpoint operations

**Success Criteria**:
- [ ] `src/worktree/session_ops.rs` exists with ~380 lines
- [ ] Session lifecycle clearly separated from merge/cleanup
- [ ] Pure functions extracted where possible
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] manager.rs reduced by ~380 lines

### Phase 4: Extract Merge Operations Module

**Goal**: Separate all merge-related operations into dedicated module

**Changes**:
- Create `src/worktree/merge_ops.rs` module
- Extract merge operations (~380 lines):
  - `merge_session`
  - `determine_default_branch`
  - `select_default_branch`
  - `should_proceed_with_merge`
  - `execute_merge_workflow`
  - `execute_claude_merge`
  - `is_permission_denied`
  - `finalize_merge_session`
  - `update_session_state_after_merge`
  - `is_branch_merged`
  - `init_merge_variables`
  - `execute_merge_shell_command`
  - `execute_merge_claude_command`
  - `interpolate_merge_variables`
  - `log_execution_context`
  - `log_claude_execution_details`
  - `execute_custom_merge_workflow`
- Organize into:
  - Merge workflow execution
  - Merge validation
  - Post-merge finalization
- Extract pure merge variable interpolation logic

**Testing**:
- Run merge-specific tests
- Verify custom merge workflows work
- Test merge variable interpolation
- Verify Claude merge command construction

**Success Criteria**:
- [ ] `src/worktree/merge_ops.rs` exists with ~380 lines
- [ ] Merge logic clearly separated
- [ ] Pure interpolation functions extracted
- [ ] All merge tests pass
- [ ] No clippy warnings
- [ ] manager.rs reduced by ~380 lines

### Phase 5: Extract Cleanup Operations Module

**Goal**: Separate cleanup operations and finalize refactoring

**Changes**:
- Create `src/worktree/cleanup_ops.rs` module
- Extract cleanup operations (~200 lines):
  - `perform_auto_cleanup`
  - `show_cleanup_diagnostics`
  - `show_manual_cleanup_message`
  - `cleanup_session`
  - `cleanup_all_sessions`
  - `detect_mergeable_sessions`
  - `cleanup_merged_sessions`
  - `cleanup_session_after_merge`
- Move `CleanupConfig` and `CleanupPolicy` types to this module
- Extract pure cleanup decision logic
- Refactor `WorktreeManager` to be a thin facade:
  - Delegate to specialized modules
  - Hold only essential state (base_dir, repo_path, subprocess, config)
  - Provide clean public API

**Testing**:
- Run cleanup-specific tests
- Verify auto-cleanup logic works
- Test manual cleanup flows
- Run full test suite: `cargo test --lib`
- Run clippy: `cargo clippy`
- Check formatting: `cargo fmt --check`

**Success Criteria**:
- [ ] `src/worktree/cleanup_ops.rs` exists with ~200 lines
- [ ] `WorktreeManager` is <500 lines (facade only)
- [ ] All modules properly expose their public APIs
- [ ] All 107 original functions still accessible through facade or modules
- [ ] All tests pass without modification
- [ ] No clippy warnings
- [ ] Proper formatting throughout
- [ ] manager.rs reduced from 2837 to <500 lines

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib worktree` to verify worktree-specific tests pass
2. Run `cargo test --lib` to verify no regressions in other modules
3. Run `cargo clippy --all-targets` to check for warnings
4. Run `cargo fmt` to ensure proper formatting
5. Verify extracted functions maintain same behavior
6. Check that error handling is preserved

**After each phase**:
- Commit the changes with descriptive message
- Document any issues encountered
- Update IMPLEMENTATION_PLAN.md with progress

**Final verification**:
1. `cargo build --release` - Ensure clean build
2. `cargo test --all` - All tests pass
3. `cargo clippy --all-targets` - No warnings
4. `cargo tarpaulin --lib` - Check coverage improvement
5. Manual smoke test: `prodigy worktree ls`, `prodigy worktree clean --dry-run`

## Rollback Plan

If a phase fails:
1. Identify the specific failure (compilation, tests, clippy)
2. Review error messages and test failures
3. Attempt to fix in place if issue is obvious (typo, missing import)
4. If fix is not obvious, revert the phase: `git reset --hard HEAD~1`
5. Document the failure in this plan
6. Reassess the approach:
   - Can the module be split differently?
   - Are dependencies too coupled?
   - Should we extract in smaller chunks?
7. Adjust the plan and retry

**Common issues to watch for**:
- Circular dependencies between new modules
- Missing imports in tests
- Visibility issues (pub vs pub(crate))
- Moved methods that depend on private state
- Test helpers that need to be in multiple modules

## Module Organization

After all phases, the worktree module will be organized as:

```
src/worktree/
├── mod.rs                    (module declarations and re-exports)
├── manager.rs                (<500 lines - facade/coordinator)
├── builder.rs                (~300 lines - construction logic)
├── queries.rs                (~240 lines - read-only queries)
├── session_ops.rs            (~380 lines - session lifecycle)
├── merge_ops.rs              (~380 lines - merge operations)
├── cleanup_ops.rs            (~200 lines - cleanup operations)
├── parsing.rs                (existing - unchanged)
└── types.rs                  (existing - may need updates)
```

**Dependency Flow**:
- `manager.rs` depends on all operation modules (facade pattern)
- Operation modules depend on `types.rs` and `parsing.rs`
- No circular dependencies between operation modules
- Test helpers may be shared through test-only modules

## Notes

### Key Principles

1. **Incremental Changes**: Each phase should compile and pass tests independently
2. **Preserve Behavior**: All existing functionality must work exactly as before
3. **Test Preservation**: No test modifications required (tests should pass through facade)
4. **Pure Functions**: Extract pure logic wherever possible for easier testing
5. **Clear Boundaries**: Each module should have a single, well-defined responsibility

### Challenges to Watch For

1. **Shared State**: WorktreeManager holds subprocess manager and config - may need Arc/Rc
2. **Test Helpers**: Many test helpers are mixed with production code - separate cleanly
3. **Error Context**: Preserve error context when moving functions between modules
4. **Git Operations**: Be careful with subprocess manager access patterns
5. **Visibility**: Balance between pub and pub(crate) for internal APIs

### Functional Programming Opportunities

1. **Pure Filtering**: `filter_sessions_by_status` is already pure - use as pattern
2. **Query Composition**: Chain queries instead of nested method calls
3. **Merge Variable Interpolation**: Extract to pure string transformation
4. **Session State Transformations**: Use immutable updates where possible
5. **Validation Logic**: Extract predicates like `is_branch_merged`

### Success Indicators

After completion, we should see:
- WorktreeManager as clean facade (~400-500 lines)
- 5-6 focused modules, each <400 lines
- Improved testability (more pure functions)
- Better separation of concerns
- Easier to navigate and understand
- Foundation for future improvements (better error types, async operations)
