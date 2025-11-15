# Implementation Plan: Refactor MapReduceCoordinator God Object

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/coordination/executor.rs:file:0
**Priority Score**: 55.05
**Debt Type**: God Object (GodClass)
**Current Metrics**:
- Lines of Code: 1916
- Functions: 64 (28 impl methods, 10 trait methods)
- Cyclomatic Complexity: 188 total, max 16
- Coverage: 6.25%
- Responsibilities: 5 (Processing, Persistence, Construction, Utilities, Data Access)
- Fields: 16

**Issue**: URGENT: 1916 lines, 64 functions! This file violates the single responsibility principle with a god object pattern. The MapReduceCoordinator class handles too many concerns: agent lifecycle management, command execution, git operations, result aggregation, error handling, and display formatting. This creates high coupling, makes testing difficult, and violates functional programming principles by mixing I/O with business logic.

## Target State

**Expected Impact**:
- Complexity Reduction: 37.6 points
- Maintainability Improvement: 130.6 points
- Test Effort: 179.6 points (indicates high value from improved testability)

**Success Criteria**:
- [ ] Split into 3-4 focused modules with clear responsibilities
- [ ] Extract pure functions that can be unit tested independently
- [ ] Separate I/O operations from business logic
- [ ] Each new module has <500 lines and <20 functions
- [ ] No function exceeds 20 lines
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Command Execution Logic

**Goal**: Separate command execution (Claude, shell, write_file) into a dedicated module

**Changes**:
- Create `src/cook/execution/mapreduce/coordination/command_executor.rs`
- Move `execute_setup_step`, `execute_step_in_agent_worktree` into new `CommandExecutor` struct
- Move `get_step_display_name` to new module
- Extract pure functions for building environment variables
- Keep only references to `ClaudeExecutor` and `SubprocessManager` dependencies

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Verify command execution still works for shell, Claude, and write_file steps

**Success Criteria**:
- [ ] New `command_executor.rs` module created with <200 lines
- [ ] Command execution logic isolated from coordinator
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 2: Extract Agent Lifecycle Management

**Goal**: Separate agent creation, execution, and cleanup into a dedicated module

**Changes**:
- Create `src/cook/execution/mapreduce/coordination/agent_orchestrator.rs`
- Move `execute_agent_for_item`, `execute_agent_commands` into new `AgentOrchestrator` struct
- Move `merge_and_cleanup_agent` into new module
- Move timeout registration helpers (`register_agent_timeout`, `register_command_lifecycle`, `unregister_agent_timeout`)
- Extract pure function `build_item_variables` for testing

**Testing**:
- Run `cargo test --lib` to verify agent execution logic still works
- Verify agent worktree creation, execution, and cleanup

**Success Criteria**:
- [ ] New `agent_orchestrator.rs` module created with <400 lines
- [ ] Agent lifecycle isolated from coordinator
- [ ] Pure variable building function extracted
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 3: Extract Phase Execution Logic

**Goal**: Separate setup, map, and reduce phase execution into focused modules

**Changes**:
- Create `src/cook/execution/mapreduce/coordination/phase_executor.rs`
- Move `execute_setup_phase`, `execute_map_phase_internal`, `execute_reduce_phase` into new `PhaseExecutor` struct
- Move `load_work_items` into new module
- Move `build_reduce_interpolation_context` into new module
- Extract pure functions for phase orchestration logic

**Testing**:
- Run `cargo test --lib` to verify phase execution
- Test setup, map, and reduce phases independently

**Success Criteria**:
- [ ] New `phase_executor.rs` module created with <500 lines
- [ ] Phase execution isolated from coordinator
- [ ] Context building functions extracted as pure functions
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 4: Extract Display and Summary Logic

**Goal**: Separate user-facing display logic into a presentation module

**Changes**:
- Create `src/cook/execution/mapreduce/coordination/display.rs`
- Move `display_map_summary`, `display_reduce_summary` into new `DisplayFormatter` struct
- Extract pure functions for message formatting
- Separate formatting logic from UserInteraction calls

**Testing**:
- Run `cargo test --lib` to verify display logic
- Test summary formatting independently

**Success Criteria**:
- [ ] New `display.rs` module created with <100 lines
- [ ] Display logic separated from business logic
- [ ] Pure formatting functions extracted
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Extract Git Operations

**Goal**: Move git-specific operations into utilities

**Changes**:
- Move `get_worktree_commits`, `get_worktree_modified_files` into existing git operations module
- Update coordinator to use git operations service
- Remove direct git dependencies from coordinator

**Testing**:
- Run `cargo test --lib` to verify git operations
- Verify commit tracking and file modification detection

**Success Criteria**:
- [ ] Git operations consolidated in git module
- [ ] Coordinator only uses high-level git service
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 6: Simplify Coordinator

**Goal**: Reduce coordinator to a thin orchestration layer

**Changes**:
- Update coordinator to delegate to new modules
- Reduce coordinator to <300 lines
- Keep only high-level workflow orchestration
- Update module exports in `mod.rs`

**Testing**:
- Run `just ci` - Full CI checks
- Run `cargo tarpaulin` - Verify coverage improvement
- Integration test of full MapReduce workflow

**Success Criteria**:
- [ ] Coordinator reduced to <300 lines
- [ ] Clear delegation to focused modules
- [ ] All integration tests pass
- [ ] Coverage improved from 6.25%
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Verify no regressions in command execution
4. Test that MapReduce workflows still execute correctly

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Regenerate coverage (expect significant improvement)
3. `cargo build --release` - Ensure production build works
4. Manual test of MapReduce workflow with real data

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the compilation or test errors
3. Adjust the extraction strategy
4. Ensure all module boundaries are correct
5. Retry with adjusted approach

## Notes

### Key Principles
- **Extract pure functions first**: Variable building, formatting, validation logic
- **Separate I/O from logic**: Command execution, git operations at boundaries
- **Keep interfaces clean**: Each module exposes minimal public API
- **Maintain backward compatibility**: Public API of coordinator remains stable

### Gotchas
- The coordinator has 16 fields - need to carefully distribute dependencies to new modules
- Timeout enforcement is stateful - keep it in coordinator or pass to agent orchestrator
- Event logging happens throughout - each module needs access to logger
- Be careful with Arc cloning in async contexts

### Testing Notes
- Current coverage is only 6.25% - focus on extracting testable pure functions
- Many functions are >20 lines and have nested logic - break them down
- Error handling uses Result types - maintain this pattern
- Integration tests exist but unit test coverage is low
