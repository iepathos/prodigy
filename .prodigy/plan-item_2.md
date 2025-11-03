# Implementation Plan: Refactor Expression Parser into Focused Modules

## Problem Summary

**Location**: ./src/cook/execution/expression/parser.rs:file:0
**Priority Score**: 104.23
**Debt Type**: God Object / High Complexity
**Current Metrics**:
- Lines of Code: 661
- Functions: 12
- Cyclomatic Complexity: 154 (avg: 12.8, max: 53)
- Coverage: 0%

**Issue**: The expression parser combines multiple responsibilities into a single monolithic module: tokenization, AST construction, and parsing logic. The recommendation is to split the parser into 3 modules: 1) Tokenizer/Lexer 2) AST builder 3) Visitor/Walker 4) Error handling, grouped by parsing phase rather than node type. This mixing of concerns leads to high complexity (max complexity of 53) and makes the code difficult to test and maintain.

## Target State

**Expected Impact**:
- Complexity Reduction: 30.8 points
- Maintainability Improvement: 10.42 points
- Test Effort: 66.1 (moderate testing effort)

**Success Criteria**:
- [ ] Tokenizer extracted to separate module with <10 avg complexity
- [ ] AST types separated from parsing logic
- [ ] Parser logic uses functional composition for clarity
- [ ] Each module has <200 lines of code
- [ ] Test coverage >80% for pure functions
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract AST Types to Separate Module

**Goal**: Separate data structures (AST types) from parsing logic to clarify responsibilities and enable independent evolution.

**Changes**:
- Create new file `src/cook/execution/expression/ast.rs`
- Move `Expression`, `SortKey`, `SortDirection`, and `NullHandling` enums/structs to `ast.rs`
- Update `mod.rs` to expose `ast` module
- Update imports in `parser.rs` to use `ast::*`

**Testing**:
- `cargo test --lib` - verify no compilation errors
- `cargo clippy` - verify no new warnings

**Success Criteria**:
- [ ] AST types cleanly separated in `ast.rs`
- [ ] Parser imports AST types correctly
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Tokenizer to Separate Module

**Goal**: Isolate lexical analysis (tokenization) from parsing to reduce complexity and enable independent testing of token generation.

**Changes**:
- Create new file `src/cook/execution/expression/tokenizer.rs`
- Move `Token` enum to `tokenizer.rs`
- Extract `tokenize()` method as standalone function `tokenize(expr: &str) -> Result<Vec<Token>>`
- Remove complex character-by-character logic from parser
- Make tokenizer pure and testable

**Testing**:
- Write unit tests for tokenizer:
  - Test literal tokenization (numbers, strings, booleans, null)
  - Test operator tokenization (==, !=, &&, ||, etc.)
  - Test identifier and keyword tokenization
  - Test error cases (unclosed strings, invalid numbers)
- Run `cargo test tokenizer`
- Verify parser still works with extracted tokenizer

**Success Criteria**:
- [ ] Tokenizer in separate module with complete test coverage
- [ ] Parser uses tokenizer as pure function
- [ ] Tokenizer tests achieve >90% coverage
- [ ] Max complexity in tokenizer <10
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Refactor Parser into Smaller Pure Functions

**Goal**: Break down complex parsing methods into smaller, focused, testable functions using functional composition.

**Changes**:
- Extract `find_operators()` into a pure helper function
- Simplify `parse_or()` and `parse_and()` by extracting common logic:
  - Create `parse_binary_operator(tokens: &[Token], op: Token, next_parser: impl Fn(&[Token]) -> Result<Expression>) -> Result<Expression>`
  - Both `parse_or()` and `parse_and()` use this helper
- Extract parenthesis matching logic into pure function `find_matching_paren(tokens: &[Token], start: usize) -> Result<usize>`
- Simplify `parse_comparison()` by extracting operator precedence logic
- Extract `parse_field_path()` logic for array wildcard handling

**Testing**:
- Write unit tests for new helper functions:
  - Test `parse_binary_operator` with different operators
  - Test `find_matching_paren` with nested parentheses
  - Test field path parsing with wildcards
- Run `cargo test parser`
- Verify existing integration tests pass

