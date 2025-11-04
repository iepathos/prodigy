# Implementation Plan: Extract Token Parsing Functions from Tokenizer

## Problem Summary

**Location**: src/cook/execution/expression/tokenizer.rs:tokenize:70
**Priority Score**: 22.3925
**Debt Type**: ComplexityHotspot (cyclomatic: 53, cognitive: 87)
**Current Metrics**:
- Lines of Code: 163
- Cyclomatic Complexity: 53
- Cognitive Complexity: 87
- Function Length: 163
- Nesting Depth: 4

**Issue**: Reduce complexity from 53 to ~10. High complexity 53/87 makes function hard to test and maintain.

## Analysis

The `tokenize` function is a pure function (confirmed by debtmap analysis) with excellent test coverage (32 unit tests). The complexity comes from:

1. **Massive match statement** - Single match with 10+ arms handling different character types
2. **Nested lookahead logic** - Each operator arm checks `chars.peek()` for multi-character operators (==, !=, >=, <=, &&, ||)
3. **Inline keyword matching** - Identifier arm contains a match with 20+ keyword cases
4. **Multiple concerns** - Character-level parsing, operator detection, number parsing, string parsing, keyword recognition all in one function

The function is already well-tested, so we can refactor safely with confidence that tests will catch regressions.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 26.5 (from 53 to ~27, target ~10)
- Coverage Improvement: 0.0 (already well-tested)
- Risk Reduction: 7.84

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 53 to ≤15 per function
- [ ] Each extracted function has single responsibility
- [ ] All 32 existing unit tests continue to pass
- [ ] No new clippy warnings
- [ ] Code is formatted with rustfmt
- [ ] Main tokenize function acts as coordinator, delegating to helpers

## Implementation Phases

### Phase 1: Extract Operator Parsing Functions

**Goal**: Extract multi-character operator parsing logic into focused helper functions, reducing the main match complexity.

**Changes**:
- Extract `parse_operator()` function that handles all operators (!, =, >, <, &, |)
- Returns `Option<Token>` for matched operators
- Consolidates the lookahead logic for multi-character operators
- Reduces main match from 6 operator arms to 1 delegation call

**New Functions**:
```rust
/// Parse operator tokens (!, !=, =, ==, >, >=, <, <=, &&, ||)
fn parse_operator(ch: char, chars: &mut Peekable<Chars>) -> Result<Option<Token>>
```

**Testing**:
- All existing operator tests should pass:
  - `test_tokenize_equal`, `test_tokenize_not_equal`
  - `test_tokenize_greater`, `test_tokenize_greater_equal`
  - `test_tokenize_less`, `test_tokenize_less_equal`
  - `test_tokenize_and_symbols`, `test_tokenize_or_symbols`
  - `test_tokenize_not_symbol`
- Error tests for single & and | should still work

**Success Criteria**:
- [ ] Operator parsing logic extracted to pure function
- [ ] Main match reduced by 6 arms (consolidated to 1 operator check)
- [ ] All operator tests pass (14 tests)
- [ ] `cargo test tokenize::` passes
- [ ] `cargo clippy` reports no new warnings
- [ ] Ready to commit

**Estimated Complexity Reduction**: -6 cyclomatic complexity

---

### Phase 2: Extract String and Number Parsing

**Goal**: Extract literal parsing (strings and numbers) into dedicated functions.

**Changes**:
- Extract `parse_string(quote: char, chars: &mut Peekable<Chars>) -> Result<String>`
  - Handles both single and double quote strings
  - Returns the parsed string content
- Extract `parse_number(chars: &mut Peekable<Chars>) -> Result<f64>`
  - Handles integers, floats, and negative numbers
  - Returns the parsed number
- Main match delegates to these functions for '" | '\'' and '0'..='9' | '-' arms

**New Functions**:
```rust
/// Parse a quoted string literal
fn parse_string(quote: char, chars: &mut Peekable<Chars>) -> Result<String>

/// Parse a numeric literal (integer or float, positive or negative)
fn parse_number(chars: &mut Peekable<Chars>) -> Result<f64>
```

**Testing**:
- String tests should pass:
  - `test_tokenize_string_double_quotes`
  - `test_tokenize_string_single_quotes`
- Number tests should pass:
  - `test_tokenize_number`
  - `test_tokenize_negative_number`
  - `test_tokenize_float`
  - `test_tokenize_number_followed_by_identifier`

**Success Criteria**:
- [ ] String parsing extracted to pure function
- [ ] Number parsing extracted to pure function
- [ ] Main match reduced by 2 more arms
- [ ] All string and number tests pass (7 tests)
- [ ] `cargo test tokenize::` passes
- [ ] No clippy warnings
- [ ] Ready to commit

**Estimated Complexity Reduction**: -4 cyclomatic complexity

---

### Phase 3: Extract Keyword Recognition

**Goal**: Extract the massive keyword matching logic from the identifier parsing arm.

**Changes**:
- Extract `parse_keyword_or_identifier(ident: String) -> Token`
  - Takes the raw identifier string
  - Returns appropriate Token (keyword or Identifier)
  - Encapsulates the 20+ arm match for keywords
- Main identifier parsing arm simplified to:
  1. Collect identifier characters
  2. Delegate to `parse_keyword_or_identifier()`

**New Function**:
```rust
/// Convert identifier string to keyword token or Identifier token
fn parse_keyword_or_identifier(ident: String) -> Token
```

