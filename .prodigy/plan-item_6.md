# Implementation Plan: Test and Refactor resolve_variable_impl

## Problem Summary

**Location**: src/cook/execution/variables.rs:VariableContext::resolve_variable_impl:491
**Priority Score**: 15.71
**Debt Type**: TestingGap (coverage: 45.45%, cyclomatic complexity: 31, cognitive complexity: 109)

**Current Metrics**:
- Function Length: 85 lines
- Cyclomatic Complexity: 31
- Cognitive Complexity: 109
- Test Coverage: 45.45%
- Nesting Depth: 6
- Uncovered Lines: 18 ranges (lines 496, 505, 509, 513, 519, 524, 527, 529, 530, 534, 539, 542, 543, 544, 547, 549, 550, 557)

**Issue**: Complex business logic with 55% coverage gap. The function handles 7 different variable resolution strategies (env, file, cmd, json with two formats, date, uuid, standard lookup) with nested error handling and caching logic. Cyclomatic complexity of 31 requires at least 31 test cases for full path coverage.

**Rationale**: This is pure logic code (PureLogic role) with high confidence (0.8) that it should be well-tested. The 55% coverage gap leaves critical error paths and edge cases untested. Testing before refactoring ensures no regressions when extracting pure functions.

## Target State

**Expected Impact**:
- Complexity Reduction: 9.3 (from 31 to ~22)
- Coverage Improvement: 27.27% (from 45% to 72%+)
- Risk Reduction: 6.60

**Success Criteria**:
- [ ] Test coverage ≥ 72% for resolve_variable_impl and related functions
- [ ] All 18 uncovered line ranges have test coverage
- [ ] Cyclomatic complexity reduced to ≤22
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Functions extracted have complexity ≤5

## Implementation Phases

### Phase 1: Add Tests for Uncovered Variable Resolution Paths

**Goal**: Achieve comprehensive test coverage for all variable resolution branches before refactoring

**Changes**:
1. Add tests for cache hit scenario (line 496)
2. Add tests for environment variable resolution with various inputs (line 505)
3. Add tests for file content variable resolution including error cases (line 509)
4. Add tests for command output variable resolution including failures (line 513)
5. Add tests for JSON path extraction with new `:from:` syntax (lines 519-539)
6. Add tests for JSON path extraction with legacy syntax (lines 542-547)
7. Add tests for date formatting with various format strings (line 557)
8. Add tests for UUID generation (already covered but verify line 560)
9. Add tests for caching logic (lines 567-571)
10. Add tests for error conditions in each resolution strategy

**Testing**:
- Run `cargo test variables_test` to verify new tests pass
- Run `cargo tarpaulin --out Stdout --skip-clean -- variables` to measure coverage improvement
- Verify coverage increases from 45% to at least 72%

**Success Criteria**:
- [ ] All 18 uncovered line ranges are now covered
- [ ] Coverage ≥ 72% for resolve_variable_impl
- [ ] All new tests pass
- [ ] All existing tests still pass
- [ ] No clippy warnings
- [ ] Ready to commit

**Test Cases to Add**:
```rust
// Cache hit path
test_variable_cache_hit()

// Environment variable paths
test_env_variable_missing()
test_env_variable_with_special_chars()

// File content paths
test_file_variable_missing_file()
test_file_variable_empty_file()
test_file_variable_binary_content()

// Command output paths
test_cmd_variable_command_failure()
test_cmd_variable_empty_output()
test_cmd_variable_multiline_output()

// JSON path with :from: syntax
test_json_from_syntax_with_string_source()
test_json_from_syntax_with_structured_source()
test_json_from_syntax_missing_path()
test_json_from_syntax_invalid_json()

// JSON path legacy syntax
test_json_legacy_syntax_valid()
test_json_legacy_syntax_invalid_format()
test_json_legacy_syntax_missing_colon()

// Date formatting
test_date_variable_invalid_format()
test_date_variable_various_formats()

// Caching behavior
test_should_cache_expensive_operations()
test_should_not_cache_uuid()
```

### Phase 2: Extract Pure Functions for Variable Resolution Strategies

**Goal**: Extract each variable resolution strategy (env, file, cmd, json, date) into separate pure functions to reduce complexity

