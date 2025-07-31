# Iteration 1753962684: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

## Debt Items to Address

### 1. Deprecated Comments in debt.rs
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**File**: /Users/glen/memento-mori/mmm/src/context/debt.rs
**Priority**: Critical

#### Current State:
Multiple deprecated comments found in debt.rs at lines 57, 179, 180, 198, 735. These indicate outdated code patterns or functionality that should be updated or removed.

#### Implementation Steps:
- Review each deprecated comment and identify the current recommended approach
- Update code to use modern patterns
- Remove deprecated comments after updating code
- Ensure all tests pass after modifications

### 2. High Complexity in test_cook_multiple_iterations_with_focus
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: /Users/glen/memento-mori/mmm/tests/cook_iteration_tests.rs
**Priority**: Critical

#### Current State:
```rust
// Function has cyclomatic complexity of 13
fn test_cook_multiple_iterations_with_focus() {
    // Complex test logic with multiple branches
}
```

#### Required Changes:
- Extract helper functions for setup and verification
- Split test into smaller, focused test cases
- Use test fixtures to reduce duplication
- Apply arrange-act-assert pattern consistently

#### Implementation Steps:
- Create helper function for test setup
- Extract assertion logic into separate functions
- Consider using parameterized tests for similar test cases
- Add documentation for complex test scenarios

### 3. High Complexity in analyze_complexity
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: /Users/glen/memento-mori/mmm/src/context/debt.rs
**Priority**: Critical

#### Current State:
```rust
// Function has cyclomatic complexity of 14
fn analyze_complexity(...) -> ... {
    // Complex analysis logic with many branches
}
```

#### Required Changes:
- Split into smaller, focused functions
- Extract pattern matching logic
- Use early returns to reduce nesting
- Consider using visitor pattern for AST traversal

#### Implementation Steps:
- Extract complexity calculation into separate function
- Create helper functions for different node types
- Implement strategy pattern for different complexity metrics
- Add unit tests for each extracted function

### 4. High Complexity in setup_improvement_session
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: /Users/glen/memento-mori/mmm/src/cook/mod.rs
**Priority**: Critical

#### Current State:
```rust
// Function has cyclomatic complexity of 12
async fn setup_improvement_session(...) -> Result<...> {
    // Complex setup logic with many conditions
}
```

#### Required Changes:
- Extract session validation logic
- Separate environment setup from session creation
- Use builder pattern for session configuration
- Reduce conditional nesting

#### Implementation Steps:
- Create SessionBuilder for cleaner configuration
- Extract validation into separate methods
- Use Result chaining with ? operator
- Add comprehensive error context

### 5. Code Duplication in cook/mod.rs
**Impact Score**: 7/10
**Effort Score**: 4/10
**Category**: Duplication
**File**: /Users/glen/memento-mori/mmm/src/cook/mod.rs
**Priority**: High

#### Current State:
Duplicate code blocks found at lines 982-993 and 1479-1490.

#### Required Changes:
- Extract common functionality into shared function
- Use generic parameters if patterns differ slightly
- Ensure consistent error handling

#### Implementation Steps:
- Identify common pattern in duplicated code
- Create generic helper function
- Update both locations to use helper
- Add tests for the extracted function

### 6. FIXME Comments Throughout debt.rs
**Impact Score**: 7/10
**Effort Score**: 3/10
**Category**: Fixme
**File**: /Users/glen/memento-mori/mmm/src/context/debt.rs
**Priority**: High

#### Current State:
Multiple FIXME comments at lines 17, 55, 173, 174, 178, 196, 513, 527, 531, 648.

#### Implementation Steps:
- Review each FIXME and determine if still relevant
- Implement fixes for valid issues
- Remove obsolete FIXME comments
- Convert remaining FIXMEs to proper GitHub issues

### 7. God Component: cook module
**Impact Score**: 6/10
**Effort Score**: 6/10
**Category**: Architecture
**File**: src/cook/
**Priority**: Medium

#### Current State:
The cook module has 13 dependencies, indicating high coupling and possibly too many responsibilities.

#### Required Changes:
- Split cook module into smaller, focused modules
- Extract workflow orchestration into separate module
- Move git operations to dedicated module
- Separate metrics collection from core logic

#### Implementation Steps:
- Analyze current responsibilities of cook module
- Create new modules for distinct concerns
- Move related functionality together
- Update imports and module structure
- Ensure all tests pass after refactoring

## Code Organization Changes

### Missing Public Interfaces
**File**: src/abstractions/
The abstractions module has no public interfaces, which violates component design principles.

#### Actions:
- Review module purpose and expose necessary interfaces
- Consider if module should be merged with another
- Add proper module documentation

## Clippy Warnings to Address

### Missing Error Documentation
- Add `# Errors` sections to all public functions returning `Result`
- Document specific error conditions and handling

### Function Too Long
- Split `display_pretty_analysis` function (125 lines) into smaller functions
- Extract display logic for different analysis sections

### Match Same Arms
- Consolidate identical match arms in analyze/command.rs:112

### Precision Loss Warnings
- Review u64 to f64 casts and ensure precision loss is acceptable
- Consider using dedicated types for metrics

## Success Criteria
- [x] All debt items with impact >= 7 addressed
- [x] Hotspots with cyclomatic complexity > 10 refactored
- [ ] All FIXME and deprecated comments resolved or converted to issues
- [ ] God component (cook module) responsibilities reduced
- [ ] All files compile without warnings
- [ ] Tests pass with same or improved coverage (currently 41.14%)
- [ ] Clippy lints resolved or explicitly allowed with justification
- [ ] Technical debt score reduced from 37.85

## Validation Commands
```bash
cargo check --all-features
cargo test --all-features
cargo clippy -- -W clippy::all
cargo fmt -- --check
```