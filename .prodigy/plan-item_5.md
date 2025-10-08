# Implementation Plan: Test Coverage and Refactoring for get_files_to_stage

## Problem Summary

**Location**: ./src/cook/commit_tracker.rs:CommitTracker::get_files_to_stage:307
**Priority Score**: 31.6125
**Debt Type**: TestingGap (0% coverage)
**Current Metrics**:
- Lines of Code: 60
- Cyclomatic Complexity: 16
- Cognitive Complexity: 66
- Coverage: 0.0%
- Nesting Depth: 8

**Issue**: Complex business logic with 100% testing gap. Cyclomatic complexity of 16 requires at least 16 test cases for full path coverage. The function handles git status parsing, pattern matching for includes/excludes, with deeply nested conditionals. Testing before refactoring ensures no regressions, then extract pure functions to reduce complexity.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 4.8 (from 16 to ~11)
- Coverage Improvement: 50.0% (from 0% to 50%+, targeting 80%+)
- Risk Reduction: 13.27725

**Success Criteria**:
- [ ] 80%+ test coverage for `get_files_to_stage` and extracted functions
- [ ] Cyclomatic complexity ≤3 per extracted function
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper rustfmt formatting
- [ ] Pure functions separated from I/O operations

## Implementation Phases

### Phase 1: Add Comprehensive Tests for Current Implementation

**Goal**: Achieve 80%+ coverage by testing all critical branches before refactoring

**Changes**:
- Add test for basic file staging (no config)
- Add test for include patterns matching
- Add test for include patterns not matching
- Add test for exclude patterns blocking files
- Add test for include + exclude interaction
- Add test for empty git status output
- Add test for malformed git status lines (< 3 chars)
- Add test for invalid glob patterns

**Testing**:
```bash
cargo test get_files_to_stage
cargo tarpaulin --lib -- get_files_to_stage
```

**Success Criteria**:
- [ ] 8+ test cases covering all major branches
- [ ] All tests pass
- [ ] Coverage ≥80% for `get_files_to_stage`
- [ ] Ready to commit with message "test: add comprehensive coverage for get_files_to_stage"

### Phase 2: Extract Git Status Parsing Logic

**Goal**: Separate I/O from pure parsing logic

**Changes**:
- Extract `parse_git_status_line(line: &str) -> Option<String>` pure function
  - Returns `Some(filename)` if line is valid (length > 3)
  - Returns `None` for invalid lines
  - Handles trimming and substring extraction
- Update `get_files_to_stage` to use new function
- Add unit tests for `parse_git_status_line`:
  - Valid status line: `"M  src/file.rs"` → `Some("src/file.rs")`
  - Short line: `"M "` → `None`
  - Empty line: `""` → `None`
  - Line with spaces: `"A  path with spaces/file.rs"` → `Some("path with spaces/file.rs")`

**Testing**:
```bash
cargo test parse_git_status_line
cargo clippy
cargo fmt --check
```

**Success Criteria**:
- [ ] New pure function with complexity ≤3
- [ ] 4+ tests for `parse_git_status_line`
- [ ] All existing tests still pass
- [ ] No clippy warnings
- [ ] Ready to commit with message "refactor: extract git status parsing to pure function"

### Phase 3: Extract Include Pattern Matching Logic

**Goal**: Extract include pattern evaluation to testable pure function

**Changes**:
- Extract `should_include_file(file: &str, include_patterns: &[String]) -> bool` pure function
  - Returns `false` if `include_patterns` is empty (no patterns = exclude all)
  - Returns `true` if any pattern matches
  - Handles invalid patterns gracefully
- Update `get_files_to_stage` to use new function
- Add unit tests for `should_include_file`:
  - No patterns: `should_include_file("file.rs", &[])` → `false`
  - Single match: `should_include_file("file.rs", &["*.rs"])` → `true`
  - Single no-match: `should_include_file("file.txt", &["*.rs"])` → `false`
  - Multiple patterns, first matches: → `true`
  - Multiple patterns, second matches: → `true`
  - Invalid pattern (graceful handling)

**Testing**:
```bash
cargo test should_include_file
cargo test get_files_to_stage
```

**Success Criteria**:
- [ ] New pure function with complexity ≤3
- [ ] 6+ tests for `should_include_file`
- [ ] All existing tests still pass
- [ ] Ready to commit with message "refactor: extract include pattern matching to pure function"

### Phase 4: Extract Exclude Pattern Filtering Logic

**Goal**: Extract exclude pattern evaluation to testable pure function

**Changes**:
- Extract `should_exclude_file(file: &str, exclude_patterns: &[String]) -> bool` pure function
  - Returns `false` if `exclude_patterns` is empty (no patterns = exclude nothing)
  - Returns `true` if any pattern matches
  - Handles invalid patterns gracefully
- Update `get_files_to_stage` to use new function
- Add unit tests for `should_exclude_file`:
  - No patterns: `should_exclude_file("file.rs", &[])` → `false`
  - Single match: `should_exclude_file("file.tmp", &["*.tmp"])` → `true`
  - Single no-match: `should_exclude_file("file.rs", &["*.tmp"])` → `false`
  - Multiple patterns with match
  - Invalid pattern (graceful handling)

