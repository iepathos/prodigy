# Implementation Plan: Extract Pure Functions from EnvironmentInputProvider::generate_inputs

## Problem Summary

**Location**: ./src/cook/input/environment.rs:EnvironmentInputProvider::generate_inputs:20
**Priority Score**: 30.024090608043835
**Debt Type**: ComplexityHotspot (cognitive: 43, cyclomatic: 16)
**Current Metrics**:
- Lines of Code: 119
- Cyclomatic Complexity: 16
- Cognitive Complexity: 43
- Coverage: Not specified (likely low based on score)
- Function Role: PureLogic (but currently not pure)

**Issue**: Apply functional patterns: 4 pure functions with Iterator chains - Moderate complexity (16), needs functional decomposition

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 8.0
- Coverage Improvement: 0.0
- Risk Reduction: 10.50843171281534

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 16 to ~8
- [ ] Function decomposed into 4+ pure functions
- [ ] Duplicated filtering logic eliminated
- [ ] Iterator chains replace procedural loops
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Environment Variable Filtering Logic

**Goal**: Create a pure function for filtering environment variables based on prefix and empty value criteria

**Changes**:
- Extract `filter_env_vars` pure function that encapsulates the filtering logic
- Parameters: prefix (Option<&str>), filter_empty (bool)
- Returns: impl Iterator<Item=(String, String)>
- Eliminates duplicated filter code (lines 39-51 and 74-86)

**Testing**:
- Unit test the filter function with various prefixes
- Test empty value filtering behavior
- Verify both single_input and multi-input modes still work

**Success Criteria**:
- [ ] No duplicated filtering code
- [ ] New pure function is testable
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Variable Enrichment Logic

**Goal**: Create pure functions for adding typed variables to inputs

**Changes**:
- Extract `try_parse_as_number` function for numeric parsing
- Extract `try_parse_as_boolean` function for boolean parsing
- Extract `is_path_like_key` predicate function
- Extract `enrich_input_with_types` function that uses the above
- Move logic from lines 113-131 into these pure functions

**Testing**:
- Unit test each parsing function
- Test path detection logic
- Test enrichment with various input types

**Success Criteria**:
- [ ] Type parsing logic extracted into pure functions
- [ ] Functions are composable and testable
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Single Input Builder

**Goal**: Create a pure function for building the single consolidated input

**Changes**:
- Extract `build_single_input` function that:
  - Takes filtered environment variables
  - Creates the ExecutionInput with all vars as an object
  - Adds metadata (count, prefix)
- Move logic from lines 29-70 into this function
- Use functional patterns (fold/collect for building the HashMap)

**Testing**:
- Unit test single input creation
- Test with various environment variable sets
- Verify metadata is correctly added

**Success Criteria**:
- [ ] Single input logic is a pure function
- [ ] Uses functional patterns instead of imperative loops
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Multi Input Builder

**Goal**: Create a pure function for building individual inputs per variable

**Changes**:
- Extract `build_multi_inputs` function that:
  - Takes filtered environment variables
  - Maps each to an ExecutionInput
  - Enriches with typed values using Phase 2 functions
- Move logic from lines 72-134 into this function
- Use iterator chains (map) instead of for loops

**Testing**:
- Unit test multi input creation
- Test prefix stripping behavior
- Test type enrichment integration

**Success Criteria**:
- [ ] Multi input logic is a pure function
- [ ] Uses iterator chains instead of loops
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Refactor Main Function to Orchestrate

**Goal**: Simplify generate_inputs to just orchestrate the pure functions

**Changes**:
- Refactor `generate_inputs` to:
  - Extract config parameters
  - Call filter function
  - Branch to either single or multi builder
  - Return results
- Function should be <20 lines
- All business logic in pure functions

**Testing**:
- Integration tests for full workflow
- Verify both modes still work correctly
- Performance should be same or better

**Success Criteria**:
- [ ] Main function is simple orchestration
- [ ] All logic is in testable pure functions
- [ ] Complexity reduced to target (~8)
- [ ] All tests pass
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Add unit tests for each new pure function
4. Run `cargo fmt` to ensure formatting

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Check coverage improvement
3. Re-run debtmap to verify complexity reduction

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure and error messages
3. Adjust the extraction strategy if needed
4. Retry with smaller incremental changes

## Notes

- The duplicated filtering logic (lines 39-51 and 74-86) is the primary source of complexity
- The function mixes I/O (env::vars()) with business logic, but since env::vars() is essentially a read operation, we'll treat the filtered results as the input to our pure functions
- Focus on making each extracted function independently testable
- Use Iterator trait methods to enable lazy evaluation and better composition
- Consider using the newtype pattern for environment variable keys/values if type safety becomes an issue