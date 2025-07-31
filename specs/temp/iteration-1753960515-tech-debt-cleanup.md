# Iteration: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

Total debt items found: 6,235
- High impact items (≥7): 21
- Critical items (≥8): 10
- Duplication issues: 6,153
- Complexity issues: 49
- Comment-based debt: 33

## Debt Items to Address

### 1. High Complexity: execute_structured_command
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: src/cook/workflow.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 33, which severely impacts maintainability and testability.

#### Required Changes:
- Break down the function into smaller, focused subfunctions
- Extract command parsing logic into a separate function
- Extract variable expansion logic into its own function
- Create separate handlers for different command types
- Use pattern matching more effectively to reduce nested conditions

#### Implementation Steps:
- Create `parse_command_type()` function to handle command classification
- Extract `expand_command_variables()` for variable substitution
- Create `execute_simple_command()` and `execute_complex_command()` handlers
- Add comprehensive unit tests for each extracted function
- Ensure error handling is consistent across all functions

### 2. High Complexity: calculate_cyclomatic_complexity
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: src/context/debt.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 24, making it difficult to understand and maintain.

#### Required Changes:
- Split AST traversal logic into separate visitor functions
- Extract complexity calculation rules into a dedicated module
- Use a visitor pattern for traversing the syntax tree
- Reduce nested conditionals through early returns

#### Implementation Steps:
- Create `ComplexityVisitor` trait with methods for each AST node type
- Implement separate calculation methods for different complexity contributors
- Add unit tests for each complexity rule
- Document the complexity calculation algorithm clearly

### 3. High Complexity: handle_worktree_merge
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: src/cook/mod.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 23, with deeply nested merge logic.

#### Required Changes:
- Extract merge conflict resolution into separate functions
- Create a `MergeStrategy` enum with different merge approaches
- Separate validation logic from merge execution
- Improve error handling with custom error types

#### Implementation Steps:
- Define `MergeStrategy` enum with variants (FastForward, ThreeWay, Squash)
- Create `validate_merge_conditions()` function
- Extract `resolve_conflicts()` function for conflict handling
- Add integration tests for different merge scenarios

### 4. Deprecated Code Cleanup
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**File**: Multiple locations
**Priority**: Critical

#### Locations to Address:
- src/context/debt.rs:57 - Deprecated DebtType enum variant
- src/context/debt.rs:179-180 - Deprecated pattern detection logic
- src/main.rs:179 - Deprecated command handler

#### Required Changes:
- Remove deprecated enum variants and update all references
- Replace deprecated pattern detection with modern approach
- Update command handlers to use new API

#### Implementation Steps:
- Search for all usages of deprecated items
- Update calling code to use replacement APIs
- Remove deprecated code blocks
- Update documentation to reflect changes

### 5. Code Duplication: Test Setup Functions
**Impact Score**: 7/10
**Effort Score**: 4/10
**Category**: Duplication
**Files**: tests/cook_iteration_tests.rs, tests/command_parsing_tests.rs
**Priority**: High

#### Current State:
Multiple test files contain duplicated setup and teardown logic.

#### Required Changes:
- Create a shared test utilities module
- Extract common test fixtures and helpers
- Implement builder pattern for test context creation

#### Implementation Steps:
- Create `tests/common/mod.rs` with shared test utilities
- Extract `TestContextBuilder` for configurable test setup
- Move duplicate assertion helpers to common module
- Update all tests to use shared utilities

### 6. Architecture Violation: Cook Module Dependencies
**Impact Score**: 7/10
**Effort Score**: 6/10
**Category**: Architecture
**File**: src/cook/mod.rs
**Priority**: High

#### Current State:
The cook module has 13 dependencies, indicating it's becoming a "god module".

#### Required Changes:
- Split cook module into focused submodules
- Extract workflow orchestration into separate module
- Move git operations to dedicated service
- Separate metric collection from core logic

#### Implementation Steps:
- Create `src/cook/orchestrator.rs` for high-level workflow management
- Extract `src/cook/git_service.rs` for git operations
- Move metrics logic to `src/cook/metrics_collector.rs`
- Update module exports and dependencies
- Add integration tests for refactored modules

### 7. Missing Public Interfaces in Abstractions Module
**Impact Score**: 6/10
**Effort Score**: 3/10
**Category**: Architecture
**File**: src/abstractions/mod.rs
**Priority**: Medium

#### Current State:
The abstractions module has no public interfaces exposed.

#### Required Changes:
- Define clear public API for abstractions
- Export necessary traits and types
- Add documentation for public interfaces

#### Implementation Steps:
- Review which types should be public
- Add proper visibility modifiers
- Document all public APIs with examples
- Add module-level documentation

## Dependency Cleanup

### Dependencies to Audit:
- Run `cargo audit` to check for security vulnerabilities
- Use `cargo outdated` to identify outdated dependencies
- Review feature flags to minimize compile-time dependencies

### Code Organization Changes

### Files to Reorganize:
- Split large test files into focused test modules
- Move test utilities from individual test files to common module
- Reorganize cook module structure as outlined above

## Success Criteria
- [x] All debt items with impact >= 8 addressed
- [x] Complexity hotspots with cyclomatic complexity > 20 refactored
- [x] Deprecated code removed
- [x] Architecture violations resolved
- [ ] All files compile without warnings
- [ ] Tests pass with same or improved coverage
- [ ] Performance benchmarks maintained or improved
- [ ] Clippy lints resolved or explicitly allowed with justification

## Additional Recommendations

1. **Test Coverage**: Current coverage is 41.14%. Focus on:
   - Adding tests for complex functions after refactoring
   - Covering error handling paths
   - Adding integration tests for refactored modules

2. **Documentation**: Doc coverage is only 22.65%. Add:
   - Module-level documentation
   - Examples in doc comments
   - Architecture decision records (ADRs)

3. **Performance**: After refactoring complex functions:
   - Run benchmarks to ensure no regression
   - Profile hot paths
   - Consider using more efficient data structures where applicable

4. **Code Quality Tools**:
   - Set up pre-commit hooks for `cargo fmt` and `cargo clippy`
   - Configure CI to fail on new clippy warnings
   - Consider adding `cargo-deny` for dependency auditing

5. **Future Improvements**:
   - Implement proper error types instead of using `anyhow` everywhere
   - Add property-based tests for complex logic
   - Consider async refactoring for I/O-heavy operations