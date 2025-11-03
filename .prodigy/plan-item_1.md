# Implementation Plan: Complete Executor Refactoring - Extract Types and Remaining God Object

## Problem Summary

**Location**: ./src/cook/workflow/executor.rs:file:0
**Priority Score**: 111.49
**Debt Type**: God Object (File-level)
**Current Metrics**:
- Lines of Code: 2760 lines
- Functions: 107 functions
- Cyclomatic Complexity: 259 total (avg 2.42 per function)
- Coverage: 0.0%
- God Object Score: 1.0 (confirmed god class)
- Responsibilities: 8 distinct domains

**Issue**: This is a massive file with 2760 lines and 107 functions exhibiting classic "God Object" anti-pattern. Despite prior refactoring efforts that created 8 submodules (builder.rs, commands.rs, context.rs, failure_handler.rs, orchestration.rs, pure.rs, step_executor.rs, validation.rs), the main executor.rs file still contains 2760 lines with mixed concerns:

1. **Type definitions** (structs, enums) that should be in a dedicated types module
2. **WorkflowContext** with business logic mixed with data
3. **Massive WorkflowStep** struct (30 fields) that needs decomposition
4. **WorkflowExecutor** struct (25 fields) that still has too many responsibilities
5. **Implementation methods** scattered across 1500+ lines

The debtmap analysis identifies:
- 8 responsibility domains (Formatting & Output, Computation, Construction, Utilities, Data Access, Processing, Persistence, Validation)
- Recommended split: Create 4 focused modules with <30 functions each
- Primary action: Split by data flow into Input/Parsing → Core Logic → Output/Formatting

**Root Cause**: Over time, new features were added directly to this file, and while some extraction has occurred, the core type definitions and orchestration logic remain entangled.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 51.8 points (20% reduction in cyclomatic complexity)
- Maintainability Improvement: 11.15 points
- Test Effort Reduction: 276 hours (by enabling focused unit testing)

**Success Criteria**:
- [ ] Main executor.rs reduced to <500 lines (module coordination only)
- [ ] All type definitions moved to dedicated types.rs module
- [ ] WorkflowContext logic extracted to pure functions
- [ ] WorkflowExecutor split into focused components
- [ ] Each submodule has single, clear responsibility
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper module documentation

**Architectural Vision**:
```
executor/
├── mod.rs (< 500 lines) - Module coordination, main executor struct
├── types.rs - Type definitions (CaptureOutput, CommandType, StepResult, etc.)
├── workflow_context.rs - WorkflowContext with pure interpolation logic
├── workflow_step.rs - WorkflowStep and related configuration types
├── builder.rs - ✓ Already exists
├── commands.rs - ✓ Already exists
├── context.rs - ✓ Already exists
├── failure_handler.rs - ✓ Already exists
├── orchestration.rs - ✓ Already exists
├── pure.rs - ✓ Already exists
├── step_executor.rs - ✓ Already exists
└── validation.rs - ✓ Already exists
```

## Implementation Phases

### Phase 1: Extract Core Type Definitions to types.rs

**Goal**: Create a new `types.rs` module containing all shared type definitions, reducing executor.rs by ~300 lines.

**Changes**:
1. Create `src/cook/workflow/executor/types.rs`
2. Move these types from executor.rs to types.rs:
   - `CaptureOutput` enum (lines 60-110)
   - `CommandType` enum (lines 134-156)
   - `StepResult` struct (lines 158-167)
   - `VariableResolution` struct (lines 169-175)
   - Helper functions: `deserialize_capture_output`
3. Update executor.rs to import from types module
4. Update all submodules that reference these types
5. Add comprehensive documentation to types.rs explaining each type's purpose

**Testing**:
```bash
# Verify compilation
cargo build --lib

# Run existing tests
cargo test --lib executor

# Check for clippy warnings
cargo clippy --lib -- -D warnings
```