**Changes**:
1. Extract `resolve_env_variable(var_name: &str) -> Result<Value>` (complexity ≤3)
2. Extract `resolve_file_variable(path: &str) -> Result<Value>` (complexity ≤3)
3. Extract `resolve_cmd_variable(command: &str) -> Result<Value>` (complexity ≤3)
4. Extract `resolve_json_from_syntax(remainder: &str, depth: usize) -> BoxFuture<Result<Value>>` (complexity ≤5)
5. Extract `resolve_json_legacy_syntax(remainder: &str, depth: usize) -> BoxFuture<Result<Value>>` (complexity ≤4)
6. Extract `resolve_date_variable(format: &str) -> Result<Value>` (complexity ≤3)
7. Refactor `resolve_variable_impl` to dispatch to these pure functions

**Testing**:
- Run `cargo test variables_test` to verify all tests still pass
- Run `cargo clippy` to check for warnings
- Run `cargo tarpaulin` to verify coverage maintained or improved
- Verify cyclomatic complexity reduced (use `cargo clippy -- -W clippy::cognitive_complexity`)

**Success Criteria**:
- [ ] 6 new pure functions extracted with complexity ≤5 each
- [ ] resolve_variable_impl complexity reduced from 31 to ≤15
- [ ] All tests pass (existing + Phase 1 tests)
- [ ] Coverage maintained at ≥72%
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 3: Add Tests for Extracted Pure Functions

**Goal**: Add focused unit tests for each extracted pure function to improve overall coverage and ensure edge cases are handled

**Changes**:
1. Add 3-5 tests per extracted function:
   - `test_resolve_env_variable_*` (3 tests)
   - `test_resolve_file_variable_*` (4 tests)
   - `test_resolve_cmd_variable_*` (4 tests)
   - `test_resolve_json_from_syntax_*` (5 tests)
   - `test_resolve_json_legacy_syntax_*` (4 tests)
   - `test_resolve_date_variable_*` (3 tests)
2. Focus on edge cases, error conditions, and boundary values
3. Add property-based tests for complex logic (JSON parsing, path resolution)

**Testing**:
- Run `cargo test variables_test` to verify all new tests pass
- Run `cargo tarpaulin` to measure final coverage
- Target coverage ≥ 80% overall for the variables module

**Success Criteria**:
- [ ] 23+ new focused unit tests added (3-5 per extracted function)
- [ ] Coverage improved to ≥80% for variables module
- [ ] All edge cases and error conditions tested
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 4: Extract Cache Management Logic

**Goal**: Extract caching logic into pure, testable functions to further reduce complexity

**Changes**:
1. Extract `should_cache_variable(expr: &str) -> bool` as a pure function
2. Extract `get_cached_value(&self, expr: &str) -> Option<Value>`
3. Extract `put_cached_value(&self, expr: &str, value: Value)`
4. Refactor `resolve_variable_impl` to use these extracted functions
5. Reduce remaining complexity in main resolution logic

**Testing**:
- Run `cargo test variables_test` to verify all tests pass
- Run `cargo clippy` to check for warnings
- Verify final cyclomatic complexity ≤10 for resolve_variable_impl

**Success Criteria**:
- [ ] 3 cache management functions extracted
- [ ] resolve_variable_impl cyclomatic complexity ≤10
- [ ] All tests pass
- [ ] Coverage maintained at ≥80%
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Final Verification and Optimization

**Goal**: Verify all goals met, run full test suite, and measure final improvements

**Changes**:
1. Run full CI checks with `just ci`
2. Generate final coverage report with `cargo tarpaulin --out Html`
3. Verify complexity improvements with `cargo clippy`
4. Review extracted functions for further optimization opportunities
5. Add documentation comments to extracted functions

**Testing**:
- `just ci` - Full CI checks including tests, clippy, formatting
- `cargo tarpaulin --out Html` - Generate coverage report
- Review HTML coverage report to identify any remaining gaps
- Manual review of code clarity and maintainability

**Success Criteria**:
- [ ] All CI checks pass (tests, clippy, formatting)
- [ ] Coverage ≥80% (target met)
- [ ] Cyclomatic complexity ≤10 for resolve_variable_impl (improved from 31)
- [ ] All extracted functions have complexity ≤5
- [ ] Documentation added to public functions
- [ ] Code is more maintainable and testable
- [ ] Ready for final commit and completion

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib variables_test` to verify tests in the variables module
2. Run `cargo clippy` to check for warnings and complexity issues
3. Run `cargo tarpaulin --skip-clean -- variables` to measure coverage for variables module
4. Commit working code after each phase completes

**Coverage measurement commands**:
```bash
# Measure coverage for variables module
cargo tarpaulin --skip-clean --out Stdout -- variables

