# Implementation Plan: Extract Pure Functions from execute_structured_workflow

## Problem Summary

**Location**: ./src/cook/orchestrator/execution_pipeline.rs:ExecutionPipeline::execute_structured_workflow:597
**Priority Score**: 24.14
**Debt Type**: ComplexityHotspot (Cyclomatic: 18, Cognitive: 64)
**Current Metrics**:
- Function Length: 152 lines
- Cyclomatic Complexity: 18
- Cognitive Complexity: 64
- Nesting Depth: 5 levels
- Purity Confidence: 0.90 (high potential for pure function extraction)

**Issue**: High complexity (18/64) makes function hard to test and maintain. The function mixes orchestration logic with pure business logic for variable resolution, output tracking, and command preparation.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 9.0 (target cyclomatic complexity: ~10)
- Coverage Improvement: 0.0 (focus on refactoring, not new coverage)
- Risk Reduction: 6.3245

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 18 to ≤10
- [ ] Cognitive complexity reduced from 64 to ≤30
- [ ] Extract 4-5 pure, testable functions
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with rustfmt

## Implementation Phases

### Phase 1: Extract Variable Resolution Logic

**Goal**: Extract pure function for resolving variables from command outputs

**Changes**:
- Create new pure function `build_variable_map(command_outputs: &HashMap<String, HashMap<String, String>>) -> HashMap<String, String>`
- Extract lines 647-655 (variable resolution from command outputs) into this function
- Add unit tests for the new function with various input scenarios
- Replace inline logic with function call

**Testing**:
- Unit test: empty command_outputs → empty variable map
- Unit test: single command with single output → correct variable name format
- Unit test: multiple commands with multiple outputs → all variables present
- Run `cargo test --lib` to verify existing tests pass

**Success Criteria**:
- [ ] Pure function `build_variable_map` created and tested
- [ ] Function has at least 3 unit tests covering edge cases
- [ ] Cyclomatic complexity reduced by ~2 points
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Command Preparation Logic

**Goal**: Extract pure function for building command string from parts

**Changes**:
- Create new pure function `build_command_string(command_name: &str, args: &[crate::cook::workflow::CommandArg], variables: &HashMap<String, String>) -> String`
- Extract lines 662-669 (command string building) into this function
- Add unit tests for command string construction
- Replace inline logic with function call

**Testing**:
- Unit test: command with no args → just command name
- Unit test: command with static args → correct concatenation
- Unit test: command with variable references → properly resolved
- Unit test: empty resolved args are skipped
- Run `cargo test --lib` to verify existing tests pass

**Success Criteria**:
- [ ] Pure function `build_command_string` created and tested
- [ ] Function has at least 4 unit tests covering edge cases
- [ ] Cyclomatic complexity reduced by ~2 points
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Output Processing Logic

**Goal**: Extract pure logic for processing command outputs

**Changes**:
- Create new function `process_command_outputs` that encapsulates output file pattern matching logic
- Extract the output handling block (lines 698-736) into a more focused function
- Separate the pure logic (output mapping) from I/O operations (file finding)
- Add tests for output processing logic

**Testing**:
- Unit test: command with no outputs → no processing
- Unit test: command with outputs but no ID → outputs not stored
- Unit test: command with outputs and ID → outputs correctly mapped
- Run `cargo test --lib` to verify existing tests pass

**Success Criteria**:
- [ ] Output processing function created and tested
- [ ] Function has at least 3 unit tests
- [ ] Cyclomatic complexity reduced by ~2 points
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Step Description Builder

**Goal**: Extract pure function for building step descriptions

**Changes**:
- Create new pure function `build_step_description(command: &crate::cook::workflow::Command) -> String`
- Extract lines 627-637 (step description building) into this function
- Add unit tests for step description formatting
- Replace inline logic with function call

**Testing**:
- Unit test: command with no args → just command name
- Unit test: command with args → formatted description
- Unit test: command with empty args → empty args filtered out
- Run `cargo test --lib` to verify existing tests pass

**Success Criteria**:
- [ ] Pure function `build_step_description` created and tested
- [ ] Function has at least 3 unit tests
- [ ] Cyclomatic complexity reduced by ~1 point
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Verify Final Complexity and Document

**Goal**: Verify complexity targets met and ensure code quality

**Changes**:
- Run `cargo clippy` to check for any new warnings
- Run `rustfmt` to ensure consistent formatting
- Add module-level documentation for new pure functions
- Verify cyclomatic complexity is ≤10 using `debtmap analyze`

**Testing**:
- Run `just ci` - Full CI checks
- Run `cargo test --all` - All tests pass
- Run `cargo clippy -- -D warnings` - No warnings
- Run `debtmap analyze` - Verify complexity reduction

**Success Criteria**:
- [ ] Cyclomatic complexity ≤10 (target met)
- [ ] Cognitive complexity ≤30 (target met)
- [ ] All CI checks pass
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Write unit tests for extracted functions FIRST (test-first approach)
2. Extract the function and make tests pass
3. Run `cargo test --lib` to verify no regressions
4. Run `cargo clippy` to check for new warnings
5. Commit the working phase

**Final verification**:
1. `just ci` - Full CI checks including all tests
2. `cargo clippy -- -D warnings` - Zero tolerance for warnings
3. `debtmap analyze` - Verify improvement in metrics
4. Compare before/after complexity scores

**Test Coverage Focus**:
Since the function has high purity confidence (0.90), the extracted pure functions should be easily testable without mocks or complex setup. Focus on:
- Edge cases (empty inputs, single item, multiple items)
- Different input combinations
- Boundary conditions

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures or compilation errors
3. Adjust the extraction strategy (smaller function, different boundaries)
4. Retry with updated approach

If tests fail after extraction:
1. Check that function signature matches usage
2. Verify all edge cases are handled
3. Ensure no implicit state was being used
4. Add debugging output to identify mismatch

## Notes

**Why This Refactoring Works**:
- High purity confidence (0.90) indicates logic is already mostly pure
- Clear separation between orchestration (loops, async calls) and business logic (variable resolution, string building)
- Extracted functions will be independently testable without ExecutionPipeline instance
- Each phase reduces complexity incrementally while maintaining working code

**Function Extraction Strategy**:
- Target pure, stateless logic first (variable resolution, string building)
- Keep I/O and orchestration in the main function
- New functions should take primitive types and collections as parameters
- Avoid passing ExecutionPipeline or complex state

**Expected Complexity Reduction Path**:
- Starting: Cyclomatic 18, Cognitive 64
- After Phase 1: Cyclomatic ~16, Cognitive ~55
- After Phase 2: Cyclomatic ~14, Cognitive ~47
- After Phase 3: Cyclomatic ~12, Cognitive ~39
- After Phase 4: Cyclomatic ~11, Cognitive ~34
- After Phase 5: Cyclomatic ≤10, Cognitive ≤30 (target achieved)

**Risks and Mitigations**:
- Risk: Breaking existing tests during extraction
  - Mitigation: Test after each small extraction, commit frequently
- Risk: Over-abstracting and making code harder to follow
  - Mitigation: Keep extracted functions simple, single-purpose, well-named
- Risk: Missing implicit dependencies or state
  - Mitigation: High purity confidence suggests this is low risk; validate with tests