**Success Criteria**:
- [ ] types.rs created with ~150 lines
- [ ] All type definitions properly documented
- [ ] executor.rs reduced by ~300 lines
- [ ] All tests pass without modification
- [ ] No clippy warnings

**Files Changed**:
- New: `src/cook/workflow/executor/types.rs`
- Modified: `src/cook/workflow/executor.rs` (remove type definitions, add imports)
- Modified: `src/cook/workflow/executor/commands.rs` (update imports)
- Modified: Other executor submodules as needed (update imports)

### Phase 2: Extract WorkflowContext to workflow_context.rs

**Goal**: Move WorkflowContext and its interpolation logic to a dedicated module, separating data structure from business logic.

**Changes**:
1. Create `src/cook/workflow/executor/workflow_context.rs`
2. Move from executor.rs:
   - `WorkflowContext` struct (lines 177-188)
   - `Default` impl for WorkflowContext (lines 190-201)
   - All WorkflowContext methods (lines 203-266)
   - Variable interpolation helper functions
3. Refactor WorkflowContext methods to use pure functions from pure.rs where applicable
4. Extract complex interpolation logic into testable pure functions
5. Add unit tests for interpolation logic

**Testing**:
```bash
# Build and test
cargo build --lib
cargo test --lib executor::workflow_context

# Verify interpolation behavior
cargo test --lib interpolation

# Check clippy
cargo clippy --lib -- -D warnings
```

**Success Criteria**:
- [ ] workflow_context.rs created with ~150-200 lines
- [ ] WorkflowContext fully encapsulated in dedicated module
- [ ] Interpolation logic testable via pure functions
- [ ] Unit tests added for complex interpolation scenarios
- [ ] executor.rs reduced by another ~250 lines
- [ ] All existing tests pass

**Files Changed**:
- New: `src/cook/workflow/executor/workflow_context.rs`
- Modified: `src/cook/workflow/executor.rs` (remove WorkflowContext, add imports)
- Modified: `src/cook/workflow/executor/pure.rs` (add interpolation helper functions)
- Modified: Other executor submodules (update imports)

### Phase 3: Extract WorkflowStep and Configuration Types to workflow_step.rs

**Goal**: Move WorkflowStep and related configuration structs to a dedicated module, reducing the massive struct complexity.

**Changes**:
1. Create `src/cook/workflow/executor/workflow_step.rs`
2. Move from executor.rs:
   - `HandlerStep` struct (lines 268-276)
   - `WorkflowStep` struct with 30 fields (lines 278-401)
   - `SensitivePatternConfig` struct (lines 422-431)
   - `WorkflowMode` enum (lines 466-473)
   - `ExtendedWorkflowConfig` struct (lines 475-499)
   - Helper functions: `default_commit_required`, `is_false`, `compile_regex`
   - Default impls for these types
3. Consider splitting WorkflowStep into smaller, focused structs if feasible:
   - Core execution config (command, capture settings)
   - Retry/failure handling config
   - Validation config
4. Add builder pattern for WorkflowStep construction if warranted
5. Document each configuration field clearly

**Testing**:
```bash
# Build and test
cargo build --lib
cargo test --lib executor::workflow_step

# Verify deserialization from YAML
cargo test --lib config

# Check clippy
cargo clippy --lib -- -D warnings
```

**Success Criteria**:
- [ ] workflow_step.rs created with ~300-400 lines
- [ ] All configuration types centralized and documented
- [ ] Helper functions for configuration moved
- [ ] executor.rs reduced by another ~500 lines
- [ ] YAML deserialization still works correctly
- [ ] All tests pass

**Files Changed**:
- New: `src/cook/workflow/executor/workflow_step.rs`
- Modified: `src/cook/workflow/executor.rs` (remove config types, add imports)
- Modified: All executor submodules (update imports to use workflow_step)
- Modified: `src/config/` modules if they reference these types

