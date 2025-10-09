# Implementation Plan: Reduce Nesting in GitRunnerImpl::status

## Problem Summary

**Location**: ./src/subprocess/git.rs:GitRunnerImpl::status:52
**Priority Score**: 54.7625
**Debt Type**: ComplexityHotspot (cognitive: 30, cyclomatic: 12)
**Current Metrics**:
- Lines of Code: 41
- Functions: 1 (monolithic)
- Cyclomatic Complexity: 12
- Nesting Depth: 4 levels
- Coverage: Extensively tested (40 upstream callers, 35+ test cases)

**Issue**: Function has 4 levels of nesting (for loop → if → if let → operations) which increases cognitive complexity. The debtmap analysis recommends reducing nesting from 4 levels using early returns and extracting deeply nested blocks into separate functions.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 6.0 points (from 12 to ~6)
- Coverage Improvement: 0.0 (already well-tested)
- Risk Reduction: 19.166875

**Success Criteria**:
- [ ] Nesting depth reduced from 4 to 2 or less
- [ ] Cyclomatic complexity reduced by extracting pure functions
- [ ] All 35+ existing tests continue to pass without modification
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Code is more readable and easier to understand

## Implementation Phases

### Phase 1: Extract Line Parsing Logic into Pure Functions

**Goal**: Extract the nested line parsing logic into separate, testable pure functions to reduce complexity and nesting.

**Changes**:
- Extract branch line parsing into `parse_branch_line(line: &str) -> Option<String>`
  - Handles `"## "` prefix stripping and upstream split logic
  - Pure function with no side effects
- Extract untracked file parsing into `parse_untracked_line(line: &str) -> Option<String>`
  - Handles `"?? "` prefix stripping
  - Returns the filename if the line is an untracked file marker
- Extract modified file parsing into `parse_modified_line(line: &str) -> Option<String>`
  - Handles all other file status lines (modified, added, deleted, renamed, copied)
  - Checks line length > 2 and extracts from position 3 onward
- Add these as module-level pure functions with `#[inline]` for performance

**Testing**:
- Run existing tests: `cargo test --lib subprocess::git`
- All 35+ tests should pass without modification
- Verify line parsing edge cases still work correctly

**Success Criteria**:
- [ ] Three new pure functions added
- [ ] Functions are properly documented
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 2: Refactor status() to Use Early Continue Pattern

**Goal**: Simplify the main loop by using the extracted functions and early continue pattern to eliminate nested if-else chains.

**Changes**:
- Restructure the for loop to use pattern matching with extracted functions:
  ```rust
  for line in output.stdout.lines() {
      // Early continue for empty lines (implicitly handled by patterns not matching)

      if let Some(branch_name) = parse_branch_line(line) {
          branch = Some(branch_name);
          continue;
      }

      if let Some(file) = parse_untracked_line(line) {
          untracked_files.push(file);
          continue;
      }

      if let Some(file) = parse_modified_line(line) {
          modified_files.push(file);
          continue;
      }
  }
  ```
- This pattern:
  - Reduces nesting from 4 levels to 2 levels
  - Makes each case independent and clear
  - Uses early continue for control flow
  - Leverages the extracted pure functions

**Testing**:
- Run all git tests: `cargo test --lib subprocess::git`
- Verify comprehensive test still passes: `test_status_comprehensive_all_scenarios`
- Test edge cases: empty lines, whitespace, malformed input

**Success Criteria**:
- [ ] Nesting reduced to 2 levels maximum
- [ ] Control flow simplified with early continue
- [ ] All existing tests pass
- [ ] Code is more readable
- [ ] Ready to commit

### Phase 3: Add Inline Documentation and Verify Complexity Reduction

**Goal**: Add clear documentation to the refactored code and verify that complexity metrics have improved.

**Changes**:
- Add doc comments to the three pure parsing functions explaining:
  - What git porcelain format they parse
  - Edge cases they handle
  - Return value semantics
- Add inline comment in `status()` explaining the porcelain format structure
- Run clippy to ensure no new warnings: `cargo clippy --lib`
- Run formatter: `cargo fmt`

**Testing**:
- Full test suite: `cargo test`
- Clippy validation: `cargo clippy`
- Format check: `cargo fmt -- --check`

**Success Criteria**:
- [ ] All functions have clear documentation
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] All tests pass
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run focused tests: `cargo test --lib subprocess::git` to verify git functionality
2. Run `cargo clippy -- -D warnings` to check for any new warnings
3. Verify the 35+ existing tests all pass (especially edge case tests added in previous phases)

**Critical test coverage to preserve**:
- Branch parsing with upstream info
- Untracked files (`??` prefix)
- Modified files (all status codes: M, A, D, R, C, MM, AM, etc.)
- Edge cases: empty lines, whitespace-only lines, line length boundaries
- Malformed input handling

**Final verification**:
1. `just ci` - Full CI checks (build, test, clippy, fmt)
2. Manual verification that nesting depth is reduced
3. Verify cyclomatic complexity reduction by inspecting the simplified control flow

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures to understand what edge case was broken
3. Adjust the implementation to handle the edge case
4. Re-run tests and retry the phase

Since this function is extensively tested (35+ tests covering edge cases), any regression will be immediately caught.

## Notes

### Why This Refactoring is Low Risk:

1. **Extensive test coverage**: 40 upstream callers and 35+ test cases covering:
   - Clean repository
   - Branch parsing (with/without upstream, special characters, spaces, very long names)
   - File status codes (M, A, D, R, C, MM, AM, etc.)
   - Edge cases (empty lines, whitespace, line length boundaries)
   - Error conditions (exit codes, malformed input)

2. **Pure function extraction**: The parsing logic being extracted has no side effects and is easy to reason about

3. **Incremental approach**: Each phase builds on the previous one and maintains all existing behavior

### Functional Programming Principles Applied:

- **Pure functions**: All parsing logic extracted into side-effect-free functions
- **Separation of I/O from logic**: Git command execution (I/O) remains separate from parsing (pure logic)
- **Single responsibility**: Each parsing function handles one specific line type
- **Composition**: The main loop composes the pure parsing functions

### Expected Complexity Reduction:

**Before**:
- Nesting depth: 4 levels
- Cyclomatic complexity: 12
- Pattern: Nested if-else chains in a loop

**After**:
- Nesting depth: 2 levels (for loop + if let)
- Cyclomatic complexity: ~6 (complexity moved to pure functions)
- Pattern: Early continue with pure function calls

The complexity isn't eliminated, but it's:
- Distributed across focused, testable pure functions
- Made more readable through clear patterns
- Easier to extend with new file status types
