# Implementation Plan: Refactor WorktreeManager God Object into Focused Modules

## Problem Summary

**Location**: ./src/worktree/manager.rs:WorktreeManager:52
**Priority Score**: 149.83
**Debt Type**: God Object (3040 lines, 121 functions)

**Current Metrics**:
- Lines of Code: 3040
- Functions: 121
- Cyclomatic Complexity: 364 (avg 3.01, max 18)
- Coverage: 34.7%
- Uncovered Lines: 1984

**Issue**: The `WorktreeManager` class is a massive god object with 6 distinct responsibilities (Construction, Core Operations, Persistence, Data Access, Validation, Communication). This violates the Single Responsibility Principle and makes the code extremely difficult to test, maintain, and reason about. The debtmap analysis recommends splitting by data flow into 4 focused modules with <30 functions each.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 72.8 points
- Maintainability Improvement: 14.98 points
- Test Effort: 198.4 (current complexity to address)

**Success Criteria**:
- [x] Extract pure functions into separate testable modules
- [x] Create focused modules with single responsibilities
- [x] Each new module has <30 functions
- [x] Improve test coverage to >60% (from 34.7%)
- [x] All existing tests continue to pass
- [x] No clippy warnings
- [x] Proper formatting
- [x] Complexity reduced by at least 50 points

## Implementation Phases

This refactoring will be done in 5 incremental phases, extracting functionality in order of dependency (least coupled first, most coupled last).

### Phase 1: Extract Pure Parsing Functions into `parsing.rs`

**Goal**: Extract all pure parsing functions that have no dependencies on WorktreeManager state. These are the easiest to test and have zero coupling to other operations.

**Changes**:
- Create `src/worktree/parsing.rs` module
- Extract these pure functions (all static, no self reference):
  - `parse_worktree_output` - Parse git worktree list output
  - `split_into_worktree_blocks` - Split output into blocks
  - `parse_worktree_block` - Parse single block
- Move all related tests from manager.rs to parsing.rs:
  - `test_parse_worktree_output`
  - `test_parse_worktree_output_empty`
  - `test_parse_worktree_output_single_entry`
  - `test_parse_worktree_output_missing_branch`
  - `test_split_into_worktree_blocks`
  - `test_split_into_worktree_blocks_empty`
  - `test_split_into_worktree_blocks_single`
  - `test_parse_worktree_block_valid`
  - `test_parse_worktree_block_missing_path`
  - `test_parse_worktree_block_missing_branch`
  - `test_parse_worktree_block_extra_fields`
- Update `mod.rs` to export the new parsing module
- Update manager.rs to use `parsing::parse_worktree_output`

**Testing**:
- All parsing tests pass in their new module
- `cargo test --lib worktree::parsing` passes
- `cargo test --lib worktree::manager` still passes
- No new clippy warnings

**Success Criteria**:
- [x] parsing.rs created with 3 functions
- [x] 11 tests moved and passing
- [x] manager.rs reduced by ~200 lines
- [x] All tests pass
- [x] Ready to commit

### Phase 2: Extract Data Access Functions into `queries.rs`

**Goal**: Extract read-only query functions that access git and file system state but don't modify anything. These have minimal coupling and are independently testable.

**Changes**:
- Create `src/worktree/queries.rs` module
- Extract these query functions (all have &self, return Result):
  - `get_session_state` - Read session state from file
  - `get_current_branch` - Get current git branch
  - `get_parent_branch` - Get parent branch for a worktree
  - `get_merge_target` - Determine merge target branch
  - `get_commit_count_between_branches` - Count commits between branches
  - `get_merged_branches` - List merged branches
  - `get_git_root_path` - Get repository root path
  - `get_worktree_for_branch` - Find worktree path for branch
  - `check_branch_exists` - Verify branch exists
  - `is_branch_merged` - Check if branch is merged
  - `get_last_successful_command` - Get last successful command from checkpoint
  - `load_session_state` - Load session state from file
- Create a `GitQueries` struct to hold subprocess manager and paths
- Refactor these functions to use `GitQueries` instead of `WorktreeManager`
- Update manager.rs to delegate to `queries::GitQueries`

**Testing**:
- Add unit tests for each query function using mock git output
- Test edge cases: missing branches, invalid paths, empty results
- `cargo test --lib worktree::queries` passes
- `cargo test --lib worktree::manager` still passes

**Success Criteria**:
- [x] queries.rs created with ~12 functions
- [x] GitQueries struct with focused responsibility
- [x] manager.rs reduced by ~300 lines
- [x] All tests pass
- [x] Ready to commit

