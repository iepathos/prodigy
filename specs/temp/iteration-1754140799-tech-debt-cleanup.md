# Iteration N: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis. This specification addresses high-impact debt items, code quality issues, and architectural improvements.

## Debt Items to Address

### 1. Clippy Warnings - Redundant Else Blocks
**Impact Score**: 5/10
**Effort Score**: 2/10
**Category**: Code Quality
**Files**: 
- src/cook/workflow.rs:391
- src/worktree/manager.rs:360
**Priority**: Medium

#### Current State:
```rust
// src/cook/workflow.rs:391
if command.metadata.continue_on_error.unwrap_or(false) {
    eprintln!("Warning: Command '{}' failed but continuing: {}", command.name, error_msg);
    return Ok((false, None));
} else {
    return Err(anyhow!(error_msg));
}
```

#### Required Changes:
```rust
if command.metadata.continue_on_error.unwrap_or(false) {
    eprintln!("Warning: Command '{}' failed but continuing: {}", command.name, error_msg);
    return Ok((false, None));
}
return Err(anyhow!(error_msg));
```

#### Implementation Steps:
- Remove redundant else blocks in workflow.rs and manager.rs
- Apply the same pattern to any other redundant else blocks found
- Run `cargo clippy` to verify warnings are resolved

### 2. Missing Documentation Backticks
**Impact Score**: 3/10
**Effort Score**: 1/10
**Category**: Documentation
**Files**: src/abstractions/claude.rs
**Priority**: Low

#### Current State:
```rust
/// Real implementation of ClaudeClient
/// Create a new RealClaudeClient instance
/// Mock implementation of ClaudeClient for testing
```

#### Required Changes:
```rust
/// Real implementation of `ClaudeClient`
/// Create a new `RealClaudeClient` instance
/// Mock implementation of `ClaudeClient` for testing
```

#### Implementation Steps:
- Add backticks around type names in documentation comments
- Apply consistent formatting across all doc comments
- Run `cargo doc` to verify documentation generates correctly

### 3. Missing #[must_use] Attributes
**Impact Score**: 4/10
**Effort Score**: 2/10
**Category**: API Design
**Files**: src/abstractions/claude.rs:43
**Priority**: Medium

#### Current State:
```rust
pub fn new() -> Self {
    Self {
        execute_calls: Arc::new(Mutex::new(vec![])),
    }
}
```

#### Required Changes:
```rust
#[must_use]
pub fn new() -> Self {
    Self {
        execute_calls: Arc::new(Mutex::new(vec![])),
    }
}
```

#### Implementation Steps:
- Add #[must_use] to constructor functions and other appropriate methods
- Review all public APIs for methods that should be marked #[must_use]
- Test that the attribute provides appropriate compiler warnings

### 4. Large Files Requiring Refactoring
**Impact Score**: 7/10
**Effort Score**: 6/10
**Category**: Code Organization
**Files**: 
- src/cook/mod.rs (1857 lines)
- src/cook/workflow.rs (1156 lines)
**Priority**: High

#### Refactoring Plan:
- Extract command execution logic into separate module
- Split workflow processing into smaller, focused modules
- Create dedicated types for command metadata and results
- Improve separation of concerns between orchestration and execution

### 5. Code Duplication in Tests
**Impact Score**: 6/10
**Effort Score**: 4/10
**Category**: Test Quality
**File**: tests/cook_iteration_tests.rs
**Priority**: Medium

#### Current State:
Multiple instances of duplicated test setup code at lines 22-30, 128-136, 217-225, and 340-348.

#### Required Changes:
Extract common test setup into helper functions or test fixtures.

#### Implementation Steps:
- Create a common test utilities module
- Extract repeated setup code into reusable functions
- Apply DRY principle to test code
- Ensure tests remain readable and maintainable

### 6. High Cyclomatic Complexity
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Code Quality
**Files**: 
- tests/cook_iteration_tests.rs (functions with complexity 12-16)
**Priority**: High

#### Refactoring Plan:
- Break down complex test functions into smaller, focused tests
- Extract complex assertion logic into helper functions
- Reduce nesting levels in test code
- Apply single responsibility principle to test functions

### 7. Excessive unwrap() and expect() Usage
**Impact Score**: 7/10
**Effort Score**: 4/10
**Category**: Error Handling
**Count**: 
- 106 instances of clone()
- 38 instances of expect()
- Multiple files with unwrap() usage
**Priority**: High

#### Implementation Steps:
- Replace unwrap() with proper error propagation using ?
- Convert expect() to context-aware error handling with anyhow
- Reduce unnecessary clone() calls by using references where possible
- Add proper error context for better debugging

### 8. TODO/FIXME Comments
**Impact Score**: 4/10
**Effort Score**: 3/10
**Category**: Technical Debt
**Files**: src/context/dependencies.rs, src/context/debt.rs
**Priority**: Medium

#### Items to Address:
- Parse exports (dependencies.rs:295)
- Parse Cargo.toml (dependencies.rs:296)
- Various test-related TODOs in debt.rs

## Dependency Cleanup

### Dependencies to Review:
- Check for unused dependencies with `cargo +nightly udeps`
- Review if all tokio features are needed (currently using "full")
- Consider if all serde features are necessary
- Evaluate need for both `dirs` and `directories` crates

## Code Organization Changes

### Files to Refactor:
- src/cook/mod.rs → Split into smaller modules for better organization
- src/cook/workflow.rs → Extract command execution and validation logic
- Consolidate error handling patterns across modules

### Modules to Restructure:
- Create dedicated error types module
- Extract common test utilities
- Improve module boundaries and visibility

## Success Criteria
- [x] All clippy warnings with -W clippy::pedantic resolved
- [x] Documentation properly formatted with backticks
- [x] #[must_use] attributes added where appropriate
- [ ] Large files refactored to under 500 lines
- [ ] Code duplication reduced in test files
- [ ] Cyclomatic complexity reduced to under 10 for all functions
- [ ] unwrap() usage replaced with proper error handling
- [ ] All TODO/FIXME comments addressed or converted to issues
- [ ] Dependencies audited and unnecessary ones removed
- [ ] All tests pass with same or improved coverage
- [ ] Performance benchmarks maintained or improved

## Validation Commands
```bash
# Run clippy with pedantic warnings
cargo clippy -- -W clippy::all -W clippy::pedantic

# Check for unused dependencies
cargo +nightly udeps

# Run tests
cargo test --all-features

# Generate documentation
cargo doc --no-deps

# Check code coverage
cargo tarpaulin

# Audit dependencies
cargo audit
```

## Next Steps
1. Address high-priority items first (complexity and error handling)
2. Refactor large files incrementally
3. Clean up test duplication
4. Review and update dependencies
5. Run full validation suite after each major change