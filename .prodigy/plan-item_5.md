# Implementation Plan: Refactor ClaudeJsonProcessor::process_line Using Functional Patterns

## Problem Summary

**Location**: ./src/subprocess/streaming/claude_processor.rs:ClaudeJsonProcessor::process_line:68
**Priority Score**: 39.096824583655184
**Debt Type**: ComplexityHotspot (cognitive: 120, cyclomatic: 17)
**Current Metrics**:
- Lines of Code: 103
- Function Length: 103 lines
- Cyclomatic Complexity: 17
- Cognitive Complexity: 120
- Nesting Depth: 3

**Issue**: Apply functional patterns: 4 pure functions with Iterator chains. The function has moderate complexity (17 cyclomatic) and needs functional decomposition.

**Current Problems**:
1. Large function (103 lines) doing multiple responsibilities
2. Deep nesting with match expressions inside match expressions
3. Repeated pattern of extracting JSON fields with `.and_then().unwrap_or()`
4. Multiple side effects (buffer mutation, printing, handler calls) mixed with parsing logic
5. No separation between pure parsing logic and I/O operations

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 8.5 (from 17 to ~8-9 cyclomatic complexity)
- Coverage Improvement: 0.0 (maintain existing coverage)
- Risk Reduction: 13.68

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 17 to ≤10
- [ ] Pure extraction functions for JSON field parsing
- [ ] Separate functions for event type handling
- [ ] Main function reduced to <30 lines (coordination only)
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Pure JSON Field Extraction Functions

**Goal**: Create pure functions for common JSON field extraction patterns to eliminate repetitive `.and_then().unwrap_or()` code.

**Changes**:
- Create module-level pure functions for field extraction:
  - `extract_string_field(json: &Value, field: &str, default: &str) -> String`
  - `extract_u64_field(json: &Value, field: &str, default: u64) -> u64`
  - `extract_string_array(json: &Value, field: &str) -> Vec<String>`
- These are pure functions with no side effects
- Replace all inline field extraction with these functions
- Reduce cognitive load of reading nested `.and_then()` chains

**Testing**:
- Add unit tests for each extraction function
- Run existing tests: `cargo test --lib streaming`
- Verify no behavior change in `test_claude_json_processor`

**Success Criteria**:
- [ ] 3 pure extraction functions added
- [ ] All field extractions use these functions
- [ ] Unit tests added for extraction functions
- [ ] All existing tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 2: Extract Event Parsing into Separate Functions

**Goal**: Break down the large match expression into separate pure functions for each event type.

**Changes**:
- Create pure functions for each event handler:
  - `parse_tool_use(json: &Value) -> Option<(String, String, Value)>` - returns (tool_name, tool_id, parameters)
  - `parse_token_usage(json: &Value) -> Option<(u64, u64, u64)>` - returns (input, output, cache)
  - `parse_message(json: &Value) -> Option<(String, String)>` - returns (content, message_type)
  - `parse_session_start(json: &Value) -> Option<(String, String, Vec<String>)>` - returns (session_id, model, tools)
- These functions return `Option` to handle missing/invalid data gracefully
- Main function calls these parsers and uses results with `?` or `if let Some`
- Reduces nesting depth from 3 to 2

**Testing**:
- Add unit tests for each parsing function with valid and invalid inputs
- Run: `cargo test --lib streaming`
- Verify `test_claude_json_processor` still passes

**Success Criteria**:
- [ ] 4 pure parsing functions created
- [ ] Each parser has unit tests (valid + invalid cases)
- [ ] Main function uses these parsers
- [ ] Nesting depth reduced to 2
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Event Dispatch Logic

**Goal**: Separate the dispatch logic (calling handler methods) from parsing logic.

**Changes**:
- Create an async helper function for event dispatch:
  - `async fn dispatch_event(&self, event_type: &str, json: &Value) -> Result<()>`
- This function uses the parsing functions from Phase 2
- Uses iterator-like pattern matching for cleaner flow
- Main `process_line` becomes:
  1. Handle buffer accumulation
  2. Handle console printing
  3. Skip empty lines
  4. Try parse as JSON
  5. Dispatch to event handler or text handler