### Phase 3: Extract Command Building into `commands.rs`

**Goal**: Extract pure functions that build git/subprocess commands. These are simple string/arg builders with no side effects.

**Changes**:
- Create `src/worktree/commands.rs` module
- Extract these command builder functions:
  - `build_branch_check_command` - Build git branch check command
  - `build_commit_diff_command` - Build git diff command
  - `build_merge_check_command` - Build merge verification command
- Make these pure functions (no self reference)
- Add unit tests for each command builder
- Update manager.rs to use `commands::`

**Testing**:
- Test each command builder with various inputs
- Verify correct git command structure
- `cargo test --lib worktree::commands` passes
- `cargo test --lib worktree::manager` still passes

**Success Criteria**:
- [x] commands.rs created with 3 functions
- [x] Each function has unit tests
- [x] manager.rs reduced by ~50 lines
- [x] All tests pass
- [x] Ready to commit

### Phase 4: Extract Merge Operations into `merge_ops.rs`

**Goal**: Extract merge-related operations into a focused module. This is a cohesive set of functionality that belongs together.

**Changes**:
- Create `src/worktree/merge_ops.rs` module
- Create `MergeOperations` struct to encapsulate merge operations
- Extract these merge functions:
  - `merge_session` - Main merge entry point
  - `execute_merge_workflow` - Execute standard merge
  - `execute_custom_merge_workflow` - Execute custom merge workflow
  - `execute_claude_merge` - Execute Claude merge command
  - `execute_merge_shell_command` - Execute shell command in merge
  - `execute_merge_claude_command` - Execute Claude command in merge
  - `verify_merge_completion` - Verify merge succeeded
  - `validate_merge_preconditions` - Check if merge is safe
  - `validate_merge_success` - Validate merge result
  - `finalize_merge_session` - Finalize after merge
  - `update_session_state_after_merge` - Update state post-merge
  - `should_proceed_with_merge` - Decision logic for merge
  - `interpolate_merge_variables` - Variable substitution
  - `init_merge_variables` - Initialize merge context
  - `save_merge_checkpoint` - Save merge progress
- Extract helper functions:
  - `create_merge_checkpoint_manager` - Create checkpoint manager
  - `log_execution_context` - Log merge context
  - `log_claude_execution_details` - Log Claude execution
- Move related tests:
  - `test_claude_merge_command_construction`
  - `test_merge_session_success`
  - `test_merge_session_claude_cli_failure`
  - `test_merge_workflow_variable_interpolation`
  - `test_workflow_env_vars_in_merge_interpolation`
- Dependency injection: Pass GitQueries and parsing functions to MergeOperations

**Testing**:
- All merge tests pass in new module
- Integration tests verify merge workflow still works
- `cargo test --lib worktree::merge_ops` passes
- `cargo test --lib worktree::manager` still passes

**Success Criteria**:
- [x] merge_ops.rs created with ~20 functions
- [x] MergeOperations struct with focused responsibility
- [x] 5 tests moved and passing
- [x] manager.rs reduced by ~800 lines
- [x] All tests pass
- [x] Ready to commit

### Phase 5: Extract Session Management into `session_ops.rs`

**Goal**: Extract session lifecycle operations (create, update, cleanup) into a dedicated module. This is the final major responsibility to extract.

**Changes**:
- Create `src/worktree/session_ops.rs` module
- Create `SessionOperations` struct to encapsulate session operations
- Extract these session functions:
  - `create_session` - Create new session
  - `create_session_with_id` - Create session with specific ID
  - `list_sessions` - List all sessions
  - `list_git_worktree_sessions` - List git worktrees
  - `list_metadata_sessions` - List metadata sessions
  - `list_detailed` - List with detailed info
  - `list_interrupted_sessions` - List interrupted sessions
  - `cleanup_session` - Clean up single session
  - `cleanup_all_sessions` - Clean up all sessions
  - `cleanup_session_after_merge` - Post-merge cleanup
  - `cleanup_merged_sessions` - Clean merged sessions
  - `detect_mergeable_sessions` - Find mergeable sessions
  - `update_session_state` - Update session state
  - `restore_session` - Restore interrupted session
  - `mark_session_abandoned` - Mark session as abandoned
  - `create_checkpoint` - Create session checkpoint
  - `update_checkpoint` - Update checkpoint
