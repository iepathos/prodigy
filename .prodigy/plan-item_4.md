# Implementation Plan: Refactor Orchestrator God Object

## Problem Summary

**Location**: ./src/cook/orchestrator.rs:DefaultCookOrchestrator:105
**Priority Score**: 143.92
**Debt Type**: God Object
**Current Metrics**:
- Lines of Code: 3136
- Functions: 82
- Cyclomatic Complexity: 327 (avg: 3.99, max: 29)
- Coverage: 21.95%
- Uncovered Lines: 2447

**Issue**: URGENT god object with 3136 lines and 82 functions. The `DefaultCookOrchestrator` has 57 methods across 6 distinct responsibilities (Construction, Core Operations, Computation, Processing, Validation, Data Access). Analysis recommends splitting by data flow into focused modules with <30 functions each.

## Target State

**Expected Impact**:
- Complexity Reduction: 65.4 points
- Maintainability Improvement: 14.39 points
- Test Effort: 244.7 (current complexity to test)

**Success Criteria**:
- [ ] Orchestrator split into 3-4 focused modules, each <500 lines
- [ ] Each module has single clear responsibility
- [ ] Pure functions extracted from I/O-heavy orchestration code
- [ ] Coverage improves from 21.95% to >40%
- [ ] Cyclomatic complexity reduced by >50%
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting maintained

## Implementation Phases

### Phase 1: Extract Construction and Configuration Logic

**Goal**: Separate orchestrator construction/setup from execution logic into a dedicated builder module.

**Changes**:
- Create new module `src/cook/orchestrator/builder.rs`
- Extract constructor and configuration methods:
  - `new()`
  - `create_env_config()`
  - `create_workflow_executor()`
  - `create_workflow_state_base()`
  - `build_command()`
  - `create_test_cook_command()`
  - `with_test_config()`
- Create `OrchestratorBuilder` struct with fluent API
- Update `DefaultCookOrchestrator` to use builder pattern
- Keep original `new()` as wrapper for backward compatibility

**Estimated Lines**: ~150 lines in new module

**Testing**:
- Run `cargo test --lib orchestrator` to verify construction tests pass
- Run `cargo build` to ensure no compilation errors
- Verify builder can create orchestrator instances correctly

**Success Criteria**:
- [ ] Builder module created with all construction logic
- [ ] Backward compatibility maintained via `new()` wrapper
- [ ] All existing tests pass
- [ ] Code compiles without warnings
- [ ] Ready to commit

### Phase 2: Extract Workflow Classification and Normalization

**Goal**: Separate workflow type detection and normalization into pure functions in a dedicated module.

**Changes**:
- Create new module `src/cook/orchestrator/workflow_classifier.rs`
- Extract pure classification functions:
  - `classify_workflow_type()` - detect Standard/Iterative/Structured/MapReduce
  - `normalize_workflow()` - convert simple commands to structured format
  - `determine_commit_required()` - extract commit requirement logic
  - `convert_command_to_step()` - command conversion logic
- Extract helper functions:
  - `find_files_matching_pattern()`
  - `matches_glob_pattern()`
- Make all functions pure (take inputs, return results, no side effects)
- Add comprehensive unit tests for classification logic

**Estimated Lines**: ~200 lines in new module

**Testing**:
- Run existing classification tests (moved to new module)
- Add new tests for edge cases
- Run `cargo test --lib workflow_classifier`
- Verify all workflow types are correctly detected

**Success Criteria**:
- [ ] Workflow classifier module with pure functions
- [ ] 10+ unit tests covering all workflow types
- [ ] All classification logic extracted from orchestrator
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 3: Extract Workflow Execution Engine

**Goal**: Separate the core workflow execution logic into a focused execution engine module.

**Changes**:
- Create new module `src/cook/orchestrator/executor.rs`
- Extract execution methods:
  - `execute_workflow()` - main dispatch logic
  - `execute_standard_workflow_from()`
  - `execute_iterative_workflow_from()`
  - `execute_structured_workflow_from()`
  - `execute_step()`
  - `execute_workflow_command()`
  - `execute_and_validate_command()`
- Create `WorkflowExecutor` struct with focused responsibilities
- Separate I/O operations from pure logic
- Extract command preparation into pure functions:
  - `prepare_environment_variables()`
  - `collect_workflow_inputs()`
  - `extract_input_from_path()`

**Estimated Lines**: ~400 lines in new module

**Testing**:
- Run `cargo test --lib executor`
- Verify workflow execution end-to-end
- Test standard, iterative, and structured workflows separately
- Ensure environment variable interpolation works

**Success Criteria**:
- [ ] Executor module handles all workflow execution
- [ ] Pure functions separated from I/O operations
- [ ] Each workflow type has dedicated execution path
- [ ] All existing workflow tests pass
- [ ] Ready to commit

### Phase 4: Extract MapReduce Workflow Handling

