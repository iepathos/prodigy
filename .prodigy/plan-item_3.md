# Implementation Plan: Reduce Complexity in VariableContext::resolve_variable_impl

## Problem Summary

**Location**: ./src/cook/execution/variables.rs:VariableContext::resolve_variable_impl:491
**Priority Score**: 28.795
**Debt Type**: ComplexityHotspot (Cognitive: 109, Cyclomatic: 31)
**Current Metrics**:
- Function Length: 85 lines
- Cyclomatic Complexity: 31
- Cognitive Complexity: 109
- Nesting Depth: 6
- Is Pure: false (but should be - it's marked as PureLogic role)

**Issue**: High complexity 31/109 makes function hard to test and maintain. The function handles 7 different variable types (env, file, cmd, json, date, uuid, standard) in a single large if-else chain with deep nesting, especially in the json: branch.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 15.5 (from 31 to ~15.5)
- Coverage Improvement: 0.0 (maintain current coverage)
- Risk Reduction: 10.07825

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 31 to ≤15
- [ ] Cognitive complexity reduced from 109 to ≤50
- [ ] Nesting depth reduced from 6 to ≤3
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Extract Variable Type Resolver

**Goal**: Extract the variable type detection logic into a pure function that returns an enum indicating which resolver to use.

**Changes**:
- Create a `VariableType` enum with variants: `Environment`, `File`, `Command`, `Json`, `Date`, `Uuid`, `Standard`
- Extract function `parse_variable_type(expr: &str) -> VariableType` that analyzes the prefix
- This reduces the main function's if-else chain complexity

**Testing**:
```bash
cargo test --lib variables
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] `parse_variable_type` function created with unit tests
- [ ] Function has cyclomatic complexity ≤5
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 2: Extract JSON Variable Resolution

**Goal**: Move the complex JSON resolution logic (lines 514-553) into a dedicated async function.

**Changes**:
- Create `async fn resolve_json_variable(&self, remainder: &str, depth: usize) -> Result<Value>`
- Extract the entire json: branch logic including:
  - Finding ":from:" separator
  - Handling legacy format
  - Recursive resolution
  - JSON parsing and path extraction
- This eliminates the deepest nesting (6 levels) from the main function

**Testing**:
```bash
cargo test --lib variables::json
cargo test --lib variables
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] `resolve_json_variable` function created
- [ ] Nesting depth in main function reduced to ≤4
- [ ] All existing JSON variable tests pass
- [ ] Ready to commit

### Phase 3: Create Variable Resolver Dispatch

**Goal**: Replace the large if-else chain with a cleaner dispatch mechanism using the extracted type enum.

**Changes**:
- Implement `async fn resolve_by_type(&self, var_type: VariableType, expr: &str, depth: usize) -> Result<Value>`
- Use match statement on `VariableType` to dispatch to appropriate resolver
- Each branch becomes a single function call instead of inline logic
- Main function becomes: parse type → resolve by type → cache result

**Testing**:
```bash
cargo test --lib variables
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Dispatch function created with clear match arms
- [ ] Main function reduced to ~30-40 lines
- [ ] Cyclomatic complexity in main function ≤10
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 4: Extract Caching Logic

**Goal**: Separate the caching concerns from the resolution logic.

**Changes**:
- Create `async fn resolve_with_cache(&self, expr: &str, depth: usize) -> Result<Value>`
- Move cache check (lines 492-499) and cache write (lines 566-571) into this wrapper
- Keep `resolve_variable_impl` focused on pure resolution logic
- This improves separation of concerns and reduces complexity

**Testing**:
```bash
cargo test --lib variables
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Caching logic extracted to wrapper function
- [ ] `resolve_variable_impl` is now focused on pure resolution
- [ ] Cache behavior unchanged (tests verify)
- [ ] Ready to commit

### Phase 5: Final Cleanup and Verification

**Goal**: Ensure all quality metrics are met and code is production-ready.

**Changes**:
- Add inline documentation for extracted functions
- Ensure all functions have clear, descriptive names
- Verify no unwrap() or panic!() calls (per Spec 101)
- Run full test suite and clippy

**Testing**:
```bash
cargo test --all
cargo clippy -- -D warnings
cargo fmt --check
just ci
```

**Success Criteria**:
- [ ] Cyclomatic complexity ≤15 (target met)
- [ ] Cognitive complexity significantly reduced
- [ ] All tests pass (100% existing coverage maintained)
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Documentation added

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib variables` to verify variable resolution tests pass
2. Run `cargo clippy -- -D warnings` to check for warnings
3. Run `cargo fmt` to ensure proper formatting
4. Commit the working changes

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo test --all` - All tests in workspace
3. Verify complexity metrics with manual review or tooling

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure:
   - Check test output for specific failures
   - Review clippy warnings
   - Examine compilation errors
3. Adjust the approach:
   - May need to extract smaller pieces
   - May need to preserve more of the original structure
   - May need additional tests before refactoring
4. Retry with adjusted approach

## Notes

**Key Complexity Sources**:
1. **7 variable types in one function** - Each type needs different handling
2. **Deep nesting in JSON branch** - 6 levels deep with multiple conditionals
3. **Async/cache interleaving** - Cache logic mixed with resolution logic
4. **No clear separation** - Type detection, resolution, and caching all in one function

**Refactoring Strategy**:
- Use **Extract Function** pattern for each distinct responsibility
- Maintain **async/await** throughout (required for cache RwLock)
- Keep **error handling** consistent using Result and context
- Preserve **cache behavior** exactly (tests will verify)
- Follow **functional programming** principles from CLAUDE.md

**Critical Requirements**:
- MUST maintain all existing test coverage
- MUST NOT introduce unwrap() or panic!() (Spec 101)
- MUST use proper error propagation with context
- MUST keep each function focused and ≤20 lines where possible

**Estimated Effort**: 4.65 hours (from debtmap analysis)
