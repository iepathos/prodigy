# /mmm-coverage

Analyze test coverage gaps using MMM context data and generate a targeted specification for implementing comprehensive test coverage improvements.

## Variables

SCOPE: $ARGUMENTS (optional - specify scope like "src/core", "src/context", or "all")
MMM_CONTEXT_AVAILABLE: Environment variable indicating .mmm context availability
MMM_CONTEXT_DIR: Path to .mmm/context/ directory with analysis data
MMM_FOCUS: Optional focus directive (e.g., "security", "api", "core")

## Execute

### Phase 1: Load MMM Context Data

**REQUIRED**: Check `MMM_CONTEXT_AVAILABLE=true` and load context files:

1. **Primary Context Sources**
   - `.mmm/context/test_coverage.json` → `untested_functions`, `critical_paths`, `file_coverage`
   - `.mmm/context/technical_debt.json` → high-impact functions needing tests
   - `.mmm/context/architecture.json` → public interfaces requiring coverage
   - `.mmm/metrics/current.json` → current coverage percentage and targets

2. **Coverage Gap Prioritization**
   - **Critical**: `untested_functions` with `criticality: "High"` from test_coverage.json
   - **Public APIs**: Functions in `architecture.json` components without tests
   - **Focus Areas**: If `MMM_FOCUS` set, boost priority for matching functions
   - **Hotspots**: Functions in `technical_debt.json` hotspots with no coverage

### Phase 2: Fallback Analysis (Only if MMM context unavailable)

**If `MMM_CONTEXT_AVAILABLE` ≠ true**:

1. **Quick Coverage Check**
   ```bash
   cargo tarpaulin --skip-clean --out Json --output-dir target/coverage --timeout 120
   ```

2. **Basic Gap Identification**
   - Files with <70% line coverage
   - Public functions without any test coverage
   - Error Result types without error path tests

**Important**: MMM context provides much better analysis. Encourage context generation.

### Phase 3: Generate Coverage Improvement Specification

**CRITICAL**: Create actionable spec file for `mmm-implement-spec` to consume.

1. **Spec File Location**
   - Directory: `specs/temp/`
   - Filename: `iteration-{unix_timestamp}-coverage-improvements.md`
   - Must match pattern: `*-coverage-improvements.md`

2. **Extract Specific Functions from Context**
   - Parse `untested_functions` array from test_coverage.json
   - Include exact file paths, function names, and line numbers
   - Prioritize by criticality: High → Medium → Low
   - Filter by MMM_FOCUS if specified

### Phase 4: Spec Content Generation

**Create comprehensive test implementation instructions**:

1. **Function-Level Test Plans**
   - For each `untested_function`: exact file path, function signature, test examples
   - Include both success and error test cases
   - Use modern Rust testing patterns: `#[tokio::test]` for async, proper assertions

2. **Integration Test Requirements**
   - Component interfaces from architecture.json that need integration tests
   - Cross-module interactions requiring end-to-end validation

3. **Test Organization**
   - Follow project conventions: inline tests for units, `tests/` for integration
   - Use existing test utilities and helpers
   - Include validation commands: `cargo test`, `cargo tarpaulin`

### Phase 5: Modern Rust Testing Patterns

**Include these patterns in generated specs**:

1. **Async Function Testing**
   ```rust
   #[tokio::test]
   async fn test_async_function() {
       let result = async_function().await;
       assert!(result.is_ok());
   }
   ```

2. **Error Path Testing**
   ```rust
   #[test]
   fn test_function_error_cases() {
       let result = function_with_errors(invalid_input);
       assert!(result.is_err());
       assert_eq!(result.unwrap_err().to_string(), "Expected error");
   }
   ```

3. **Integration Test Structure**
   ```rust
   // tests/integration_test.rs
   use mmm::*;
   
   #[tokio::test]
   async fn test_component_integration() {
       // Test component interactions
   }
   ```

### Phase 6: Generate and Commit Specification

