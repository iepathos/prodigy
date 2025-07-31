# Iteration 1753959906: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

## Debt Items to Address

### 1. Excessive Comment Markers in debt.rs Test File
**Impact Score**: 8/10
**Effort Score**: 2/10
**Category**: Fixme
**File**: src/context/debt.rs
**Priority**: High

#### Current State:
```rust
// Lines 508-515 contain test comments:
// TODO: Refactor this function
// FIXME: This is a hack
let x = 42; // HACK: Magic number
// TODO(high): Critical security issue
// XXX: This needs review
```

#### Required Changes:
These comment markers appear to be test fixtures for the debt detection system, not actual technical debt. They should be properly scoped within test functions or test data strings to avoid false positives in debt analysis.

#### Implementation Steps:
- Move test comment examples into string literals within test functions
- Ensure debt detection tests use proper test data isolation
- Validate that real technical debt comments are distinguished from test fixtures

### 2. High Complexity Functions
**Complexity Score**: 16-17
**Change Frequency**: High
**Risk Level**: High
**Files**: 
- tests/command_parsing_tests.rs::parse_command_string (complexity: 17)
- tests/cook_iteration_tests.rs::test_focus_applied_every_iteration (complexity: 16)

#### Refactoring Plan:
- Extract command parsing logic into smaller helper functions
- Split test setup from test assertions in complex test functions
- Consider using test builders or fixtures to reduce test complexity

### 3. Clippy Pedantic Warnings
**Impact Score**: 5/10
**Effort Score**: 3/10
**Category**: Code Quality
**Priority**: Medium

#### Documentation Issues:
```rust
// Missing backticks in documentation:
- src/abstractions/git.rs:43: GitOperations → `GitOperations`
- src/abstractions/git.rs:50: RealGitOperations → `RealGitOperations`
- src/abstractions/git.rs:144: GitOperations → `GitOperations`
- src/abstractions/git.rs:148: is_git_repo → `is_git_repo`
- src/abstractions/git.rs:157: MockGitOperations → `MockGitOperations`
```

#### Missing Attributes:
```rust
// Add #[must_use] attributes:
- src/abstractions/git.rs:51: pub fn new() -> Self
- src/abstractions/git.rs:158: pub fn new() -> Self
- src/config/command.rs:39: pub fn is_variable() -> bool
- src/config/command.rs:44: pub fn resolve() -> String
```

#### Code Quality:
```rust
// Inefficient string operations:
- src/abstractions/git.rs:203: Use (*s).to_string() instead of s.to_string()
// Redundant closures:
- src/abstractions/git.rs:203: Replace |s| s.to_string() with std::string::ToString::to_string
```

#### Implementation Steps:
- Add backticks to documentation for proper code formatting
- Add #[must_use] attributes to constructors and getters
- Fix inefficient string operations and redundant closures
- Add missing "# Errors" sections to functions returning Result

### 4. Architectural Violations
**Impact Score**: 6/10
**Effort Score**: 5/10
**Category**: Architecture
**Priority**: Medium

#### God Component Issue:
**Location**: cook module
**Description**: cook has 13 dependencies, consider splitting
**Dependencies**: serde, clap, anyhow, once_cell, tokio, tempfile, chrono, git_ops, retry, tracing, workflow, glob, signal_hook

#### Refactoring Approach:
- Extract workflow management into separate module
- Move retry logic to utility module
- Consider dependency injection for better testability
- Group related functionality into sub-modules

### 5. Test Coverage Gaps
**Impact Score**: 7/10
**Effort Score**: 6/10
**Category**: Testing
**Overall Coverage**: 42.64%
**Priority**: High

#### Critical Areas:
- Overall test coverage is below 50%
- No specific untested functions listed, indicating incomplete coverage analysis
- Type coverage shows unusual value (109.47%) suggesting measurement issues

#### Implementation Steps:
- Run proper coverage analysis with cargo-tarpaulin
- Identify and prioritize untested critical paths
- Add unit tests for core functionality
- Focus on error handling paths and edge cases

### 6. Deprecated Usage Warning
**Impact Score**: 6/10
**Effort Score**: 2/10
**Category**: Deprecated
**File**: src/main.rs:179
**Priority**: Medium

#### Current State:
```rust
// Check if user used the 'improve' alias (deprecated as of v0.3.0)
```

#### Required Changes:
- Add proper deprecation warnings when 'improve' alias is used
- Document migration path in help text
- Consider removing in next major version

## Dependency Cleanup

No unused dependencies detected. All dependencies in Cargo.toml appear to be actively used.

## Code Organization Changes

### Files to Refactor:
- Split cook module (13 dependencies) into smaller, focused modules
- Extract test helper functions from complex test files

### Modules to Restructure:
- Create dedicated test utilities module for command parsing tests
- Extract workflow execution from cook module

## Success Criteria
- [x] All debt items with impact >= 7 identified
- [ ] Clippy pedantic warnings resolved
- [ ] Documentation properly formatted with backticks
- [ ] #[must_use] attributes added where appropriate
- [ ] Test coverage improved above 50%
- [ ] Complex functions refactored (cyclomatic complexity < 10)
- [ ] All files compile without warnings
- [ ] Tests pass with same or improved coverage
- [ ] Performance benchmarks maintained or improved