# Implementation Plan: Refactor Worktree Command Module

## Problem Summary

**Location**: ./src/cli/commands/worktree.rs:file:0
**Priority Score**: 57.31
**Debt Type**: God Object / File-Level Complexity

**Current Metrics**:
- Lines of Code: 464
- Functions: 9
- Cyclomatic Complexity: 96 (avg: 10.67, max: 27)
- Coverage: 0%
- Responsibilities: 2 (Utilities, Parsing & Input)

**Issue**: This file is a god object mixing CLI argument handling, business logic, formatting output, and utility functions. The recommendation is to split by data flow into: 1) Input/parsing functions 2) Core logic/transformation 3) Output/formatting. The high complexity and zero test coverage make this code risky to modify and difficult to maintain.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 19.2 points
- Maintainability Improvement: 5.73 points
- Test Effort Reduction: 46.4 points

**Success Criteria**:
- [ ] File split into focused modules (<200 lines each)
- [ ] Pure business logic extracted and testable
- [ ] Test coverage reaches >60% for core logic
- [ ] All existing functionality preserved
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting maintained

## Implementation Phases

### Phase 1: Extract Pure Parsing/Utility Functions

**Goal**: Extract the pure, stateless utility functions (`parse_duration`) into a separate testable module.

**Changes**:
- Create new module `src/cli/commands/worktree/utils.rs`
- Move `parse_duration` function to utils module
- Add comprehensive unit tests for `parse_duration`
- Update imports in main worktree.rs file

**Testing**:
- Write unit tests for all duration formats: "1ms", "5s", "10m", "2h", "7d"
- Test error cases: invalid format, invalid numbers, empty strings
- Run `cargo test --lib` to verify all tests pass

**Success Criteria**:
- [ ] `parse_duration` moved to utils module with proper documentation
- [ ] 100% test coverage for `parse_duration` (all branches tested)
- [ ] All existing tests pass
- [ ] Clippy clean
- [ ] Code compiles and formats correctly
- [ ] Ready to commit

### Phase 2: Extract Business Logic - Session Operations

**Goal**: Separate core business logic for session operations into a dedicated operations module.

**Changes**:
- Create `src/cli/commands/worktree/operations.rs`
- Extract pure functions for:
  - `list_sessions_operation` - wraps manager.list_sessions with filtering/sorting logic
  - `merge_session_operation` - handles single session merge logic
  - `merge_all_sessions_operation` - handles batch merge logic with error aggregation
  - `cleanup_session_operation` - session cleanup logic
- These functions take dependencies as parameters (no direct instantiation)
- Return structured results (not printing directly)

**Testing**:
- Write integration tests for each operation function
- Mock WorktreeManager using test fixtures
- Test error handling and edge cases
- Verify Result types propagate correctly

**Success Criteria**:
- [ ] Business logic extracted to operations module
- [ ] Functions are pure (dependencies injected, no side effects)
- [ ] Test coverage >60% for operations module
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 3: Extract Business Logic - Cleanup Operations

**Goal**: Extract cleanup-related business logic into the operations module.

**Changes**:
- Add to `operations.rs`:
  - `filter_old_sessions` - pure function to filter sessions by age
  - `cleanup_old_sessions_operation` - orchestrates cleanup of old sessions
  - `cleanup_mapreduce_operation` - handles MapReduce cleanup logic
  - `cleanup_orphaned_operation` - handles orphaned worktree cleanup
- Extract logic from `cleanup_old_worktrees`, `run_mapreduce_cleanup`, `run_worktree_clean_orphaned`
- Keep presentation/I/O separate

**Testing**:
- Test session age filtering with various time ranges
- Test cleanup orchestration with mocked managers
- Test MapReduce cleanup scenarios
- Test orphaned worktree scenarios

**Success Criteria**:
- [ ] Cleanup logic extracted and testable
- [ ] Test coverage >60% for cleanup operations
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 4: Create Presentation Layer

**Goal**: Separate output formatting and user interaction into a presentation module.

**Changes**:
- Create `src/cli/commands/worktree/presentation.rs`
- Extract all printing/formatting logic:
  - `format_sessions_table` - formats session list as table
  - `print_merge_result` - formats merge success/failure messages
  - `print_cleanup_summary` - formats cleanup results
  - `prompt_user_confirmation` - handles user prompts
- These are pure functions taking data and returning formatted strings
- CLI command functions call presentation functions for output

**Testing**:
- Test table formatting with various session data
- Test message formatting for different scenarios
- Verify output strings match expected format
- No need for complex mocking (pure string formatting)