**REQUIRED OUTPUT**: Create spec file at `specs/temp/iteration-{timestamp}-coverage-improvements.md`

1. **Spec Template Structure**
   ```markdown
   # Coverage Improvements - Iteration {timestamp}
   
   ## Overview
   Test coverage improvements based on MMM context analysis.
   Current coverage: {overall_coverage}% → Target: {target_coverage}%
   {If MMM_FOCUS: Focus area: {focus}}
   
   ## Critical Functions Needing Tests
   
   ### Function: `{function_name}` in {file_path}:{line_number}
   **Criticality**: {High|Medium|Low}
   **Current Status**: No test coverage
   
   #### Add these tests to {file_path}:
   ```rust
   #[cfg(test)] 
   mod tests {
       use super::*;
       
       #[test] // or #[tokio::test] for async
       fn test_{function_name}_success() {
           // Test normal operation
           {concrete_test_example}
       }
       
       #[test]
       fn test_{function_name}_error_cases() {
           // Test error conditions  
           {error_test_example}
       }
   }
   ```
   
   ## Integration Tests Needed
   
   ### Component: {component_name}
   **File**: tests/{component_name}_integration.rs
   ```rust
   use mmm::*;
   
   #[tokio::test]
   async fn test_{component_name}_integration() {
       {integration_test_example}
   }
   ```
   
   ## Implementation Checklist
   - [ ] Add unit tests for {count} critical functions
   - [ ] Create {count} integration test files
   - [ ] Verify tests pass: `cargo test`
   - [ ] Check coverage improves: `cargo tarpaulin`
   - [ ] Follow project conventions from .mmm/context/conventions.json
   ```

2. **Context Data Integration**
   - Extract exact function names, file paths, line numbers from `untested_functions`
   - Use `file_coverage` data for current coverage percentages
   - Reference `conventions.json` for project testing patterns
   - Include concrete test examples based on function signatures

3. **Validation Requirements**
   - Each function must have both success and error test cases
   - Include exact commands to verify improvements
   - Reference existing test utilities from the codebase

4. **Git Commit (Automation Mode)**
   ```bash
   mkdir -p specs/temp
   # Create spec file
   git add specs/temp/iteration-{timestamp}-coverage-improvements.md
   git commit -m "test: generate coverage improvement spec for iteration-{timestamp}"
   ```
   
   **Skip commit if**: No critical coverage gaps found (overall coverage >85%)

## Success Criteria & Output

**Create spec only if**: Critical coverage gaps found (>5 untested critical functions OR overall coverage <75%)

**Console Output**:
```
✓ MMM context loaded - {count} untested critical functions found
✓ Generated spec: iteration-{timestamp}-coverage-improvements.md  
✓ Spec committed for mmm-implement-spec processing
```

**Or if no gaps**:
```
✓ Coverage analysis complete - targets met ({coverage}%)
✓ No critical gaps requiring immediate attention
```

**Spec File Output**: `specs/temp/iteration-{timestamp}-coverage-improvements.md`

## Coverage Targets

**Priority Levels**:
- **Critical**: Functions with `criticality: "High"` in untested_functions
- **Public APIs**: Architecture components without adequate test coverage  
- **Focus Areas**: Functions matching MMM_FOCUS directive
- **Error Paths**: Result-returning functions without error case tests

**Target Thresholds**:
- Overall project coverage: Current + 10% (minimum 75%)
- Critical functions: 100% coverage
- Public APIs: >90% coverage
- New code: 100% coverage requirement

## Command Integration

**Workflow Chain**: `mmm-coverage` → generates spec → `mmm-implement-spec` → `mmm-lint`

**Context Dependencies**: 
- Requires `.mmm/context/test_coverage.json` with `untested_functions` array
- Uses `.mmm/context/architecture.json` for component interfaces
- Follows patterns from `.mmm/context/conventions.json`
- References current metrics from `.mmm/metrics/current.json`

**Output Contract**: Spec file matching `*-coverage-improvements.md` pattern for workflow consumption