- Extract helper functions:
  - `save_session_state` - Save session state
  - `save_session_state_with_original_branch` - Save with branch
  - `find_session_by_name` - Find session by name
  - `create_worktree_session` - Create session object
  - `determine_default_branch` - Get default branch
  - `select_default_branch` - Select main/master
  - `handle_auto_cleanup_if_enabled` - Auto cleanup logic
  - `perform_auto_cleanup` - Execute cleanup
  - `show_cleanup_diagnostics` - Show cleanup info
  - `show_manual_cleanup_message` - Show manual cleanup message
  - `get_cleanup_config` - Get cleanup configuration
- Move related tests:
  - `test_cleanup_config_defaults`
  - `test_get_cleanup_config_from_env`
  - `test_cleanup_session_after_merge_not_merged`
  - `test_detect_mergeable_sessions_empty`
  - `test_update_checkpoint_success`
  - `test_update_checkpoint_increments_iteration`
  - `test_list_detailed_empty`
  - `test_list_detailed_with_sessions`
  - `test_list_detailed_with_workflow_info`
  - `test_list_detailed_with_mapreduce_info`
- Dependency injection: Pass GitQueries, MergeOperations to SessionOperations

**Testing**:
- All session tests pass in new module
- Integration tests verify session lifecycle works
- `cargo test --lib worktree::session_ops` passes
- `cargo test --lib worktree::manager` still passes

**Success Criteria**:
- [x] session_ops.rs created with ~28 functions
- [x] SessionOperations struct with focused responsibility
- [x] 10 tests moved and passing
- [x] manager.rs reduced by ~1200 lines
- [x] All tests pass
- [x] Ready to commit

## Final State (After All Phases)

**Module Structure**:
```
src/worktree/
├── mod.rs                    (Updated exports)
├── manager.rs               (~400 lines - thin orchestration layer)
├── parsing.rs               (~200 lines - pure parsing functions)
├── queries.rs               (~350 lines - data access)
├── commands.rs              (~100 lines - command builders)
├── merge_ops.rs             (~800 lines - merge operations)
└── session_ops.rs           (~1200 lines - session management)
```

**WorktreeManager Role**:
- Construction and initialization
- Delegation to specialized modules
- Public API facade
- Configuration management

**Benefits**:
- Each module has <30 functions (actually <25)
- Clear single responsibilities
- Improved testability (pure functions isolated)
- Better organization (related functions together)
- Reduced coupling (explicit dependencies)
- Higher coverage (easier to test small modules)

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib worktree::<module>` for new module
2. Run `cargo test --lib worktree::manager` to verify no regressions
3. Run `cargo test --lib` to verify full test suite
4. Run `cargo clippy` to check for warnings
5. Verify code coverage for new module >80%

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib --exclude-files 'tests/*'` - Regenerate coverage, should be >60%
3. Verify manager.rs is <500 lines
4. Verify each new module is <30 functions
5. Verify no increase in cyclomatic complexity
6. Run integration tests to verify end-to-end workflows

## Rollback Plan

If a phase fails:
1. Identify the failing test or compilation error
2. Check if it's a simple fix (missing import, wrong function signature)
3. If simple: Fix and retry
4. If complex: Revert the phase with `git reset --hard HEAD~1`
5. Review what went wrong
6. Adjust the plan (may need to extract fewer functions or different grouping)
7. Retry with adjusted scope

## Notes

**Function Extraction Order**:
The phases are ordered by coupling/dependency:
1. **Parsing** - Zero dependencies, pure functions
2. **Queries** - Only depends on parsing (via subprocess output)
3. **Commands** - Pure builders, no dependencies
4. **Merge** - Depends on queries and commands
5. **Sessions** - Depends on everything (core orchestration)

**Avoiding Common Pitfalls**:
- Don't extract functions that are tightly coupled to WorktreeManager state
- Don't break up legitimate patterns (e.g., builder patterns, visitors)
- Don't add helper methods only used in tests (test them directly)
- Keep related test helpers with their functions
- Maintain backward compatibility in public API
- Use dependency injection to avoid circular dependencies

**Test Strategy**:
- Move tests with their functions to maintain coverage
- Add new tests for edge cases exposed during extraction
- Keep integration tests in manager.rs
- Use smaller unit tests in new modules

**Expected Complexity Reduction**:
- Parsing: ~20 complexity points
- Queries: ~30 complexity points
- Commands: ~5 complexity points
- Merge: ~80 complexity points (largest reduction)
- Sessions: ~100 complexity points
- **Total reduction: ~235 points (from 364 to ~129)**

This exceeds the target reduction of 72.8 points and will significantly improve maintainability.
