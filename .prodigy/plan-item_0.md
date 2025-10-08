# Implementation Plan: Refactor God Object - CookOrchestrator Core

## Problem Summary

**Location**: ./src/cook/orchestrator/core.rs:DefaultCookOrchestrator:105
**Priority Score**: 120.14
**Debt Type**: GOD_OBJECT
**Current Metrics**:
- Lines of Code: 2864
- Functions: 77
- Cyclomatic Complexity: 297 (avg 3.86, max 29)
- Coverage: 20.78%
- Uncovered Lines: 2268

**Issue**: URGENT - God Object with 2864 lines and 77 functions across 6 distinct responsibilities. The DefaultCookOrchestrator class violates single responsibility principle with mixed concerns: construction, core operations, validation, processing, data access, and computation. This creates testing challenges, high complexity, and tight coupling.

## Target State

**Expected Impact**:
- Complexity Reduction: 59.4 points
- Maintainability Improvement: 12.01 points
- Test Effort Reduction: 226.8 points

**Success Criteria**:
- [ ] Core orchestrator reduced to <500 lines with single responsibility (coordination)
- [ ] Extract 3+ focused modules with <30 functions each
- [ ] Pure functions separated from I/O operations
- [ ] Test coverage increased to >40% for extracted modules
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Extract Session Management Operations

**Goal**: Separate session lifecycle and state management from core orchestrator

**Changes**:
- Create `src/cook/orchestrator/session_ops.rs` module
- Extract functions (~10-12 functions, ~400 lines):
  - `generate_session_id()` - Pure function
  - `calculate_workflow_hash()` - Pure function
  - `resume_workflow()` - Session operation
  - `restore_environment()` - Session operation
  - `resume_workflow_execution()` - Session operation
  - `check_prerequisites()` - Validation
  - `check_prerequisites_with_config()` - Validation
- Create `SessionOperations` struct with dependencies
- Update `DefaultCookOrchestrator` to use `SessionOperations`

**Testing**:
- Unit tests for pure functions (hash, session ID generation)
- Integration tests for resume operations
- Run `cargo test --lib orchestrator::core`

**Success Criteria**:
- [ ] SessionOperations module created with clear API
- [ ] Pure functions (hash, ID gen) have 100% test coverage
- [ ] Core orchestrator delegates to SessionOperations
- [ ] All existing session tests pass
- [ ] Ready to commit

### Phase 2: Extract Workflow Execution Logic

**Goal**: Separate workflow execution strategies from orchestration

**Changes**:
- Create `src/cook/orchestrator/workflow_executor.rs` module
- Extract functions (~15-20 functions, ~600 lines):
  - `execute_standard_workflow_from()` - Execution strategy
  - `execute_iterative_workflow_from()` - Execution strategy
  - `execute_structured_workflow_from()` - Execution strategy
  - `execute_unified()` - Unified execution
  - `execute_normalized()` - Normalized execution
  - `normalize_workflow()` - Pure transformation
  - `execute_mapreduce_workflow()` - MapReduce execution
  - `classify_workflow_type()` - Pure classification logic
  - `convert_command_to_step()` - Pure transformation
  - `determine_commit_required_old()` - Pure decision logic
- Create `WorkflowExecutor` struct with execution strategies
- Extract workflow classification into pure functions

**Testing**:
- Unit tests for pure classification/transformation functions
- Integration tests for each execution strategy
- Run `cargo test --lib orchestrator`

**Success Criteria**:
- [ ] WorkflowExecutor module with strategy pattern
- [ ] Pure functions (classify, convert) have 100% test coverage
- [ ] Each execution strategy independently testable
- [ ] Workflow type classification logic is pure and tested
- [ ] All workflow execution tests pass
- [ ] Ready to commit

### Phase 3: Extract Command Execution and Validation

**Goal**: Separate command execution and validation logic

**Changes**:
- Create `src/cook/orchestrator/command_ops.rs` module
- Extract functions (~12-15 functions, ~500 lines):
  - `execute_workflow_command()` - Command execution
  - `execute_and_validate_command()` - Validation wrapper
  - `build_command()` - Pure command building
  - `prepare_environment_variables()` - Pure env preparation
  - `execute_step()` - Step execution
  - `collect_workflow_inputs()` - Input collection
  - `extract_input_from_path()` - Pure extraction
  - `process_workflow_input()` - Input processing
  - `find_files_matching_pattern()` - File search
  - `matches_glob_pattern()` - Pure pattern matching
  - `process_glob_pattern()` - Pattern processing