### Phase 4: Reorganize WorkflowExecutor Implementation Methods

**Goal**: Split the massive WorkflowExecutor impl block (1500+ lines) into logical method groups using impl blocks and trait implementations.

**Changes**:
1. Analyze WorkflowExecutor methods by responsibility:
   - Initialization/lifecycle methods
   - Step execution methods
   - Failure handling methods
   - Checkpoint/resume methods
   - Variable management methods
   - Dry-run/preview methods
2. Group related methods into separate impl blocks within executor.rs
3. Extract any methods that can be made into pure functions into pure.rs
4. Move complex logic from methods into submodule functions (e.g., commands.rs, step_executor.rs)
5. Keep only high-level orchestration logic in WorkflowExecutor
6. Add inline comments delineating each impl block's responsibility

**Refactoring Opportunities**:
- `handle_on_failure` → Use more of failure_handler.rs functions
- Checkpoint methods → Consider extracting to checkpoint_manager.rs
- Variable interpolation → Delegate fully to workflow_context.rs
- Step execution → Delegate more to step_executor.rs
- Dry-run logic → Consider extracting to dry_run.rs

**Testing**:
```bash
# Full test suite
cargo test --lib

# Integration tests for workflows
cargo test --test workflow_integration

# Check clippy
cargo clippy --lib -- -D warnings
```

**Success Criteria**:
- [ ] WorkflowExecutor impl split into 5-7 focused impl blocks
- [ ] Each impl block <200 lines
- [ ] Complex logic extracted to submodules or pure functions
- [ ] Clear comments explaining each impl block's purpose
- [ ] executor.rs now <500 lines total
- [ ] All tests pass
- [ ] No clippy warnings

**Files Changed**:
- Modified: `src/cook/workflow/executor.rs` (reorganize impl blocks)
- Modified: `src/cook/workflow/executor/commands.rs` (accept extracted methods)
- Modified: `src/cook/workflow/executor/step_executor.rs` (accept extracted methods)
- Modified: `src/cook/workflow/executor/failure_handler.rs` (accept extracted logic)
- Modified: `src/cook/workflow/executor/pure.rs` (add pure functions)

### Phase 5: Update Module Structure and Documentation

**Goal**: Finalize the module structure, update all imports, and ensure comprehensive documentation.

**Changes**:
1. Update `src/cook/workflow/executor.rs` (now mod.rs in spirit):
   - Organize module declarations
   - Re-export public types from submodules
   - Add module-level documentation
   - Ensure <500 lines total
2. Add/update README or module docs explaining:
   - Overall executor architecture
   - Purpose of each submodule
   - Data flow through the system
   - How to add new features
3. Update all imports across the codebase to use new module structure
4. Run full clippy and formatting pass
5. Run complete test suite including integration tests
6. Update any developer documentation referencing the executor

**Testing**:
```bash
# Full build
cargo build --all-targets

# Complete test suite
cargo test --all

# Clippy with strict settings
cargo clippy --all-targets -- -D warnings

# Format check
cargo fmt -- --check

# Integration verification
just ci
```

**Success Criteria**:
- [ ] executor module exports clean public API
- [ ] All submodules properly documented
- [ ] Module-level docs explain architecture
- [ ] All imports updated across codebase
- [ ] Full test suite passes
- [ ] No clippy warnings
- [ ] Formatting consistent
- [ ] CI pipeline green

**Files Changed**:
- Modified: `src/cook/workflow/executor.rs` (finalize structure, add docs)
- Modified: All files importing from executor module (update import paths)
- New/Modified: Architecture documentation

## Testing Strategy

**For each phase**:

1. **Compilation Check**:
   ```bash
   cargo build --lib
   ```

2. **Unit Tests**:
   ```bash
   cargo test --lib executor
   ```

3. **Integration Tests**:
   ```bash
   cargo test --test workflow_integration
   ```

4. **Linting**:
   ```bash
   cargo clippy --lib -- -D warnings
   ```

