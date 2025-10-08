# Implementation Plan: Add Tests and Refactor YamlValidator::validate_mapreduce_workflow

## Problem Summary

**Location**: ./src/cli/yaml_validator.rs:YamlValidator::validate_mapreduce_workflow:61
**Priority Score**: 31.88125
**Debt Type**: TestingGap (cognitive: 41, cyclomatic: 20, coverage: 0.0%)

**Current Metrics**:
- Lines of Code: 90
- Cyclomatic Complexity: 20
- Cognitive Complexity: 41
- Coverage: 0.0%
- Nesting Depth: 5

**Issue**: Complex business logic with 100% testing gap. Cyclomatic complexity of 20 requires at least 20 test cases for full path coverage. After extracting 8 functions, each will need only 3-5 tests. Testing before refactoring ensures no regressions.

## Target State

**Expected Impact**:
- Complexity Reduction: 6.0 (from 20 to ~14)
- Coverage Improvement: 50.0% (from 0% to 50%+)
- Risk Reduction: 13.39

**Success Criteria**:
- [ ] Coverage increases from 0% to at least 50%
- [ ] Cyclomatic complexity reduced from 20 to ≤14
- [ ] Extract 6-8 pure validation functions with complexity ≤3 each
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Comprehensive Tests for Current Implementation

**Goal**: Achieve baseline test coverage before refactoring to prevent regressions

**Changes**:
- Create `tests/yaml_validator_tests.rs` or add to existing test module
- Write tests covering all 20 branches:
  - Missing 'name' field
  - Missing 'map' section
  - Missing 'input' field in map
  - Missing 'json_path' field in map
  - Missing 'agent_template' field
  - Simplified syntax (sequence agent_template)
  - Nested 'commands' syntax (deprecated)
  - Invalid agent_template structure
  - Deprecated 'timeout_per_agent' parameter
  - Deprecated 'retry_on_failure' parameter
  - Simplified reduce syntax (sequence)
  - Nested reduce 'commands' syntax (deprecated)
  - Invalid reduce structure
  - Valid MapReduce workflow (happy path)
  - Workflow with check_simplified=false (skip syntax checks)

**Testing**:
```bash
cargo test yaml_validator --lib
cargo tarpaulin --lib --out Stdout | grep yaml_validator
```

**Success Criteria**:
- [ ] At least 15 test cases written covering critical branches
- [ ] Coverage for validate_mapreduce_workflow increases to 70%+
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Pure Validation Functions (Part 1 - Map Section)

**Goal**: Extract map section validation logic into focused, testable functions

**Changes**:
- Extract `validate_map_section(map: &Mapping) -> Result<Vec<String>>` - validates map fields
- Extract `validate_agent_template(template: &Value, check_simplified: bool) -> Result<Vec<String>>` - validates agent_template structure
- Extract `check_deprecated_map_params(map: &Mapping) -> Vec<String>` - checks for deprecated parameters
- Update `validate_mapreduce_workflow` to call these extracted functions
- Each function should have complexity ≤3

**Testing**:
```bash
cargo test yaml_validator --lib
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] 3 new pure functions extracted with clear single responsibilities
- [ ] Each function has complexity ≤3
- [ ] All existing tests still pass
- [ ] Add 2-3 unit tests per extracted function
- [ ] Ready to commit

### Phase 3: Extract Pure Validation Functions (Part 2 - Reduce Section)

**Goal**: Extract reduce section validation logic into focused, testable functions

**Changes**:
- Extract `validate_reduce_section(reduce: &Value, check_simplified: bool) -> Result<Vec<String>>` - validates reduce structure
- Extract `validate_simplified_syntax(value: &Value, section_name: &str) -> Option<String>` - checks for deprecated nested syntax
- Update `validate_mapreduce_workflow` to call these functions
- Each function should have complexity ≤3

**Testing**:
```bash
cargo test yaml_validator --lib
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] 2 new pure functions extracted
- [ ] Each function has complexity ≤3
- [ ] All existing tests still pass
- [ ] Add 2-3 unit tests per extracted function
- [ ] Ready to commit

### Phase 4: Extract Required Fields Validation

**Goal**: Extract top-level field validation into a pure function

**Changes**:
- Extract `validate_required_fields(workflow: &Mapping) -> Vec<String>` - validates required fields like 'name'
- Update `validate_mapreduce_workflow` to call this function
- Function should have complexity ≤2

**Testing**:
```bash
cargo test yaml_validator --lib
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] 1 new pure function extracted
- [ ] Function has complexity ≤2
- [ ] All existing tests still pass
- [ ] Add 2-3 unit tests for the extracted function
- [ ] Ready to commit

### Phase 5: Final Verification and Cleanup

**Goal**: Verify all improvements meet target metrics and clean up any remaining issues

**Changes**:
- Run full test suite and coverage analysis
- Verify cyclomatic complexity reduction
- Add any missing edge case tests
- Clean up any remaining clippy warnings
- Update documentation if needed

**Testing**:
```bash
just ci
cargo tarpaulin --lib --out Stdout
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Coverage for validate_mapreduce_workflow ≥50%
- [ ] Cyclomatic complexity reduced to ≤14
- [ ] All 6-8 extracted functions have complexity ≤3
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code is properly formatted
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo tarpaulin --lib` to verify coverage improvements
4. Verify extracted functions are truly pure (no side effects)

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Verify coverage meets 50%+ target
3. Run debtmap analysis to verify improvement (if available)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures or compilation errors
3. Adjust the extraction approach
4. Retry with smaller, more incremental changes

## Notes

**Key Insights from Code Analysis**:
- The function has 5 levels of nesting due to nested pattern matching
- Main complexity sources:
  - Pattern matching on `agent_template` (3 branches)
  - Pattern matching on `reduce` (3 branches)
  - Multiple deprecated parameter checks
  - Conditional logic based on `check_simplified` flag

**Extraction Strategy**:
- Extract validation logic that returns `Vec<String>` of issues
- Keep the function focused on orchestration
- Each extracted function validates a specific section
- Use Result<Vec<String>> for error-prone operations

**Testing Approach**:
- Start with happy path tests
- Add edge cases for each validation branch
- Test deprecated parameter detection
- Test both check_simplified=true and false modes
- Use helper functions to create test YAML structures

**Gotchas**:
- The function mutates `issues` and `suggestions` vectors
- Extracted functions should return new vectors instead
- Need to handle both `Value::Mapping` and `Value::Sequence` for agent_template/reduce
- `check_simplified` flag affects validation behavior