**Goal**: Move MapReduce-specific logic into its own module to reduce main orchestrator complexity.

**Changes**:
- Create new module `src/cook/orchestrator/mapreduce.rs`
- Extract MapReduce execution:
  - `execute_mapreduce_workflow()`
  - MapReduce-specific setup and teardown
  - Result aggregation logic
- Create `MapReduceExecutor` struct
- Move MapReduce config handling from main orchestrator
- Extract pure functions for result processing

**Estimated Lines**: ~150 lines in new module

**Testing**:
- Run `cargo test --lib mapreduce`
- Verify MapReduce workflows execute correctly
- Test parallel execution scenarios
- Ensure result aggregation works

**Success Criteria**:
- [ ] MapReduce module handles all MapReduce workflows
- [ ] Main orchestrator delegates to MapReduce module
- [ ] MapReduce tests pass independently
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 5: Refactor Main Orchestrator as Coordinator

**Goal**: Reduce main orchestrator to a thin coordination layer that delegates to specialized modules.

**Changes**:
- Refactor `DefaultCookOrchestrator` to use extracted modules
- Keep only coordination methods:
  - `run()` - main entry point
  - `check_prerequisites()`
  - `setup_environment()`
  - `cleanup()`
  - High-level workflow dispatch
- Remove all extracted logic
- Update to delegate to:
  - `OrchestratorBuilder` for construction
  - `WorkflowClassifier` for classification
  - `WorkflowExecutor` for standard execution
  - `MapReduceExecutor` for MapReduce execution
- Update module structure in `src/cook/orchestrator/mod.rs`

**Estimated Lines**: Main orchestrator reduced to ~200-300 lines

**Testing**:
- Run full test suite: `cargo test`
- Run integration tests
- Verify all workflow types still work
- Test error handling and cleanup

**Success Criteria**:
- [ ] Main orchestrator is <300 lines
- [ ] Delegates to specialized modules
- [ ] All 82 original functions accounted for
- [ ] All tests pass
- [ ] Clippy clean
- [ ] Ready to commit

## Implementation Phases Summary

| Phase | Module | Lines | Functions | Purpose |
|-------|--------|-------|-----------|---------|
| 1 | `builder.rs` | ~150 | 7 | Construction & config |
| 2 | `workflow_classifier.rs` | ~200 | 8 | Pure classification logic |
| 3 | `executor.rs` | ~400 | 12 | Core execution engine |
| 4 | `mapreduce.rs` | ~150 | 4 | MapReduce workflows |
| 5 | `orchestrator.rs` (refactored) | ~250 | 4 | Thin coordinator |
| - | Tests | ~800 | 40+ | Moved test functions |

**Total**: ~1950 lines across focused modules vs 3136 in monolithic file (38% reduction)

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo build` to ensure compilation
4. Add new unit tests for extracted modules
5. Commit working code before next phase

**Coverage improvement plan**:
1. Phase 2: Add 10+ tests for workflow classification (pure functions)
2. Phase 3: Add 15+ tests for execution logic
3. Phase 4: Add 5+ tests for MapReduce handling
4. Final: Aim for >40% coverage (up from 21.95%)

**Final verification**:
1. `cargo test` - All tests pass
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Formatting correct
4. `just ci` - Full CI checks pass
5. Integration test with sample workflows

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the compilation/test errors
3. Check for missed dependencies or incorrect extraction
4. Adjust extraction approach (smaller chunks if needed)
5. Retry with corrected approach

## Notes

### Key Challenges

**Dependency Management**: The orchestrator has 7 injected dependencies. All extracted modules will need appropriate subset of these dependencies. Consider:
- Builder only needs construction parameters
- Executor needs session_manager, command_executor, claude_executor, git_operations
- Classifier needs no dependencies (pure functions)

**Backward Compatibility**: The `new()` constructor must remain unchanged to avoid breaking existing code. Use wrapper pattern in Phase 1.

**Test Organization**: 40+ test functions currently in orchestrator.rs. Move tests to corresponding new modules as logic is extracted.

**Async Boundaries**: Many methods are async. Maintain async/await boundaries correctly when extracting to new modules.

### Functional Programming Approach

Each extracted module should favor:
- **Pure functions** for business logic (classification, normalization)
- **I/O at boundaries** (executor handles I/O, delegates to pure logic)
- **Single responsibility** (each module does one thing well)
- **Composition** (orchestrator composes specialized modules)

### Success Metrics

After all phases:
- Main orchestrator: <300 lines (from 3136)
- Average module size: ~250 lines (manageable)
- Functions per module: <15 (focused)
- Coverage: >40% (from 21.95%)
- Cyclomatic complexity: Reduced by >50%
- God object score: From 1.0 to <0.3

This plan transforms a 3136-line god object into a clean, modular architecture following functional programming principles with proper separation of concerns.
