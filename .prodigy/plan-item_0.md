# Implementation Plan: Split God Object Core Orchestrator

## Problem Summary

**Location**: ./src/cook/orchestrator/core.rs:DefaultCookOrchestrator:105
**Priority Score**: 148.67
**Debt Type**: God Object
**Current Metrics**:
- Lines of Code: 3176
- Functions: 86
- Cyclomatic Complexity: 331 (avg 3.85 per function, max 29)
- Coverage: 20.9%
- Uncovered Lines: 2511

**Issue**: The `DefaultCookOrchestrator` is a massive god object with 61 methods, 7 fields, and a god object score of 1.0. It handles construction, workflow execution, environment management, session management, file operations, validation, and computation. The file contains 3176 lines with mixed responsibilities making it difficult to test, maintain, and understand.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 66.2 points
- Maintainability Improvement: 14.87 points
- Test Effort: 251.1 points

**Success Criteria**:
- [ ] Split into 3-4 focused modules (construction, execution, validation, utilities)
- [ ] Each module has <30 functions and <800 lines
- [ ] Extract pure functions for easier unit testing
- [ ] Separate I/O operations from business logic
- [ ] Coverage increases from 20.9% to >40% through testable pure functions
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Extract Construction Module

**Goal**: Move construction-related methods to a dedicated `construction.rs` module to handle object creation and configuration.

**Changes**:
- Create `src/cook/orchestrator/construction.rs`
- Extract construction functions:
  - `new()` - Main constructor
  - `from_builder()` - Builder constructor
  - `create_env_config()` - Environment configuration creation
  - `create_workflow_executor_internal()` - Workflow executor creation
  - `create_workflow_state_base_internal()` - State base creation
  - `with_test_config()` - Test configuration setter
  - `generate_session_id()` - Session ID generation
- Move these as pure functions or methods on a `OrchestratorConstruction` struct
- Update `core.rs` to use the construction module
- Update `mod.rs` to expose construction types

**Testing**:
- Run `cargo test --lib` to verify construction tests pass
- Verify builder pattern still works correctly
- Test session ID generation is deterministic

**Success Criteria**:
- [ ] Construction module created with ~120 lines
- [ ] All construction logic extracted from core.rs
- [ ] Core.rs reduced by ~200 lines
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 2: Extract Workflow Classification and Normalization

**Goal**: Extract workflow type classification and normalization logic into a dedicated module for pure business logic.

**Changes**:
- Create `src/cook/orchestrator/normalization.rs`
- Extract pure classification functions:
  - `classify_workflow_type()` - Determine workflow type
  - `classify_workflow_type_old()` - Legacy classifier
  - `normalize_workflow()` - Normalize workflow config
  - `determine_commit_required()` - Check commit requirement
  - `determine_commit_required_old()` - Legacy commit check
  - `convert_command_to_step()` - Convert commands to steps
  - `process_step_failure_config()` - Process failure config
- Make these pure functions that take config as input and return results
- Add comprehensive unit tests for each pure function
- Update `core.rs` to delegate to normalization module

**Testing**:
- Run `cargo test --lib`
- Add new unit tests for each extracted function
- Test workflow classification with various inputs
- Test normalization edge cases

**Success Criteria**:
- [ ] Normalization module created with ~300 lines
- [ ] 7 pure functions extracted and tested
- [ ] Test coverage >80% for normalization module
- [ ] Core.rs reduced by ~400 lines
- [ ] All tests pass including new tests
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 3: Extract File and Pattern Matching Utilities

**Goal**: Extract file operations and pattern matching into a pure utility module.

**Changes**:
- Create `src/cook/orchestrator/file_utils.rs`
- Extract pure file utility functions:
  - `find_files_matching_pattern()` - File search (make pure by returning paths)
  - `matches_glob_pattern()` - Glob pattern matching
  - `matches_glob_pattern_old()` - Legacy pattern matching
  - `collect_workflow_inputs()` - Collect inputs (make pure)
  - `process_glob_pattern()` - Process glob patterns (make pure)
  - `extract_input_from_path()` - Extract input from path