# Generate HTML report for detailed analysis
cargo tarpaulin --skip-clean --out Html -- variables

# Check cyclomatic complexity
cargo clippy -- -W clippy::cognitive_complexity -W clippy::cyclomatic_complexity
```

**Final verification**:
1. `just ci` - Full CI checks (tests, clippy, format)
2. `cargo tarpaulin --out Html` - Final coverage report
3. Manual review of extracted functions for clarity and correctness
4. Verify all success criteria met across all phases

## Rollback Plan

If a phase fails:
1. Review the test failures or errors carefully
2. Revert the phase with `git reset --hard HEAD~1`
3. Analyze why the phase failed:
   - Were tests incorrectly written?
   - Did refactoring break existing functionality?
   - Were there unforeseen dependencies?
4. Adjust the approach:
   - For Phase 1: Review test coverage report to identify truly uncovered branches
   - For Phase 2: Extract smaller, more focused functions
   - For Phase 3: Add simpler tests first, then edge cases
   - For Phase 4: Consider if caching logic can be simplified differently
5. Retry the phase with adjusted approach
6. If stuck after 3 attempts, STOP and document the issue

**Critical safeguards**:
- Never skip tests that fail
- Never disable clippy warnings
- Always verify existing tests pass before committing
- Each commit should leave the codebase in a working state

## Notes

**Key insights from analysis**:

1. **Function is pure logic**: Despite being marked `is_pure: false`, this function is primarily pure business logic with side effects isolated to caching and I/O operations. The caching can be extracted, and I/O is delegated to specialized variable types.

2. **Seven distinct resolution strategies**: The function handles env, file, cmd, json (two formats), date, uuid, and standard lookup. Each strategy is independent and can be extracted.

3. **Existing tests are good but incomplete**: The test file has 18 tests covering basic functionality, but they don't cover error conditions, edge cases, or all branch paths.

4. **Complexity sources**:
   - Multiple nested if-let statements (nesting depth 6)
   - JSON path resolution with two different formats
   - Error handling throughout
   - Caching logic interleaved with resolution logic

5. **Refactoring benefits**:
   - Each extracted function will be easier to test (3-5 tests vs 31+ tests)
   - Error handling will be more localized
   - Caching logic separated from resolution logic
   - Overall function becomes a simple dispatcher

6. **Risk mitigation**:
   - Phase 1 (testing) before Phase 2 (refactoring) ensures we catch regressions
   - Incremental extraction reduces risk
   - Each phase independently commits working code
   - Existing test suite provides safety net

**Pattern recognition**:
- This is a classic "strategy pattern" opportunity
- Each variable type (env, file, cmd, etc.) is a strategy
- Main function becomes a strategy selector
- Each strategy can be tested independently

**Expected final structure**:
```rust
async fn resolve_variable_impl(&self, expr: &str, depth: usize) -> Result<Value> {
    // Check cache
    if let Some(cached) = self.get_cached_value(expr) {
        return Ok(cached);
    }

    // Dispatch to appropriate strategy
    let value = match parse_variable_type(expr) {
        VarType::Env(name) => self.resolve_env_variable(name)?,
        VarType::File(path) => self.resolve_file_variable(path)?,
        VarType::Cmd(cmd) => self.resolve_cmd_variable(cmd)?,
        VarType::JsonFrom(expr) => self.resolve_json_from_syntax(expr, depth).await?,
        VarType::JsonLegacy(expr) => self.resolve_json_legacy_syntax(expr, depth).await?,
        VarType::Date(fmt) => self.resolve_date_variable(fmt)?,
        VarType::Uuid => self.resolve_uuid_variable()?,
        VarType::Standard(path) => self.lookup_variable(path)?,
    };

    // Cache if appropriate
    if self.should_cache_variable(expr) {
        self.put_cached_value(expr, value.clone());
    }

    Ok(value)
}
```

This structure reduces complexity from 31 to ~8, with each strategy function having complexity ≤5.
