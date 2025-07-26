# /mmm-lint

Automatically detect and fix linting issues in the Rust codebase using cargo clippy and rustfmt. This command ensures code quality and consistency by addressing compiler warnings, clippy lints, and formatting issues.

## Variables

SCOPE: $ARGUMENTS (optional - specify scope like "src/parser", "tests", or omit for entire codebase)

## Execute

### Phase 1: Pre-lint Analysis

1. **Current Status Check**
   - Run `cargo check` to identify compilation errors
   - Execute `cargo clippy --all-targets --all-features -- -D warnings` to get current lint status
   - Run `cargo fmt --check` to see formatting issues
   - Generate baseline report of current issues

2. **Categorize Issues**
   - **Critical**: Compilation errors that prevent building
   - **High Priority**: Clippy warnings that could cause bugs
   - **Medium Priority**: Style and performance lints
   - **Low Priority**: Formatting and minor style issues

### Phase 2: Automated Fixes

1. **Formatting Issues**
   - Run `cargo fmt` to fix all formatting issues
   - Verify formatting changes don't break compilation
   - Commit formatting fixes separately if requested

2. **Auto-fixable Clippy Issues**
   - Run `cargo clippy --fix --allow-dirty --allow-staged`
   - Apply automatic fixes for safe suggestions
   - Review changes to ensure they're correct

3. **Compilation Errors**
   - Address any remaining compilation errors
   - Fix import issues and missing dependencies
   - Resolve type errors and syntax issues

### Phase 3: Manual Lint Resolution

1. **Complex Clippy Warnings**
   - Review warnings that require manual intervention
   - Fix performance-related issues (e.g., unnecessary clones)
   - Address logic issues flagged by clippy
   - Improve error handling patterns

2. **Code Quality Issues**
   - Fix unused imports and variables
   - Remove dead code flagged by compiler
   - Address deprecated API usage
   - Improve variable naming and documentation

3. **Security and Safety Issues**
   - Address unsafe code warnings
   - Fix potential panic conditions
   - Resolve security-related clippy lints
   - Improve input validation

### Phase 4: Project-Specific Fixes

1. **Rust Idioms**
   - Use `?` operator instead of manual error handling
   - Replace manual iteration with iterator methods
   - Use `match` instead of nested `if let` where appropriate
   - Apply RAII patterns for resource management

2. **Performance Optimizations**
   - Fix unnecessary allocations
   - Use string slices instead of owned strings where possible
   - Optimize iterator chains
   - Address clone-heavy code patterns

3. **Error Handling Improvements**
   - Replace `unwrap()` with proper error handling
   - Add context to error messages using `anyhow`
   - Implement proper error propagation
   - Remove `panic` from production code paths

### Phase 5: Testing and Validation

1. **Compilation Check**
   - Ensure `cargo check` passes without warnings
   - Verify `cargo clippy` produces no warnings
   - Confirm `cargo fmt --check` shows no formatting issues
   - Test that `cargo build` succeeds

2. **Test Suite Validation**
   - Run `cargo test` to ensure all tests pass
   - Execute `cargo test -- --nocapture` for detailed output
   - Run integration tests if present
   - Verify no regressions introduced

3. **Benchmark Validation**
   - Run benchmarks if available with `cargo bench`
   - Ensure performance hasn't regressed
   - Validate memory usage patterns
   - Check for new performance improvements

### Phase 6: Documentation and Reporting

1. **Generate Lint Report**
   - Summary of issues found and fixed
   - List of manual interventions made
   - Performance improvements achieved
   - Remaining issues requiring attention

2. **Update Documentation**
   - Fix documentation warnings
   - Update code examples in rustdoc
   - Ensure public APIs are properly documented
   - Add missing doc comments

## Example Usage

```
/mmm-lint
/mmm-lint "src/parser"
/mmm-lint "tests"
/mmm-lint "src/parser/inventory.rs"
```

## Lint Categories Addressed

### Clippy Warnings
- **Correctness**: Logic errors and potential bugs
- **Style**: Code style and readability improvements
- **Complexity**: Overly complex code patterns
- **Performance**: Inefficient code patterns
- **Pedantic**: Extra pedantic lints for code quality

### Compiler Warnings
- Unused imports and variables
- Dead code detection
- Deprecated API usage
- Type inference improvements
- Pattern matching exhaustiveness

### Formatting Issues
- Consistent indentation and spacing
- Line length adherence (100 characters)
- Import organization and grouping
- Trailing whitespace removal
- Consistent bracket and brace style

## Safety Measures

1. **Backup and Recovery**
   - Check git status before making changes
   - Ensure working directory is clean
   - Create stash if uncommitted changes exist
   - Provide rollback instructions if needed

2. **Incremental Changes**
   - Apply fixes in logical groups
   - Test after each category of fixes
   - Commit formatting separately from logic changes
   - Maintain bisectability of changes

3. **Verification Steps**
   - All tests must pass after fixes
   - No new warnings introduced
   - Compilation must succeed cleanly
   - Performance benchmarks maintained

## Integration with Development Workflow

### Pre-commit Integration
- Run lint fixes before committing changes
- Ensure CI pipeline requirements are met
- Maintain consistent code quality standards
- Reduce review feedback on style issues

### CI/CD Compatibility
- Ensure fixes align with CI lint requirements
- Address any CI-specific clippy configurations
- Maintain compatibility with automated checks
- Support for custom lint rules if configured

## Error Handling

### Fix Failures
- If auto-fixes break compilation: Rollback and report issues
- If tests fail after fixes: Identify problematic changes and revert
- If clippy fixes introduce bugs: Manual review and correction
- If formatting breaks code: Investigate and fix manually

### Conflict Resolution
- Handle merge conflicts in auto-generated fixes
- Resolve competing lint suggestions
- Address contradictory clippy recommendations
- Manage formatter vs. clippy conflicts

## Advanced Features

### Custom Lint Configuration
- Respect project-specific clippy.toml settings
- Handle allow/deny lint attributes in code
- Support for custom lint rule sets
- Integration with project coding standards

### Performance Analysis
- Identify and fix performance-related lints
- Optimize hot code paths flagged by clippy
- Address memory usage patterns
- Improve algorithmic complexity where possible

## Output Format

The command provides:
1. **Initial Analysis**: Current lint status and issue categorization
2. **Fix Progress**: Real-time updates on fixes being applied
3. **Test Results**: Compilation and test outcomes after fixes
4. **Final Report**: Complete summary of changes made
5. **Recommendations**: Suggestions for ongoing code quality improvements

## Rust-Specific Considerations

### Ownership and Borrowing
- Fix unnecessary clones and allocations
- Optimize lifetime annotations
- Improve borrow checker compliance
- Address move vs. borrow decisions

### Error Handling Patterns
- Standardize on `Result<T, E>` patterns
- Improve error context and propagation
- Remove unwrap/expect from production code
- Implement proper error handling strategies

### Async Code Quality
- Fix async/await pattern issues
- Address tokio-specific lints
- Optimize async resource management
- Improve concurrent code patterns

### Dependencies and Features
- Address dependency-related warnings
- Fix feature flag compilation issues
- Optimize conditional compilation
- Manage crate feature interactions