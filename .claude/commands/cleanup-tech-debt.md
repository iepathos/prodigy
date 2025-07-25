# /cleanup-tech-debt

Analyze the Rust codebase for technical debt and perform comprehensive cleanup including code organization improvements, dead code removal, dependency optimization, and structural refactoring. This command systematically identifies and resolves technical debt to improve maintainability, performance, and code quality in Rust projects.

## Variables

SCOPE: $ARGUMENTS (optional - specify scope like "src/agents", "src/mcp", "tests", or "all" for entire codebase)

## Execute

### Phase 1: Technical Debt Analysis

1. **Code Organization Analysis**
   - Scan for misplaced modules and inconsistent module structure
   - Identify files that should be moved to better locations
   - Check for circular dependencies between crates and modules
   - Analyze module cohesion and coupling
   - Review visibility modifiers (pub, pub(crate), pub(super))

2. **Dead Code Detection**
   - Use `cargo +nightly udeps` to find unused dependencies
   - Run with `#![warn(dead_code)]` to identify unused items
   - Locate unused feature flags and conditional compilation blocks
   - Find orphaned test modules and benchmarks
   - Check for commented-out code blocks

3. **Dependency Audit**
   - Review Cargo.toml for unused dependencies
   - Run `cargo audit` for security vulnerabilities
   - Check for duplicate functionality across crates
   - Identify outdated dependencies with `cargo outdated`
   - Find missing dependencies that should be explicit
   - Review feature flags usage and minimize dependencies

4. **Code Quality Issues**
   - Run `cargo clippy -- -W clippy::all -W clippy::pedantic`
   - Scan for overly complex functions (high cyclomatic complexity)
   - Find functions with too many parameters (>5)
   - Identify large files that should be split into modules
   - Check for inconsistent naming conventions (snake_case, CamelCase)
   - Find excessive use of `unwrap()` and `expect()`

5. **Error Handling Patterns**
   - Find inconsistent error handling approaches (Result vs panic)
   - Identify missing error propagation with `?` operator
   - Check for panic usage that should be Result returns
   - Review custom error types and error conversion implementations
   - Verify proper use of `anyhow` vs custom error types

### Phase 2: Cleanup Strategy Planning

1. **Prioritize Issues**
   - High Impact: Security vulnerabilities, performance issues, maintainability blockers
   - Medium Impact: Code organization, redundancy, minor architectural issues
   - Low Impact: Style consistency, documentation gaps, minor optimizations

2. **Create Cleanup Plan**
   - Generate ordered list of cleanup tasks
   - Group related changes for atomic commits
   - Identify breaking changes that need careful handling
   - Plan testing strategy for each change

3. **Risk Assessment**
   - Identify changes that could break existing functionality
   - Plan rollback strategies for risky changes
   - Determine which changes need comprehensive testing
   - Flag changes that require manual review

### Phase 3: Automated Cleanup

1. **Safe Automated Fixes**
   - Run `cargo fmt` to standardize formatting
   - Execute `cargo fix --edition` for edition idioms
   - Apply `cargo clippy --fix` for automatic corrections
   - Remove unused imports with rustfmt
   - Fix simple linting issues automatically

2. **Code Organization**
   - Move misplaced modules to appropriate locations
   - Rename files to follow Rust naming conventions (snake_case)
   - Reorganize module structure following Rust patterns
   - Update mod.rs declarations and use statements
   - Ensure proper module visibility (pub/private)

3. **Dead Code Removal**
   - Remove items marked by `#[warn(dead_code)]`
   - Delete commented-out code blocks
   - Remove empty modules and test files
   - Clean up unused feature-gated code
   - Eliminate redundant trait implementations

4. **Dependency Optimization**
   - Run `cargo update` for compatible updates
   - Remove unused dependencies from Cargo.toml
   - Consolidate duplicate functionality across crates
   - Minimize feature flags to reduce compile time
   - Replace heavy dependencies with lighter alternatives

### Phase 4: Structural Improvements

1. **Function Refactoring**
   - Split large functions into smaller, focused ones
   - Extract common functionality into trait implementations
   - Reduce function parameter counts using builder pattern
   - Convert tuple returns to named structs for clarity
   - Add #[must_use] annotations where appropriate

2. **Module Structure**
   - Reorganize modules for better cohesion
   - Fix circular dependencies between modules
   - Ensure proper separation of concerns (data/logic/IO)
   - Define clear public APIs with minimal surface area
   - Use workspace features for optional functionality

3. **Type System Improvements**
   - Replace stringly-typed APIs with enums
   - Add phantom types for compile-time guarantees
   - Use newtype pattern for domain modeling
   - Implement proper trait bounds on generics
   - Convert runtime checks to compile-time guarantees

4. **Error Handling Standardization**
   - Define custom error types with thiserror
   - Use anyhow for application-level errors
   - Add proper error context with .context()
   - Replace unwrap() with proper error propagation
   - Implement From traits for error conversions

5. **Code Duplication Elimination**
   - Extract common patterns into generic functions
   - Create derive macros for boilerplate code
   - Use trait default implementations
   - Consolidate similar implementations with generics
   - Leverage cargo workspaces for shared code

### Phase 5: Testing and Validation

1. **Pre-cleanup Testing**
   - Run `cargo test --all-features` to establish baseline
   - Execute `cargo test --release` for optimized builds
   - Run `cargo miri test` for undefined behavior detection
   - Execute benchmarks with `cargo bench`
   - Check test coverage with `cargo tarpaulin`

