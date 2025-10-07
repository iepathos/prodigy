# Implementation Plan: Refactor WorkflowExecutor God Object

## Problem Summary

**Location**: ./src/cook/workflow/executor.rs:WorkflowExecutor:695
**Priority Score**: 457.41
**Debt Type**: God Object (Complexity + Low Coverage)
**Current Metrics**:
- Lines of Code: 5650
- Functions: 182
- Cyclomatic Complexity: 652 (avg 3.58)
- Max Complexity: 29
- Coverage: 42.3% (3259 uncovered lines)
- Methods: 121
- Fields: 25
- Responsibilities: 7 distinct concerns

**Issue**: Critical god object with 5650 lines and 182 functions handling 7 different responsibilities. The code mixes I/O, validation, communication, construction, persistence, data access, and core operations. This creates a massive testing and maintenance burden.

## Target State

**Expected Impact**:
- Complexity Reduction: 130.4 points
- Maintainability Improvement: 45.74%
- Test Effort Reduction: 325.9 points

**Success Criteria**:
- [ ] WorkflowExecutor reduced to <1000 lines (primary orchestration only)
- [ ] Each extracted module has <800 lines
- [ ] Each module has single, clear responsibility
- [ ] Test coverage increases to >60% overall
- [ ] Max function complexity reduced to <15
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting maintained

## Implementation Phases

This refactoring follows the principle of extracting pure functions and separating I/O from business logic. We'll break the 5650-line god object into focused, testable modules over 5 incremental phases.

### Phase 1: Extract Variable & Context Management (Communication Layer)

**Goal**: Extract variable interpolation, context building, and formatting logic into a dedicated module. This is pure logic that can be easily tested.

**Changes**:
- Create `src/cook/workflow/executor/context.rs`
- Move variable interpolation functions:
  - `build_interpolation_context`
  - `build_iteration_context`
  - `build_commit_variables`
  - `log_variable_resolutions`
  - `format_variable_value_with_masking`
  - `format_variable_value`
  - `format_variable_value_static`
  - `format_env_var_for_logging`
  - `format_variable_for_logging`
- Move context initialization:
  - `init_workflow_context`
  - `prepare_env_vars`
- Extract pure context types:
  - `WorkflowContext` (if not already in separate file)
  - `VariableResolution`
- Approximately 15-20 functions, ~400 lines

**Testing**:
- Run `cargo test --lib executor::context` for new tests
- Run `cargo test --lib executor` to verify no regressions
- Verify test coverage for interpolation edge cases

**Success Criteria**:
- [ ] `context.rs` module compiles independently
- [ ] All variable interpolation tests pass
- [ ] No clippy warnings in new module
- [ ] WorkflowExecutor imports from context module
- [ ] executor.rs reduced by ~400 lines

### Phase 2: Extract Validation & Conditional Logic (Validation Layer)

**Goal**: Separate validation, condition evaluation, and decision-making logic from execution flow.

**Changes**:
- Create `src/cook/workflow/executor/validation.rs`
- Move validation functions:
  - `handle_validation`
  - `handle_incomplete_validation`
  - `handle_step_validation`
  - `execute_step_validation`
  - `execute_validation`
- Move conditional logic:
  - `handle_conditional_execution`
  - `evaluate_when_condition`
  - `should_skip_step_execution`
  - `should_fail_workflow_for_step`
- Move decision functions:
  - `determine_command_type`
  - `determine_execution_flags`
  - `determine_iteration_continuation`
  - `should_continue_iterations`
  - `should_stop_early_in_test_mode`
- Move test helpers:
  - `is_focus_tracking_test`
  - `is_test_mode_no_changes_command`
- Approximately 18-20 functions, ~350 lines

**Testing**:
- Test validation logic with various ValidationConfig inputs
- Test condition evaluation with complex boolean expressions
- Test decision logic with edge cases
- Verify all integration tests pass

**Success Criteria**:
- [ ] `validation.rs` module has clear public API
- [ ] Validation logic is pure and testable
- [ ] Condition evaluation tests cover edge cases
- [ ] No duplicate validation logic remains
- [ ] executor.rs reduced by ~350 lines