**Success Criteria**:
- [ ] All output formatting extracted to presentation module
- [ ] Functions are pure (data in, strings out)
- [ ] Test coverage >80% for presentation module
- [ ] Output matches original format
- [ ] Ready to commit

### Phase 5: Refactor CLI Command Functions

**Goal**: Simplify CLI command functions to orchestrate operations and presentation only.

**Changes**:
- Refactor `run_worktree_ls`, `run_worktree_merge`, `run_worktree_clean`, `run_worktree_clean_orphaned`
- Each function becomes thin orchestration:
  1. Parse/validate CLI arguments (already done by clap)
  2. Initialize dependencies (WorktreeManager, etc.)
  3. Call operation functions
  4. Call presentation functions
  5. Handle errors
- Maximum 20-30 lines per function
- No business logic in these functions

**Testing**:
- Integration tests verifying end-to-end flows
- Error handling tests
- Verify all CLI argument combinations work

**Success Criteria**:
- [ ] CLI functions are thin orchestrators (<30 lines each)
- [ ] No business logic in CLI layer
- [ ] All functionality preserved
- [ ] Integration tests pass
- [ ] Ready to commit

### Phase 6: Create Module Structure and Documentation

**Goal**: Organize the refactored code into a proper module structure with clear public API.

**Changes**:
- Create `src/cli/commands/worktree/mod.rs` with:
  - Public re-exports of necessary functions
  - Module documentation explaining structure
  - Clear separation of public vs private functions
- Update `src/cli/commands/worktree.rs` to become `src/cli/commands/worktree/cli.rs`
- Ensure module organization follows:
  ```
  src/cli/commands/worktree/
  ├── mod.rs           (public API, module docs)
  ├── cli.rs           (CLI command handlers)
  ├── operations.rs    (business logic)
  ├── presentation.rs  (output formatting)
  └── utils.rs         (utility functions)
  ```

**Testing**:
- Verify public API is minimal and clear
- Ensure private functions are not exposed
- Run full test suite
- Run `just ci` to verify all checks pass

**Success Criteria**:
- [ ] Module structure properly organized
- [ ] Clear module documentation
- [ ] Public API is minimal
- [ ] All tests pass
- [ ] Full CI passes (just ci)
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Write tests BEFORE refactoring (TDD where possible)
2. Run `cargo test --lib` after each change
3. Run `cargo clippy` to check for warnings
4. Run `cargo fmt` to maintain formatting
5. Verify no regressions in existing functionality

**Final verification**:
1. `just ci` - Full CI checks pass
2. `cargo test --all` - All tests pass
3. `cargo tarpaulin --workspace` - Verify coverage improvement
4. Manually test each CLI command:
   - `prodigy worktree ls`
   - `prodigy worktree merge --name <name>`
   - `prodigy worktree clean --all`
   - `prodigy worktree clean-orphaned`

**Coverage Goals**:
- utils.rs: 100% (pure functions)
- operations.rs: >60% (business logic)
- presentation.rs: >80% (formatting logic)
- cli.rs: >40% (orchestration/integration)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure carefully:
   - What broke?
   - Why did the test fail?
   - What assumption was incorrect?
3. Adjust the approach:
   - Break the phase into smaller steps
   - Add more tests first
   - Simplify the refactoring
4. Retry with the adjusted plan

**Important**: Each phase should be independently revertable. Always commit working code after each phase.

## Notes

### Key Functional Programming Principles Applied

1. **Pure Functions First**: Extract stateless utility and business logic functions
2. **Separate I/O from Logic**: Keep printing/formatting separate from computation
3. **Dependency Injection**: Pass dependencies as parameters, not global instantiation
4. **Immutability**: Return new data structures rather than mutating in place
5. **Composition**: Build complex operations from simple, testable functions

### Potential Challenges

1. **WorktreeManager Integration**: The manager is currently instantiated in each CLI function. Consider passing it as a parameter or creating a factory pattern.

2. **Error Context**: Ensure error messages remain helpful after refactoring. Use `.context()` to add meaningful error information.

3. **Testing MapReduce Cleanup**: This involves complex file system operations. May need to use temporary directories or mocking for thorough testing.

4. **Maintaining Output Format**: The presentation layer must preserve exact output format to avoid breaking scripts/tools that parse output.

### Success Indicators

After completion:
- File reduced from 464 lines to ~50-100 lines (CLI orchestration only)
- 4 focused modules, each <200 lines
- Test coverage improved from 0% to >50% overall
- Complexity reduced by ~19 points (as predicted by debtmap)
- Code is significantly more maintainable and testable
- Foundation for future improvements is established