**Testing**:
```bash
cargo test should_exclude_file
cargo test get_files_to_stage
```

**Success Criteria**:
- [ ] New pure function with complexity ≤3
- [ ] 5+ tests for `should_exclude_file`
- [ ] All existing tests still pass
- [ ] Ready to commit with message "refactor: extract exclude pattern filtering to pure function"

### Phase 5: Extract File Filtering Decision Logic

**Goal**: Combine include/exclude logic into a single decision function

**Changes**:
- Extract `should_stage_file(file: &str, config: Option<&CommitConfig>) -> bool` pure function
  - Handles `None` config → `true` (stage all files)
  - Handles include patterns using `should_include_file`
  - Handles exclude patterns using `should_exclude_file`
  - Combines both with proper precedence (include first, then exclude)
- Update `get_files_to_stage` to use new function
- Add integration tests for `should_stage_file`:
  - No config → `true`
  - Only include patterns (matching and non-matching)
  - Only exclude patterns (matching and non-matching)
  - Both include and exclude (file passes include, blocked by exclude)
  - Both include and exclude (file passes both)

**Testing**:
```bash
cargo test should_stage_file
cargo test get_files_to_stage
```

**Success Criteria**:
- [ ] New pure function with complexity ≤3
- [ ] 6+ tests for `should_stage_file`
- [ ] `get_files_to_stage` now has complexity ≤5
- [ ] All existing tests still pass
- [ ] Ready to commit with message "refactor: extract file filtering decision to pure function"

### Phase 6: Final Verification and Coverage Check

**Goal**: Verify all metrics meet target state

**Changes**:
- Run full test suite
- Generate coverage report
- Run clippy for final check
- Verify complexity metrics

**Testing**:
```bash
just ci
cargo tarpaulin --lib
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] All tests pass (including existing test suite)
- [ ] Coverage ≥80% for `get_files_to_stage` and all extracted functions
- [ ] Cyclomatic complexity ≤3 for all extracted functions
- [ ] Overall complexity reduced by ~5 points
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Ready to commit with message "chore: verify test coverage and complexity improvements for file staging"

## Testing Strategy

**For each phase**:
1. Write tests FIRST (TDD approach where possible)
2. Run `cargo test --lib` to verify tests pass
3. Run `cargo clippy` to check for warnings
4. Run `cargo fmt` to ensure formatting
5. Commit working changes

**Pattern Matching Test Coverage**:
- Valid glob patterns: `*.rs`, `src/**/*.rs`, `test_*.rs`
- Invalid patterns: Handle gracefully without panicking
- Edge cases: Empty patterns, patterns with spaces

**Git Status Parsing Coverage**:
- Normal status lines: `M  file.rs`, `A  new.rs`, `D  old.rs`
- Renamed files: `R  old.rs -> new.rs`
- Short lines: `M ` (< 3 chars)
- Empty lines
- Lines with special characters

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Coverage report
3. Visual inspection of reduced nesting in `get_files_to_stage`

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure (test failure, clippy warning, or complexity increase)
3. Adjust the approach:
   - For test failures: Review test logic and fix
   - For clippy warnings: Address before committing
   - For complexity issues: Re-evaluate extraction strategy
4. Retry with corrected approach

## Notes

### Function Extraction Strategy

The extraction follows a clear progression:
1. **Phase 2**: Isolate I/O parsing (git status line → filename)
2. **Phase 3**: Extract include logic (filename + patterns → bool)
3. **Phase 4**: Extract exclude logic (filename + patterns → bool)
4. **Phase 5**: Combine logic (filename + config → final decision)

This creates a clear data flow:
```
git status → parse_git_status_line → filename
                                     ↓
filename + config → should_stage_file → bool
                    (uses should_include_file and should_exclude_file)
```

### Pure Function Benefits

All extracted functions are **pure** (no side effects):
- Easy to unit test
- No mocking required
- Deterministic behavior
- Can be tested in parallel
- Clear input/output contracts

### Complexity Reduction

Current complexity: 16 (from nested if/let/for loops)

After extraction:
- `parse_git_status_line`: ~2 (simple string check)
- `should_include_file`: ~3 (loop with early return)
- `should_exclude_file`: ~3 (loop with early return)
- `should_stage_file`: ~3 (two function calls + option handling)
- `get_files_to_stage`: ~5 (async I/O + iteration with filtering)

Total complexity distributed: ~16 → 5 main + 11 in helpers
Each helper function: ≤3 (easy to understand and test)

### Testing Guidelines

Follow existing test patterns from `commit_tracker_tests.rs`:
- Use descriptive test names: `test_<function>_<scenario>`
- Use `MockGitOperations` for I/O operations
- Use `assert_eq!` and `assert!` for clear assertions
- Group related tests in the same module
- Add comments for complex test scenarios