### Phase 3: Extract Step Execution Pipeline (Core Operations Layer)

**Goal**: Create a focused execution pipeline that separates step processing from orchestration coordination.

**Changes**:
- Create `src/cook/workflow/executor/step_executor.rs`
- Move step execution core:
  - `execute_step` (the main public entry point)
  - `execute_step_internal`
  - `execute_single_step`
  - `normalized_to_workflow_step`
  - `convert_workflow_command_to_step`
- Move execution tracking:
  - `initialize_step_tracking`
  - `track_and_commit_changes`
  - `track_and_update_session`
  - `log_step_execution_context`
  - `log_step_output`
- Move result handling:
  - `capture_step_output`
  - `write_output_to_file`
  - `finalize_step_result`
- Move retry logic:
  - `execute_with_retry_if_configured`
  - `execute_with_enhanced_retry`
- Approximately 16-18 functions, ~500 lines

**Testing**:
- Test step execution with various command types
- Test retry logic with transient failures
- Test output capture and tracking
- Verify commit tracking works correctly
- Run full workflow tests

**Success Criteria**:
- [ ] `step_executor.rs` has clean separation of concerns
- [ ] Retry logic is testable independently
- [ ] Output capture works for all command types
- [ ] Tracking functions are pure where possible
- [ ] executor.rs reduced by ~500 lines

### Phase 4: Extract Specialized Command Handlers (Command Layer)

**Goal**: Consolidate and enhance command-specific execution logic that's already partially in `commands.rs`.

**Changes**:
- Enhance existing `src/cook/workflow/executor/commands.rs`
- Move remaining command execution:
  - `execute_command_by_type` (coordinator)
  - `execute_claude_command`
  - `execute_shell_command`
  - `execute_shell_with_retry`
  - `execute_shell_for_step`
  - `execute_test_command`
  - `execute_foreach_command`
  - `execute_goal_seek_command`
  - `execute_handler_command`
  - `execute_mapreduce`
- Move specialized execution:
  - `handle_test_mode_execution`
- Move helper functions:
  - `json_to_attribute_value`
  - `json_to_attribute_value_static`
- Approximately 12-15 functions, ~450 lines

**Testing**:
- Test each command type with various inputs
- Test error handling for each command
- Test MapReduce integration
- Test goal-seeking behavior
- Verify shell command execution with retries

**Success Criteria**:
- [ ] All command types have dedicated test coverage
- [ ] Command execution is independent of orchestration
- [ ] Error handling is consistent across types
- [ ] commands.rs is well-organized by command type
- [ ] executor.rs reduced by ~450 lines

### Phase 5: Extract Construction & Configuration (Construction Layer)

**Goal**: Separate builder pattern methods and configuration setup from runtime execution.

**Changes**:
- Create `src/cook/workflow/executor/builder.rs`
- Move builder methods:
  - `new` (with builder pattern)
  - `with_command_registry`
  - `with_resume_context`
  - `with_workflow_path`
  - `with_dry_run`
  - `with_environment_config`
  - `with_checkpoint_manager`
  - `with_sensitive_patterns`
  - `with_test_config`
  - `with_test_config_and_git`
- Move configuration helpers:
  - `create_auto_commit`
  - `create_validation_handler`
  - `create_test_executor`
  - `restore_error_recovery_state`
- Move checkpoint functions:
  - `save_retry_checkpoint`
- Move dry-run functions:
  - `display_dry_run_info`
  - `display_dry_run_summary`
- Move message builders:
  - `build_step_error_message`
  - `generate_commit_message`
- Approximately 18-20 functions, ~300 lines

**Testing**:
- Test builder pattern with various configurations
- Test dry-run mode behavior
- Test checkpoint save/restore
- Verify all configuration options work
- Integration test with full workflow

**Success Criteria**:
- [ ] Builder pattern is clean and fluent
- [ ] Configuration is validated at build time
- [ ] Dry-run mode fully functional
- [ ] Checkpoint logic is correct
- [ ] executor.rs reduced to core orchestration (~900 lines)