- Refactor to separate I/O (file reading) from logic (pattern matching)
- Pure functions for pattern matching
- Separate functions for I/O operations
- Add comprehensive unit tests

**Testing**:
- Run `cargo test --lib`
- Add unit tests for glob pattern matching
- Add tests for path extraction
- Test with various file patterns

**Success Criteria**:
- [ ] File utilities module created with ~250 lines
- [ ] Pure pattern matching functions with >70% test coverage
- [ ] I/O separated from pattern logic
- [ ] Core.rs reduced by ~300 lines
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 4: Extract Project Analysis Functions

**Goal**: Extract project health scoring and analysis into a focused module.

**Changes**:
- Create `src/cook/orchestrator/analysis.rs`
- Extract analysis functions:
  - `display_health_score()` - Display project health
  - `get_test_coverage()` - Extract coverage data
  - `get_lint_warnings()` - Extract lint warnings
  - `get_code_duplication()` - Extract duplication metrics
- Separate pure calculation logic from I/O
- Make metrics extraction pure by returning Result<Metrics>
- Display logic stays as thin wrapper
- Add unit tests for pure functions

**Testing**:
- Run `cargo test --lib`
- Add unit tests for metrics calculation
- Mock file I/O for testing
- Test health score calculation

**Success Criteria**:
- [ ] Analysis module created with ~150 lines
- [ ] Pure metrics functions tested
- [ ] I/O separated from calculation logic
- [ ] Core.rs reduced by ~200 lines
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Extract Resume and Session Management

**Goal**: Extract workflow resume and session restoration logic into a dedicated module.

**Changes**:
- Create `src/cook/orchestrator/resume.rs`
- Extract session/resume functions:
  - `resume_workflow()` - Resume workflow execution
  - `restore_environment()` - Restore execution environment
  - `resume_workflow_execution()` - Resume from checkpoint
  - `execute_standard_workflow_from()` - Execute from iteration
  - `execute_iterative_workflow_from()` - Execute iterative from checkpoint
  - `execute_structured_workflow_from()` - Execute structured from checkpoint
  - `get_current_head()` - Get current git head
- Separate state restoration (pure) from execution (I/O)
- Add tests for resume logic

**Testing**:
- Run `cargo test --lib`
- Test resume with various session states
- Test environment restoration
- Test checkpoint recovery

**Success Criteria**:
- [ ] Resume module created with ~400 lines
- [ ] Session restoration logic extracted
- [ ] Resume functions tested
- [ ] Core.rs reduced by ~500 lines
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 6: Refactor Core Execution Methods

**Goal**: With helper modules extracted, refactor remaining core execution methods to be leaner and delegate to specialized modules.

**Changes**:
- Refactor `execute_workflow()` to delegate to specialized modules
- Refactor `execute_unified()` to use normalization module
- Refactor `execute_mapreduce_workflow()` to use file_utils
- Refactor `execute_workflow_with_args()` to use resume module
- Ensure core methods are thin orchestration layers
- Update documentation to reference new modules
- Core.rs should now be <1500 lines

**Testing**:
- Run `cargo test --lib`
- Run `cargo test --all` for integration tests
- Test all workflow types (standard, structured, iterative, mapreduce)
- Verify end-to-end workflows still work

**Success Criteria**:
- [ ] Core.rs reduced to <1500 lines (from 3176)
- [ ] Core methods delegate to specialized modules
- [ ] All workflow types tested and working
- [ ] Integration tests pass
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 7: Final Cleanup and Documentation

**Goal**: Clean up remaining code, update documentation, and verify all quality metrics.