**Success Criteria**:
- [ ] Parser methods <30 lines each
- [ ] Helper functions are pure and well-tested
- [ ] Cyclomatic complexity reduced by >20 points
- [ ] No function exceeds complexity of 15
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Add Comprehensive Tests for Parser Logic

**Goal**: Achieve >80% test coverage for the parser module by testing core parsing functionality.

**Changes**:
- Create `src/cook/execution/expression/parser_tests.rs` (if not exists)
- Add tests for `parse_filter()`:
  - Simple comparisons (field = value)
  - Logical operators (AND, OR, NOT)
  - Nested expressions with parentheses
  - String functions (contains, starts_with, etc.)
  - Aggregate functions (length, sum, count, etc.)
  - Field paths with wildcards
  - Error cases (empty expressions, mismatched parens)
- Add tests for `parse_sort()`:
  - Single field sorting
  - Multiple field sorting
  - Sort directions (ASC, DESC)
  - Null handling (NULLS FIRST, NULLS LAST)
  - Error cases (invalid syntax)

**Testing**:
- Run `cargo test --lib expression::parser`
- Run `cargo tarpaulin --lib` to measure coverage
- Verify coverage >80% for parser module

**Success Criteria**:
- [ ] Comprehensive test suite covers all parsing scenarios
- [ ] Edge cases and error conditions tested
- [ ] Test coverage >80% for parser module
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Optional - Extract Error Handling Module

**Goal**: Centralize error handling and provide better error messages for parsing failures (optional based on complexity reduction achieved).

**Changes**:
- Create `src/cook/execution/expression/errors.rs`
- Define `ParseError` enum with specific error types:
  - `UnexpectedToken { expected: String, got: Token }`
  - `MismatchedParentheses { position: usize }`
  - `InvalidNumber { value: String }`
  - `EmptyExpression`
  - etc.
- Update parser and tokenizer to use `ParseError` instead of generic `anyhow!`
- Provide better error messages with context

**Testing**:
- Update tests to verify specific error types
- Test error message quality
- Run `cargo test --lib expression`

**Success Criteria**:
- [ ] Custom error types defined
- [ ] Parser provides clear error messages
- [ ] Error tests validate error types
- [ ] All tests pass
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure proper formatting
4. For phases with new tests, verify coverage with `cargo tarpaulin --lib`

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Verify coverage improvement
3. Run debtmap again to measure improvement:
   ```bash
   debtmap analyze
   ```
4. Verify complexity reduction and coverage improvement

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure - check error messages and test output
3. Adjust the plan if needed:
   - Smaller increments if changes too large
   - Different approach if architectural issue
4. Retry with adjusted strategy

## Notes

### Key Insights from Code Analysis

1. **Tokenizer is Embedded**: The `tokenize()` method (lines 170-316) is 146 lines with complex character-by-character logic. This should be extracted first to reduce coupling.

2. **Complex Parsing Methods**:
   - `parse_or()` and `parse_and()` follow similar patterns - good candidates for extraction
   - `parse_comparison()` has high cyclomatic complexity due to operator matching
   - `parse_primary()` handles many token types - could benefit from dispatch pattern

3. **No Tests Currently**: 0% coverage means we need comprehensive tests. Focus on:
   - Pure functions (tokenizer, helper functions)
   - Parser entry points (`parse_filter`, `parse_sort`)
   - Error cases

4. **Public API Surface**: Only 3 public functions (`new()`, `parse_filter()`, `parse_sort()`) - internal refactoring won't break external consumers.

5. **Functional Refactoring Opportunities**:
   - Extract operator precedence logic
   - Use combinators for binary operator parsing
   - Separate token stream manipulation from AST construction

### Potential Gotchas

- **Token Equality**: The `Token` enum uses `PartialEq` for matching - ensure this works correctly when extracting tokenizer
- **Error Context**: Currently using `anyhow!` - may need to preserve error context when extracting
- **Field Path Parsing**: The `parse_field_path()` method handles array wildcards - complex logic that needs careful testing

### Dependencies

- No external dependencies beyond `anyhow` and `serde_json`
- Clean module structure makes extraction straightforward
- No circular dependencies to worry about
