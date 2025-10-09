# Implementation Plan: Refactor execute_with_streaming Function

## Problem Summary

**Location**: ./src/cook/execution/claude.rs:ClaudeExecutorImpl::execute_with_streaming:210
**Priority Score**: 31.075
**Debt Type**: ComplexityHotspot (Cognitive: 41, Cyclomatic: 17)
**Current Metrics**:
- Lines of Code: 141
- Cyclomatic Complexity: 17
- Cognitive Complexity: 41
- Nesting Depth: 4
- Function Role: PureLogic (but contains I/O)

**Issue**: This function has high complexity (cyclomatic: 17, cognitive: 41) and mixes multiple concerns. It needs functional decomposition through extracting pure functions and separating I/O from business logic.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 8.5 points
- Risk Reduction: 10.87625 points
- Coverage Improvement: 0.0 (focus on refactoring)

**Success Criteria**:
- [ ] Extract at least 4 pure functions for configuration, decision logic, and formatting
- [ ] Reduce cyclomatic complexity from 17 to ≤8
- [ ] Reduce cognitive complexity from 41 to ≤20
- [ ] Reduce nesting depth from 4 to ≤2
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting (cargo fmt)

## Implementation Phases

### Phase 1: Extract Configuration Logic

**Goal**: Extract pure functions for parsing and validating configuration from environment variables

**Changes**:
- Extract `parse_timeout_from_env(env_vars: &HashMap<String, String>) -> Option<u64>`
  - Pure function that parses PRODIGY_COMMAND_TIMEOUT
  - Returns None for invalid values
  - Testable in isolation

- Extract `should_print_to_console(env_vars: &HashMap<String, String>, verbosity: u8) -> bool`
  - Pure predicate that determines console output setting
  - Checks PRODIGY_CLAUDE_CONSOLE_OUTPUT env var first
  - Falls back to verbosity >= 1
  - Testable in isolation

**Testing**:
- Add unit tests for `parse_timeout_from_env`:
  - Valid timeout string → Some(timeout)
  - Invalid timeout string → None
  - Missing timeout → None
- Add unit tests for `should_print_to_console`:
  - Env var "true" → true (regardless of verbosity)
  - Env var "false" → false (regardless of verbosity)
  - No env var, verbosity >= 1 → true
  - No env var, verbosity < 1 → false

**Success Criteria**:
- [ ] Two new pure functions extracted and tested
- [ ] All existing tests pass
- [ ] cargo clippy passes
- [ ] cargo fmt applied
- [ ] Ready to commit

### Phase 2: Extract Stream Processor Factory Logic

**Goal**: Extract the complex handler creation logic into a pure factory function

**Changes**:
- Create new module `src/cook/execution/claude_stream_factory.rs`
- Extract `create_stream_processor(event_logger: Option<Arc<EventLogger>>, agent_id: String, print_to_console: bool) -> Box<dyn StreamProcessor>`
  - Encapsulates the handler creation logic (currently lines 263-276)
  - Pure logic (takes inputs, returns processor)
  - Separated from execution context

- Update `execute_with_streaming` to use the factory:
  ```rust
  let processor = create_stream_processor(
      self.event_logger.clone(),
      "agent-default".to_string(),
      print_to_console,
  );
  ```

**Testing**:
- Add unit tests for `create_stream_processor`:
  - With event logger → EventLoggingClaudeHandler processor
  - Without event logger → ConsoleClaudeHandler processor
  - Both with print_to_console true/false

**Success Criteria**:
- [ ] Stream processor factory extracted
- [ ] New module created with tests
- [ ] execute_with_streaming simplified
- [ ] All existing tests pass
- [ ] cargo clippy passes
- [ ] Ready to commit

### Phase 3: Extract Error Formatting Logic

**Goal**: Extract the complex error message formatting into pure functions

**Changes**:
- Extract `format_execution_error_details(result: &ExecutionResult) -> String`
  - Pure function that creates error detail string
  - Uses iterator chain to prioritize stderr → stdout → exit code
  - Currently implemented as nested if-else (lines 315-321)

- Extract `format_error_with_log_location(command: &str, error_details: &str, log_location: Option<&Path>) -> String`
  - Pure function that formats final error message
  - Includes JSON log location if available
  - Currently implemented as if-let-else (lines 326-334)

**Testing**:
- Add unit tests for `format_execution_error_details`:
  - With stderr → returns stderr message
  - With stdout only → returns stdout message
  - With neither → returns exit code message

- Add unit tests for `format_error_with_log_location`:
  - With log location → includes log path
  - Without log location → basic error message