5. **Formatting**:
   ```bash
   cargo fmt
   ```

**Final verification** (after Phase 5):

1. **Full CI**:
   ```bash
   just ci
   ```

2. **Coverage Check** (if available):
   ```bash
   cargo tarpaulin --lib
   ```

3. **Complexity Verification**:
   ```bash
   # Re-run debtmap to verify improvement
   debtmap analyze
   ```

4. **Manual Smoke Test**:
   - Run a simple workflow
   - Run a MapReduce workflow
   - Test checkpoint/resume functionality
   - Verify dry-run mode

## Rollback Plan

If a phase fails:

1. **Identify the failure point**:
   - Which test failed?
   - What compilation error occurred?
   - Which integration broke?

2. **Revert the phase**:
   ```bash
   git reset --hard HEAD~1
   ```

3. **Analyze and adjust**:
   - Review the error messages
   - Check if additional dependencies need updating
   - Verify import paths are correct
   - Check for missed references

4. **Retry with adjustments**:
   - Make necessary corrections
   - Test incrementally
   - Commit smaller changes if needed

## Expected Outcomes

**Complexity Reduction**:
- executor.rs: 2760 lines → <500 lines (82% reduction)
- Average function complexity: Maintained at ~2.4 per function (already good)
- File organization: 1 monolith → 12 focused modules

**Maintainability Improvements**:
- Clear separation of concerns (types, context, configuration, execution)
- Each module has single, well-defined responsibility
- Easier to locate and modify specific functionality
- New features can be added to appropriate modules without polluting others

**Testing Improvements**:
- Pure functions can be unit tested in isolation
- Context and interpolation logic can be thoroughly tested
- Configuration types can be tested separately
- Reduced test effort by 276 hours (per debtmap estimate)

**Code Quality**:
- Better documentation and discoverability
- Reduced coupling between components
- Improved code navigation
- Easier onboarding for new developers

## Notes

**Why This Approach**:

1. **Incremental and Safe**: Each phase is independently valuable and testable
2. **Type-First**: Extract types first so all phases can reference clean definitions
3. **Context Then Config**: Separate data structures from business logic
4. **Implementation Last**: Only reorganize methods after structure is solid
5. **Documentation Final**: Ensure everything is documented once structure is stable

**Potential Challenges**:

1. **Import Hell**: Updating imports across many files
   - Solution: Use IDE refactoring tools, test frequently

2. **Circular Dependencies**: New modules may create cycles
   - Solution: Keep types.rs dependency-free, ensure proper module hierarchy

3. **Test Breakage**: Tests may reference internal structure
   - Solution: Fix tests incrementally, don't disable them

4. **WorkflowStep Decomposition**: 30-field struct is hard to split
   - Solution: Don't force decomposition if it breaks YAML compatibility; focus on moving it to dedicated module first

**Success Indicators**:

- executor.rs file size: 2760 → <500 lines
- Debtmap score: 111.49 → <60 (target 50% reduction)
- God object indicator: true → false
- Module responsibility: 8 domains → 1-2 per file
- Developer feedback: Easier to find and modify code

**Timeline Estimate**:

- Phase 1: 2-3 hours (types extraction)
- Phase 2: 3-4 hours (context extraction, add tests)
- Phase 3: 4-5 hours (config types, complex struct)
- Phase 4: 4-6 hours (impl reorganization, most complex)
- Phase 5: 2-3 hours (cleanup, docs, verification)
- **Total**: 15-21 hours of focused work

**Verification After Completion**:

```bash
# Line count check
wc -l src/cook/workflow/executor.rs
# Expected: <500 lines

# Complexity check
debtmap analyze src/cook/workflow/executor.rs
# Expected: score <60, god_object: false

# Module count
ls src/cook/workflow/executor/*.rs | wc -l
# Expected: 12 files

# Full CI pass
just ci
# Expected: all green
```