## Final Structure

After all phases, the module structure will be:

```
src/cook/workflow/executor/
├── mod.rs (~900 lines) - Core orchestration, main execute() loop
├── builder.rs (~300 lines) - Construction & configuration
├── context.rs (~400 lines) - Variable interpolation & context management
├── validation.rs (~350 lines) - Validation & conditional logic
├── step_executor.rs (~500 lines) - Step execution pipeline
├── commands.rs (~450 lines) - Command-specific execution
├── failure_handler.rs (existing ~14k) - Error handling
├── orchestration.rs (existing ~12k) - High-level orchestration
└── pure.rs (existing ~21k) - Pure functions
```

Total lines remain ~5650, but split into 9 focused modules averaging ~628 lines each.

## Testing Strategy

**For each phase**:
1. Create new module file
2. Move functions incrementally (5-10 at a time)
3. Update imports in executor.rs
4. Run `cargo test --lib` after each batch
5. Fix any compilation errors immediately
6. Run `cargo clippy` to catch issues
7. Commit working state

**After each phase**:
1. `cargo test --all` - All tests pass
2. `cargo clippy -- -D warnings` - No warnings
3. `cargo fmt` - Format code
4. `just ci` - Full CI checks pass
5. Manual testing of affected workflows

**Final verification**:
1. `just ci` - Complete CI suite passes
2. `cargo tarpaulin --lib --out Html` - Coverage report generated
3. Verify coverage improvement (target: >60%)
4. Run sample workflows end-to-end
5. Review debtmap output for improvement

## Rollback Plan

If a phase fails:
1. Review the error carefully - is it a simple import issue?
2. If yes, fix the import and continue
3. If no, run `git status` to see changes
4. Run `git diff` to review what was changed
5. Revert using `git restore <files>` for specific files
6. Or `git reset --hard HEAD` if fully committed phase
7. Document the failure reason
8. Revise the plan for that phase
9. Retry with smaller batches (2-3 functions at a time)

## Module Dependency Flow

The extracted modules will have clear dependency flow:

```
builder.rs (constructs) → WorkflowExecutor
    ↓
WorkflowExecutor.execute() (orchestrates)
    ↓
step_executor.rs (processes steps)
    ↓
validation.rs (checks conditions) → commands.rs (executes)
    ↓
context.rs (interpolates variables)
    ↓
pure.rs (pure functions)
```

## Important Notes

### Functional Programming Approach
- **Pure functions first**: Extract functions with no side effects
- **I/O at boundaries**: Keep git operations, file I/O, and command execution at module edges
- **Testability**: Every extracted function should be independently testable
- **Immutable context**: Pass context by reference, return new state

### Avoiding Common Pitfalls
- **Don't just move code**: Look for opportunities to simplify during extraction
- **Keep related functions together**: Group by responsibility, not alphabetically
- **Maintain test coverage**: Tests should get easier to write, not harder
- **Preserve behavior**: Use git diff and tests to verify no changes in behavior

### Success Indicators
- Smaller, focused modules (each <800 lines)
- Higher test coverage (target >60%)
- Lower cyclomatic complexity (functions <15)
- Clearer responsibilities (one per module)
- Easier to understand and modify

### Performance Considerations
- Variable interpolation is hot path - keep optimized
- Avoid unnecessary cloning during refactor
- Maintain Arc<> patterns for shared state
- Don't add indirection that impacts performance

## Timeline Estimate

- Phase 1: 2-3 hours (straightforward pure functions)
- Phase 2: 2-3 hours (validation logic requires care)
- Phase 3: 3-4 hours (core execution, most complex)
- Phase 4: 2-3 hours (command handlers mostly exists)
- Phase 5: 2-3 hours (builder pattern is standard)

**Total**: 11-16 hours of focused work

## Next Steps

After completing this plan:
1. Run `/prodigy-debtmap-implement .prodigy/plan-item_0.md`
2. Execute each phase incrementally
3. Verify improvement with debtmap analysis
4. Document any learnings for future refactors