**Success Criteria**:
- [ ] Two error formatting functions extracted and tested
- [ ] Error handling logic simplified to use pure functions
- [ ] All existing tests pass
- [ ] cargo clippy passes
- [ ] Ready to commit

### Phase 4: Extract Command Args Builder

**Goal**: Extract the command args construction into a pure function

**Changes**:
- Extract `build_streaming_claude_args(command: &str) -> Vec<String>`
  - Pure function that constructs the args vector
  - Currently hardcoded array (lines 238-244)
  - Returns owned Vec for flexibility

- Update `execute_with_streaming` to use the builder:
  ```rust
  let args = build_streaming_claude_args(command);
  ```

**Testing**:
- Add unit tests for `build_streaming_claude_args`:
  - Verify correct args order
  - Verify all required flags present
  - Test with different command strings

**Success Criteria**:
- [ ] Args builder function extracted and tested
- [ ] Function clearly documents required flags
- [ ] All existing tests pass
- [ ] cargo clippy passes
- [ ] Ready to commit

### Phase 5: Simplify Main Function Flow

**Goal**: Use the extracted pure functions to simplify execute_with_streaming

**Changes**:
- Refactor `execute_with_streaming` to use all extracted functions:
  ```rust
  async fn execute_with_streaming(...) -> Result<ExecutionResult> {
      let execution_start = SystemTime::now();

      // Use pure functions for configuration
      let mut context = build_execution_context(project_path, env_vars.clone());
      if let Some(timeout) = parse_timeout_from_env(&env_vars) {
          context.timeout_seconds = Some(timeout);
      }

      let args = build_streaming_claude_args(command);
      let print_to_console = should_print_to_console(&env_vars, self.verbosity);
      let processor = create_stream_processor(
          self.event_logger.clone(),
          "agent-default".to_string(),
          print_to_console,
      );

      // Execute (I/O boundary)
      let result = self.runner.run_with_streaming("claude", &args, &context, processor).await;

      // Process result using pure functions
      handle_streaming_result(result, command, project_path, execution_start, self.verbosity).await
  }
  ```

- Extract `build_execution_context` helper for context setup
- Extract `handle_streaming_result` for result processing
- Reduce nesting and complexity

**Testing**:
- Run existing integration tests
- Verify all error paths still work
- Test JSON log detection still functions

**Success Criteria**:
- [ ] execute_with_streaming reduced to <50 lines
- [ ] Cyclomatic complexity ≤8
- [ ] Cognitive complexity ≤20
- [ ] Nesting depth ≤2
- [ ] All existing tests pass
- [ ] cargo clippy passes
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Write unit tests for extracted pure functions FIRST
2. Run `cargo test --lib` to verify existing tests pass
3. Run `cargo clippy -- -D warnings` to check for issues
4. Run `cargo fmt` to format code
5. Commit the phase with descriptive message

**Final verification**:
1. `cargo test --all` - All tests pass
2. `cargo clippy -- -D warnings` - No warnings
3. `cargo fmt --check` - Code properly formatted
4. Verify complexity reduction with metrics:
   - Use `cargo-complexity` or review manually
   - Confirm cyclomatic complexity ≤8
   - Confirm nesting depth ≤2

## Rollback Plan

If a phase fails:
1. Review test failures to understand the issue
2. Check if the extraction broke any assumptions
3. Revert with `git reset --hard HEAD~1`
4. Adjust the approach:
   - Smaller extraction scope
   - Different function boundaries
   - Additional helper functions
5. Retry the phase

If multiple phases fail:
1. Return to last successful phase
2. Re-evaluate the refactoring strategy
3. Consider alternative approaches:
   - Builder pattern for configuration
   - Strategy pattern for handler selection
   - Result type for error formatting

## Notes

**Key Principles**:
- Extract pure functions first (no side effects)
- Keep I/O at the boundaries (command execution)
- Use function composition to build complex behavior
- Each function should have a single, clear purpose

**Avoid**:
- Moving code without reducing complexity
- Creating helper functions that are just code extraction
- Breaking legitimate patterns (e.g., Result handling)
- Adding abstractions that don't clarify intent

**Dependencies**:
- No external crate changes needed
- All refactoring uses existing types and patterns
- Maintains backward compatibility with existing tests

**Expected Outcome**:
- 6-8 new pure functions
- Main function reduced from 141 lines to ~40-50 lines
- Cyclomatic complexity reduced from 17 to ~6-8
- Cognitive complexity reduced from 41 to ~15-20
- All logic testable in isolation
- Clearer separation of concerns