**Changes**:
- Remove any duplicate or dead code
- Update module documentation
- Ensure all public APIs are documented
- Update `mod.rs` to properly expose new modules
- Run full CI suite
- Verify coverage improvements
- Generate final debtmap to confirm reduction

**Testing**:
- Run `just ci` - Full CI checks
- Run `cargo tarpaulin` - Verify coverage increase
- Run `debtmap analyze` - Confirm debt reduction
- Manual testing of key workflows

**Success Criteria**:
- [ ] All modules properly documented
- [ ] Coverage increased from 20.9% to >40%
- [ ] God object score reduced from 1.0 to <0.5
- [ ] Cyclomatic complexity reduced by >50 points
- [ ] All CI checks pass
- [ ] Ready for final commit and PR

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify unit tests pass
2. Run `cargo clippy -- -D warnings` to check for issues
3. Run `cargo fmt -- --check` to verify formatting
4. Manually test affected functionality if needed
5. Commit only when phase is complete and working

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --out Xml` - Generate coverage report
3. `debtmap analyze --output final-debt.json` - Verify improvement
4. Compare metrics: before (3176 lines, 86 functions) vs after (<2000 lines distributed across modules)

## Rollback Plan

If a phase fails:
1. Review the error carefully
2. `git diff` to see what changed
3. `git reset --hard HEAD` to revert uncommitted changes
4. Or `git revert HEAD` if already committed
5. Analyze the failure and adjust the plan
6. Retry the phase with fixes

For integration issues:
1. Revert to last known good commit
2. Review module boundaries
3. Check for missed dependencies
4. Fix and retry

## Notes

### Key Architectural Principles

1. **Separation of Concerns**: Each module handles one responsibility
2. **Pure Functions**: Extract pure logic from I/O operations
3. **Testability**: Pure functions are easy to unit test
4. **Incremental**: Each phase is independently valuable
5. **Backward Compatible**: Existing tests must continue to pass

### Module Structure After Refactoring

```
src/cook/orchestrator/
├── mod.rs                    # Module exports
├── core.rs                   # <1500 lines - main orchestration
├── builder.rs                # Existing builder
├── workflow_classifier.rs    # Existing classifier
├── construction.rs           # NEW - Object construction (~120 lines)
├── normalization.rs          # NEW - Workflow classification/normalization (~300 lines)
├── file_utils.rs             # NEW - File operations and patterns (~250 lines)
├── analysis.rs               # NEW - Health scoring and metrics (~150 lines)
└── resume.rs                 # NEW - Session resume logic (~400 lines)
```

### Potential Issues and Mitigations

**Issue**: Tests may be tightly coupled to core.rs structure
- **Mitigation**: Update tests incrementally, preserve test behavior

**Issue**: Circular dependencies between new modules
- **Mitigation**: Ensure dependency flow is one-way (core → modules)

**Issue**: Breaking changes to public API
- **Mitigation**: Keep public API unchanged, only refactor internals

**Issue**: Performance regression from extra indirection
- **Mitigation**: Modules are in same compilation unit, optimizer will inline

### Debtmap Alignment

This plan directly addresses the debtmap recommendations:
- **God Object Split**: Creating 5 new focused modules from 1 massive file
- **Responsibility Separation**: Each module has clear, single responsibility
- **Complexity Reduction**: Pure functions reduce cyclomatic complexity
- **Test Coverage**: Extracting pure functions enables unit testing
- **Maintainability**: Smaller, focused modules are easier to understand and modify

The recommended splits from debtmap suggested:
1. Construction (120 lines) - ✓ Phase 1
2. Core Operations (880 lines) - ✓ Phases 2-6 (distributed across normalization, file_utils, resume)

We're further breaking down "Core Operations" into:
- Normalization (~300 lines) - Pure workflow logic
- File Utils (~250 lines) - File operations
- Analysis (~150 lines) - Health scoring
- Resume (~400 lines) - Session management

This results in better separation of concerns than the debtmap suggestion.