2. **Post-cleanup Validation**
   - Run full test suite after each major change
   - Verify no new unsafe code without justification
   - Use `cargo +nightly build -Z timings` to check build time
   - Validate with `cargo check --all-targets`
   - Run property-based tests if using proptest/quickcheck

3. **Memory and Thread Safety**
   - Run tests under valgrind for memory leaks
   - Use `cargo +nightly miri` for UB detection
   - Check with ThreadSanitizer via `RUSTFLAGS="-Z sanitizer=thread"`
   - Verify no data races with concurrent tests
   - Profile memory usage with heaptrack or similar

### Phase 6: Documentation and Reporting

1. **Update Documentation**
   - Update module documentation for moved files
   - Fix outdated rustdoc comments and examples
   - Update README if crate structure changed
   - Add missing /// or //! documentation comments
   - Generate docs with `cargo doc --no-deps --open`

2. **Generate Cleanup Report**
   - Summary of changes made
   - List of removed dead code and dependencies
   - Performance improvements (compile time & runtime)
   - Memory usage reduction statistics
   - Remaining technical debt items
   - Security vulnerabilities fixed

3. **Commit Changes**
   - Create atomic commits for each cleanup category
   - Use conventional commit format (fix:, refactor:, perf:)
   - Include benchmark results in commit messages
   - Run `cargo fmt` before each commit
   - Add Claude Code attribution

## Example Usage

```
/cleanup-tech-debt
/cleanup-tech-debt "src/agents"
/cleanup-tech-debt "src/mcp"
/cleanup-tech-debt "tests"
/cleanup-tech-debt "all"
```

## Cleanup Categories

### Code Organization
- Move modules to appropriate locations
- Rename files following snake_case convention
- Fix module visibility and re-exports
- Eliminate circular dependencies between crates

### Dead Code Removal
- Remove unused functions, structs, and traits
- Delete commented-out code blocks
- Remove empty modules and test files
- Clean up unused macro definitions
- Eliminate redundant implementations

### Dependency Management
- Remove unused crate dependencies
- Update outdated dependencies safely
- Minimize feature flags usage
- Replace heavy dependencies with lighter ones
- Audit and fix security vulnerabilities

### Structural Improvements
- Refactor complex functions into smaller units
- Extract common patterns into traits
- Improve error handling with Result types
- Use strong typing instead of primitives
- Apply SOLID principles to module design

### Performance Optimizations
- Replace inefficient algorithms
- Optimize memory allocations
- Use zero-copy operations where possible
- Improve async/await patterns
- Fix unnecessary cloning and allocations

## Safety Measures

1. **Backup Strategy**
   - Create backup branches before major changes
   - Commit frequently with descriptive messages
   - Test after each significant change
   - Maintain rollback capability

2. **Testing Requirements**
   - All tests must pass before and after cleanup
   - No new race conditions introduced
   - Performance benchmarks maintained
   - Critical functionality verified

3. **Review Process**
   - Generate detailed change summary
   - Highlight breaking changes
   - Document removed functionality
   - Provide migration guidance if needed

## Integration with Existing Commands

- Use `/commit-changes` for individual cleanup commits
- Create specs with `/create-spec` for major refactoring
- Run `/debug` if issues arise during cleanup
- Use project's Makefile or justfile for validation
- Run `/test` to verify changes don't break functionality

## Quality Standards

The cleanup process must:
- Maintain all existing functionality
- Follow Rust idioms and project conventions
- Leverage Rust's type system for safety
- Not introduce new bugs or regressions
- Pass all clippy lints (with justification for allows)
- Include comprehensive testing and benchmarks
- Provide clear documentation of changes
- Maintain or improve performance characteristics

## Error Handling

- If tests fail: Stop cleanup and report which tests failed
- If build fails: Rollback changes and report compilation errors
- If dependencies conflict: Use cargo tree to analyze and resolve
- If unsafe code detected: Require justification or safe alternative
- If performance regression: Analyze with cargo bench and flamegraph
- If breaking changes needed: Create separate spec for major refactoring
- If miri detects UB: Fix immediately or rollback changes

## Output Format

The command will provide:
1. **Analysis Report**: Summary of technical debt found with severity levels
2. **Cleanup Plan**: Ordered list of changes to be made with risk assessment
3. **Progress Updates**: Real-time updates during cleanup with cargo commands
4. **Performance Metrics**: Before/after comparison of compile time and binary size
5. **Final Summary**: Complete report of changes made with statistics
6. **Recommendations**: Remaining debt and future improvements

## Rust-Specific Tools and Commands

### Analysis Tools
- `cargo clippy -- -W clippy::all -W clippy::pedantic` - Comprehensive linting
- `cargo +nightly udeps` - Find unused dependencies
- `cargo audit` - Security vulnerability scanning
- `cargo outdated` - Check for outdated dependencies
- `cargo tree` - Analyze dependency graph
- `cargo bloat` - Analyze binary size contributors

### Cleanup Tools
- `cargo fmt` - Format code according to rustfmt.toml
- `cargo fix` - Apply compiler suggestions
- `cargo clippy --fix` - Auto-fix clippy warnings
- `cargo machete` - Remove unused dependencies

### Validation Tools
- `cargo test --all-features` - Run all tests
- `cargo miri test` - Detect undefined behavior
- `cargo bench` - Run performance benchmarks
- `cargo tarpaulin` - Measure test coverage
- `cargo +nightly build -Z timings` - Profile build times