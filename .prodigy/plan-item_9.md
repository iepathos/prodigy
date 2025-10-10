# Implementation Plan: Reduce Complexity in FilePatternInputProvider::generate_inputs

## Problem Summary

**Location**: ./src/cook/input/file_pattern.rs:FilePatternInputProvider::generate_inputs:51
**Priority Score**: 28.827423093799048
**Debt Type**: ComplexityHotspot (Cognitive: 42, Cyclomatic: 13)
**Current Metrics**:
- Lines of Code: 134
- Nesting Depth: 5 levels
- Cyclomatic Complexity: 13
- Cognitive Complexity: 42
- Coverage: Not specified

**Issue**: The `generate_inputs` function has excessive nesting (5 levels) and high cognitive complexity (42). The function mixes multiple responsibilities: pattern expansion, file discovery, variable creation, and metadata construction. The recommendation is to reduce nesting using early returns and extract deeply nested blocks into separate pure functions.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 6.5
- Coverage Improvement: 0.0
- Risk Reduction: 10.089598082829664

**Success Criteria**:
- [ ] Nesting depth reduced from 5 to 2-3 levels maximum
- [ ] Cyclomatic complexity reduced from 13 to ~6-7
- [ ] Cognitive complexity reduced from 42 to ~20-25
- [ ] Function broken into smaller, testable pure functions
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Extract File Variable Creation

**Goal**: Extract the deeply nested variable creation logic (lines 115-157) into a pure function that takes a PathBuf and returns a Vec of (String, VariableValue) tuples.

**Changes**:
- Create a new pure function `create_file_variables(file_path: &Path) -> Vec<(String, VariableValue)>`
- Move lines 115-157 into this function
- Replace the nested variable creation calls with a single call to the new function and iteration over results
- This reduces nesting from 5 to 4 levels and extracts 43 lines into a testable unit

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings
- Function should compile and maintain existing behavior

**Success Criteria**:
- [ ] New `create_file_variables` function exists and is pure
- [ ] Function is under 50 lines
- [ ] No nested if/match statements in the extracted function
- [ ] All existing tests pass
- [ ] No clippy warnings

### Phase 2: Extract Metadata Creation

**Goal**: Extract metadata creation logic (lines 166-177) into a pure function that takes a PathBuf and metadata and returns InputMetadata.

**Changes**:
- Create a new pure function `create_input_metadata(file_path: &Path, metadata: &fs::Metadata) -> InputMetadata`
- Move lines 166-177 into this function
- Replace the inline metadata creation with a call to the new function
- This further reduces complexity and isolates metadata construction

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings
- Verify metadata fields are correctly populated

**Success Criteria**:
- [ ] New `create_input_metadata` function exists and is pure
- [ ] Function is under 20 lines
- [ ] All existing tests pass
- [ ] No clippy warnings

### Phase 3: Extract Pattern Expansion Logic

**Goal**: Extract the pattern expansion logic (lines 63-67) into a pure function that takes a pattern string and recursive flag and returns the expanded pattern.

**Changes**:
- Create a new pure function `expand_pattern(pattern: &str, recursive: bool) -> String`
- Move the pattern expansion logic into this function
- Replace inline pattern expansion with a call to the new function
- This isolates decision logic and makes it testable

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Consider adding unit tests for pattern expansion edge cases
- Verify recursive and non-recursive patterns work correctly

**Success Criteria**:
- [ ] New `expand_pattern` function exists and is pure
- [ ] Function is under 10 lines
- [ ] All existing tests pass
- [ ] No clippy warnings

### Phase 4: Extract File Discovery Logic

**Goal**: Extract the file discovery logic (lines 57-87) into a separate function that takes patterns and recursive flag and returns a HashSet of PathBufs.

**Changes**:
- Create a new function `discover_files(patterns: &[serde_json::Value], recursive: bool) -> Result<HashSet<PathBuf>>`
- Move the pattern iteration and glob logic into this function
- Use the `expand_pattern` function from Phase 3
- Replace lines 57-87 with a single call to `discover_files`
- This reduces main function nesting from 4 to 2 levels

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings
- Verify glob errors are handled correctly
- Verify inaccessible files are skipped silently

**Success Criteria**:
- [ ] New `discover_files` function exists
- [ ] Function is under 40 lines
- [ ] Uses `expand_pattern` helper
- [ ] Error handling preserved
- [ ] All existing tests pass
- [ ] No clippy warnings

### Phase 5: Simplify Main Function with Early Returns

**Goal**: Simplify the remaining logic in `generate_inputs` by using early returns and the extracted helper functions.

**Changes**:
- Refactor lines 91-181 to use the extracted helpers
- Use early returns for error cases to reduce nesting
- Result should be a clear, linear flow:
  1. Get configuration
  2. Discover files (call `discover_files`)
  3. For each file, build ExecutionInput using helpers
  4. Return inputs
- Main function should now be under 50 lines with nesting depth of 2

**Testing**:
- Run `cargo test --lib` to verify all existing tests pass
- Run `cargo clippy` to ensure no warnings
- Run `cargo fmt` to ensure proper formatting
- Verify the function behavior is identical to the original

**Success Criteria**:
- [ ] Main function is under 50 lines
- [ ] Nesting depth is 2 levels maximum
- [ ] All helper functions are called correctly
- [ ] Error handling is preserved
- [ ] All existing tests pass
- [ ] No clippy warnings
- [ ] Code is properly formatted

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to format code
4. Review the diff to ensure changes are minimal and focused

**Final verification**:
1. `just ci` - Full CI checks
2. Compare complexity metrics before/after using a complexity analyzer
3. Verify nesting depth is reduced to 2-3 levels
4. Verify all existing tests pass without modification

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure and error messages
3. Adjust the approach (e.g., keep function signatures simpler, preserve more of the original structure)
4. Retry the phase with the adjusted approach

## Notes

**Key Insights**:
- The function is marked as `PureLogic` role but contains I/O operations (fs::metadata, glob). The pure functions we extract should truly be pure (no I/O).
- The pattern repetition score (0.77) suggests there's repeated logic that can be factored out.
- We're not changing behavior, only structure - all tests should pass without modification.
- Focus on reducing cognitive load: fewer nested blocks, clearer separation of concerns.

**Gotchas**:
- The function checks file accessibility twice (once during glob, once during iteration) - preserve this defensive approach.
- Error handling uses `eprintln!` for non-fatal errors - maintain this pattern.
- The `HashSet` is used to deduplicate files across patterns - preserve this behavior.
- The `unwrap_or_default()` calls are safe for path components - keep them.

**Success Markers**:
- When complete, the main function should read like a recipe: get config, find files, build inputs, return.
- Each extracted function should have a single, clear purpose.
- Nesting should feel natural, not forced or deeply nested.
- The code should be boring and obvious, not clever.
