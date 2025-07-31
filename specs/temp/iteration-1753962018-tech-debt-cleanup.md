# Iteration 1753962018: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

## Debt Items to Address

### 1. High Complexity Function: execute_structured_command
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: /Users/glen/memento-mori/mmm/src/cook/workflow.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 33, which is extremely high and makes the code difficult to understand, test, and maintain.

#### Required Changes:
- Break down the function into smaller, focused sub-functions
- Extract command parsing logic into separate function
- Extract environment setup logic into separate function
- Extract error handling into dedicated error handler functions
- Create a command execution pipeline with clear stages

#### Implementation Steps:
- Identify logical sections within the function that can be extracted
- Create helper functions for: command parsing, environment setup, variable resolution, output capture
- Implement a command execution context struct to manage state
- Add comprehensive unit tests for each extracted function
- Ensure error propagation is consistent throughout

### 2. High Complexity Function: calculate_cyclomatic_complexity
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: /Users/glen/memento-mori/mmm/src/context/debt.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 24, making it error-prone and hard to maintain.

#### Required Changes:
- Split pattern matching logic into separate functions
- Create a visitor pattern for AST traversal
- Extract complexity calculation rules into a configuration struct
- Implement builder pattern for complexity accumulation

#### Implementation Steps:
- Create separate functions for each syntax element's complexity calculation
- Implement a ComplexityVisitor trait for AST traversal
- Move pattern matching into dedicated matcher functions
- Add unit tests for each complexity calculation rule
- Validate results match current implementation before replacing

### 3. Deprecated Code Cleanup
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**File**: /Users/glen/memento-mori/mmm/src/context/debt.rs, /Users/glen/memento-mori/mmm/src/main.rs
**Priority**: High

#### Current State:
Multiple deprecated markers found at lines 57, 179, 180, 198 in debt.rs and line 179 in main.rs.

#### Required Changes:
- Review each deprecated item and determine replacement strategy
- Update calling code to use modern alternatives
- Remove deprecated code after migration
- Update documentation to reflect changes

#### Implementation Steps:
- Audit each deprecated function/method to understand its usage
- Identify and implement modern replacements
- Update all call sites to use new implementations
- Add deprecation warnings if gradual migration needed
- Remove deprecated code once all migrations complete

### 4. FIXME: Potential Panic
**Impact Score**: 7/10
**Effort Score**: 3/10
**Category**: Fixme
**File**: /Users/glen/memento-mori/mmm/src/context/debt.rs:669
**Priority**: High

#### Current State:
```rust
// FIXME: This can panic
```

#### Required Changes:
```rust
// Replace unwrap/expect with proper error handling
// Return Result<T, Error> instead of panicking
// Add context to errors for better debugging
```

#### Implementation Steps:
- Identify the code that can panic at line 669
- Replace `.unwrap()` or `.expect()` with `?` operator or proper match
- Ensure function returns `Result<T, Error>`
- Add error context using `.context()` from anyhow
- Write tests to verify panic conditions are handled gracefully

### 5. High Complexity Function: handle_worktree_merge
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: /Users/glen/memento-mori/mmm/src/cook/mod.rs
**Priority**: High

#### Current State:
Function has cyclomatic complexity of 23.

#### Required Changes:
- Extract merge validation logic
- Separate conflict resolution from merge execution
- Create dedicated merge strategy handlers
- Implement state machine for merge workflow

#### Implementation Steps:
- Create MergeValidator struct to handle pre-merge checks
- Extract conflict detection into separate function
- Implement MergeStrategy trait with different merge approaches
- Create MergeContext to manage merge state
- Add comprehensive error handling and rollback capability

## Dependency Cleanup

### Dependencies to Audit:
- Run `cargo outdated` to identify outdated dependencies
- Run `cargo audit` to check for security vulnerabilities
- Review Cargo.toml for unused dependencies

## Code Organization Changes

### Module Restructuring:
- Consider splitting large modules (debt.rs, workflow.rs, cook/mod.rs) into smaller, focused modules
- Group related functionality together
- Improve module visibility and API boundaries

## Success Criteria
- [ ] All functions with complexity > 20 refactored to < 10
- [ ] All deprecated code removed or migrated
- [ ] All FIXME comments addressed
- [ ] No functions that can panic without proper error handling
- [ ] Test coverage improved from 41.14% towards 70%
- [ ] All files compile without warnings
- [ ] Performance benchmarks maintained or improved
- [ ] Clippy lints resolved or explicitly allowed with justification