- Reduces cyclomatic complexity of main function

**Testing**:
- Run: `cargo test --lib streaming`
- Verify all event types still handled correctly
- Check that `test_claude_json_processor` buffer accumulation works

**Success Criteria**:
- [ ] Event dispatch extracted to separate function
- [ ] Main function focuses on flow control
- [ ] Cyclomatic complexity of main function ≤10
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Refactor Buffer and Console Handling

**Goal**: Extract side effects (buffer accumulation, console printing) into focused helper methods.

**Changes**:
- Create helper methods:
  - `async fn accumulate_line(&self, line: &str)` - handles buffer mutation
  - `fn print_if_enabled(&self, line: &str, source: StreamSource)` - handles console output
  - `fn should_process_line(line: &str) -> bool` - predicate for non-empty lines
- Main function becomes a clean pipeline:
  ```rust
  async fn process_line(&self, line: &str, source: StreamSource) -> Result<()> {
      self.accumulate_line(line).await;
      self.print_if_enabled(line, source);

      if !Self::should_process_line(line) {
          return Ok(());
      }

      match serde_json::from_str::<Value>(line) {
          Ok(json) => self.dispatch_event_from_json(&json).await,
          Err(_) => self.handler.on_text_line(line, source).await,
      }
  }
  ```
- Clear separation of concerns: side effects → validation → processing

**Testing**:
- Run: `cargo test --lib streaming`
- Verify buffer accumulation still works
- Test console output behavior (if testable)
- Confirm `test_claude_json_processor` buffer assertions pass

**Success Criteria**:
- [ ] Side effect helpers extracted
- [ ] Main function is linear pipeline (<30 lines)
- [ ] Clear separation of concerns
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Final Cleanup and Verification

**Goal**: Polish the refactored code and verify all quality metrics are met.

**Changes**:
- Add documentation comments to all new functions
- Ensure all functions follow the module's style
- Review and optimize any remaining complexity
- Run full CI checks
- Verify complexity reduction with debtmap

**Testing**:
- Run full test suite: `cargo test`
- Run clippy: `cargo clippy --all-targets -- -D warnings`
- Run formatting: `cargo fmt --check`
- Run CI: `just ci` (if available)

**Success Criteria**:
- [ ] All functions documented
- [ ] Cyclomatic complexity ≤10 in main function
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Complexity improvement verified

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib streaming` to verify stream processing tests pass
2. Run `cargo clippy` to check for warnings
3. Manually verify buffer accumulation test still works
4. Check that no panics occur with invalid JSON inputs

**Final verification**:
1. `cargo test` - Full test suite
2. `cargo clippy --all-targets -- -D warnings` - Strict linting
3. `cargo fmt --check` - Format verification
4. `just ci` - Full CI checks (if available)
5. Optionally: `debtmap analyze` to verify complexity reduction

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures or errors
3. Identify the issue (logic error, test assumption, etc.)
4. Adjust the implementation approach
5. Retry with corrected approach

## Notes

**Key Functional Programming Principles Applied**:
- **Pure Functions**: All extraction and parsing functions have no side effects
- **Separation of I/O and Logic**: Side effects isolated to helpers, parsing is pure
- **Single Responsibility**: Each function does one thing (extract, parse, dispatch)
- **Composition**: Main function composes smaller functions into a pipeline

**Complexity Reduction Strategy**:
- Phase 1: Eliminate cognitive load of nested `.and_then()` chains
- Phase 2: Reduce branching by extracting event parsers (reduces cyclomatic complexity)
- Phase 3: Flatten dispatch logic (reduces nesting depth)
- Phase 4: Linearize main function into a pipeline (reduces overall complexity)

**Testing Confidence**:
- Existing test `test_claude_json_processor` covers all event types
- Each phase maintains behavior (refactoring only)
- New unit tests added for extracted functions provide regression protection
- No changes to public API or trait implementation

**Risk Mitigation**:
- Each phase is independently valuable and committable
- Incremental changes allow easy rollback if issues arise
- Existing test coverage ensures behavior preservation
- Pure functions are easy to test in isolation