- Create `CommandOperations` struct
- Separate pure functions from I/O operations

**Testing**:
- Unit tests for pure functions (build_command, prepare_env, pattern matching)
- Integration tests for command execution
- Run `cargo test --lib orchestrator::command`

**Success Criteria**:
- [ ] CommandOperations module with clear separation
- [ ] Pure functions (build, prepare, match) have 100% test coverage
- [ ] Command execution delegates to CommandOperations
- [ ] Pattern matching logic is pure and tested
- [ ] All command execution tests pass
- [ ] Ready to commit

### Phase 4: Extract Analysis and Metrics

**Goal**: Separate project analysis and metrics collection

**Changes**:
- Create `src/cook/orchestrator/analysis.rs` module
- Extract functions (~6-8 functions, ~300 lines):
  - `display_health_score()` - Display logic
  - `get_test_coverage()` - Metrics collection
  - `get_lint_warnings()` - Metrics collection
  - `get_code_duplication()` - Metrics collection
  - `run_analysis_if_needed()` - Analysis orchestration
  - `execute_workflow_with_analysis()` - Analysis wrapper
- Create `AnalysisOperations` struct
- Extract metrics collection into separate module

**Testing**:
- Unit tests for metrics parsing
- Integration tests for analysis operations
- Run `cargo test --lib orchestrator::analysis`

**Success Criteria**:
- [ ] AnalysisOperations module created
- [ ] Metrics collection separated from orchestration
- [ ] Analysis logic is testable
- [ ] All metrics tests pass
- [ ] Ready to commit

### Phase 5: Refactor Core Orchestrator to Pure Coordination

**Goal**: Reduce core orchestrator to pure coordination logic using extracted modules

**Changes**:
- Update `DefaultCookOrchestrator` to compose extracted modules:
  - `session_ops: SessionOperations`
  - `workflow_executor: WorkflowExecutor`
  - `command_ops: CommandOperations`
  - `analysis_ops: AnalysisOperations`
- Simplify `run()` method to orchestrate via delegation
- Reduce `setup_environment()` to pure coordination
- Simplify `execute_workflow()` to delegate to WorkflowExecutor
- Reduce core file to <500 lines
- Update all tests to use new structure

**Testing**:
- Integration tests for full orchestration flow
- Verify all existing tests still pass
- Run full test suite: `cargo test --lib`
- Run `cargo clippy` for warnings
- Check coverage improvement with `cargo tarpaulin --lib`

**Success Criteria**:
- [ ] Core orchestrator reduced to <500 lines
- [ ] All orchestration delegates to focused modules
- [ ] No business logic in core orchestrator (pure coordination)
- [ ] Test coverage increased to >40%
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib orchestrator` to verify module tests pass
2. Run `cargo test` to verify all tests pass
3. Run `cargo clippy` to check for warnings
4. Verify file count and line reduction

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Verify coverage improvement
3. `cargo build --release` - Verify no build errors
4. Review module structure:
   - `core.rs`: <500 lines (coordination)
   - `session_ops.rs`: ~400 lines (session management)
   - `workflow_executor.rs`: ~600 lines (execution strategies)
   - `command_ops.rs`: ~500 lines (command operations)
   - `analysis.rs`: ~300 lines (metrics/analysis)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure reason
3. Adjust the extraction strategy:
   - Too many functions at once? Split into smaller chunks
   - Dependencies unclear? Map dependencies first
   - Tests failing? Fix tests before proceeding
4. Retry with adjusted plan

## Notes

### Key Principles:
- **Separate I/O from logic**: Pure functions (hash, classify, transform) should be extracted first and tested independently
- **Extract by responsibility**: Each module should have one clear purpose
- **Maintain test coverage**: Existing tests must continue to pass after each phase
- **Incremental commits**: Each phase should result in working, committable code

### Architectural Pattern:
The refactored orchestrator will follow a **Facade pattern**:
- Core orchestrator coordinates high-level workflow
- Specialized modules handle specific concerns
- Pure functions separated from effectful operations
- Dependencies injected via constructor

### Complexity Reduction Strategy:
- Move complex conditionals into pure predicate functions
- Extract nested logic into well-named functions
- Use strategy pattern for workflow execution variants
- Separate validation from execution

### Testing Approach:
- Pure functions: Unit tests with 100% coverage target
- I/O operations: Integration tests with mocks
- Orchestration: End-to-end workflow tests
- Focus on behavior, not implementation details