**Testing**:
- Keyword tests should pass:
  - `test_tokenize_boolean_true`, `test_tokenize_boolean_false`
  - `test_tokenize_null`, `test_tokenize_and_word`, `test_tokenize_or_word`, `test_tokenize_not_word`
  - `test_tokenize_contains`, `test_tokenize_starts_with`, `test_tokenize_ends_with`, `test_tokenize_matches`
  - `test_tokenize_length`, `test_tokenize_sum`, `test_tokenize_count`, `test_tokenize_min`, `test_tokenize_max`, `test_tokenize_avg`
- Identifier tests should pass:
  - `test_tokenize_identifier`, `test_tokenize_field_path`, `test_tokenize_array_wildcard`

**Success Criteria**:
- [ ] Keyword matching extracted to pure function
- [ ] Identifier arm simplified significantly
- [ ] All keyword and identifier tests pass (20+ tests)
- [ ] `cargo test tokenize::` passes
- [ ] No clippy warnings
- [ ] Ready to commit

**Estimated Complexity Reduction**: -20 cyclomatic complexity (from keyword match)

---

### Phase 4: Extract Identifier Character Parsing

**Goal**: Extract the identifier character collection logic into a focused function.

**Changes**:
- Extract `parse_identifier(chars: &mut Peekable<Chars>) -> String`
  - Collects identifier characters (alphanumeric, _, ., [, ], *)
  - Returns the raw identifier string
  - Removes the nested while loop from main match
- Main identifier arm becomes:
  1. Call `parse_identifier(chars)`
  2. Call `parse_keyword_or_identifier(ident)`
  3. Push resulting token

**New Function**:
```rust
/// Parse an identifier (variable name, field path, or array accessor)
fn parse_identifier(chars: &mut Peekable<Chars>) -> String
```

**Testing**:
- Same identifier tests from Phase 3 should continue to pass
- Complex identifier tests:
  - `test_tokenize_field_path` (user.profile.name)
  - `test_tokenize_array_wildcard` (items[*].score)

**Success Criteria**:
- [ ] Identifier character collection extracted
- [ ] Main match arm simplified to 3 function calls
- [ ] All identifier tests pass
- [ ] `cargo test tokenize::` passes
- [ ] No clippy warnings
- [ ] Ready to commit

**Estimated Complexity Reduction**: -2 cyclomatic complexity

---

### Phase 5: Final Cleanup and Verification

**Goal**: Verify all improvements and document the refactored architecture.

**Changes**:
- Add module-level documentation explaining the parsing strategy
- Ensure all helper functions have clear doc comments
- Verify cyclomatic complexity of main `tokenize` function ≤15
- Run full test suite and linting

**Documentation Updates**:
- Update module-level doc comment to explain tokenizer architecture
- Document each helper function with examples and edge cases
- Add comment in main `tokenize` explaining the delegation pattern

**Testing**:
- Run `cargo test --lib` - all tests must pass
- Run `cargo clippy` - no warnings
- Run `cargo fmt --check` - properly formatted
- Verify complex expression tests pass:
  - `test_tokenize_simple_comparison`
  - `test_tokenize_complex_expression`
  - `test_tokenize_with_parentheses`
  - `test_tokenize_function_call`

**Success Criteria**:
- [ ] All 32 unit tests pass
- [ ] Main `tokenize` function has cyclomatic complexity ≤15
- [ ] Each helper function has cyclomatic complexity ≤10
- [ ] All functions properly documented
- [ ] No clippy warnings
- [ ] Code formatted with rustfmt
- [ ] Ready for final commit

**Estimated Complexity Reduction**: Final verification of ~32 point reduction (53 → ~21)

## Testing Strategy

**For each phase**:
1. Run `cargo test tokenize::` to verify tokenizer tests pass
2. Run `cargo clippy -- -D warnings` to ensure no new warnings
3. Run `cargo fmt` to ensure proper formatting
4. Manually review the complexity reduction

**Final verification**:
1. `cargo test --lib` - All tests pass
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Properly formatted
4. Visual inspection of `tokenize()` function - should be <50 lines, mostly delegation

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures or clippy warnings
3. Identify the issue (likely: missed edge case, incorrect delegation, signature mismatch)
4. Adjust the implementation
5. Retry the phase

## Notes

**Why This Approach Works**:
- The function is already pure and well-tested (32 tests)
- Each extraction maintains the same external behavior
- Helper functions are also pure and testable
- Incremental extraction reduces risk
- Existing tests provide safety net

**Complexity Reduction Breakdown**:
- Phase 1 (operators): -6 complexity
- Phase 2 (literals): -4 complexity
- Phase 3 (keywords): -20 complexity
- Phase 4 (identifier parsing): -2 complexity
- **Total**: ~32 complexity reduction (53 → ~21)

**Target**: While we won't reach cyclomatic complexity of 10 for the main function (too aggressive), we will:
- Reduce main `tokenize` to ~21 complexity (match arms + minimal logic)
- Create 5 helper functions with ≤10 complexity each
- Achieve the spirit of the recommendation: "function hard to test and maintain" → "simple coordinator with testable helpers"

**Edge Cases to Watch**:
- String parsing: Unclosed quotes (currently consumes rest of input)
- Number parsing: Invalid formats (e.g., "1.2.3")
- Operators: Single & or | should still error
- Identifiers: Complex paths like `items[*].score`
