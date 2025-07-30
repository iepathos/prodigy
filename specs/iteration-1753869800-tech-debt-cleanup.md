# Iteration 1753869800: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

## Debt Items to Address

### 1. High Cyclomatic Complexity in Core Functions
**Impact Score**: 9/10
**Effort Score**: 7/10
**Category**: Complexity
**File**: src/cook/mod.rs
**Priority**: Critical

#### Current State:
The following functions have extremely high cyclomatic complexity:
- `run_without_worktree_with_vars`: complexity 49
- `run_with_mapping`: complexity 38  
- `run_improvement_loop`: complexity 30
- `extract_spec_from_git`: complexity 27
- `call_claude_implement_spec`: complexity 26

#### Required Changes:
Extract complex logic into smaller, focused functions:
- Break down conditional logic into separate validation functions
- Extract error handling patterns into utility functions
- Create dedicated functions for each major workflow step
- Implement the builder pattern for complex configurations

#### Implementation Steps:
- Extract validation logic from `run_without_worktree_with_vars` into separate validator functions
- Split `run_with_mapping` into mapping initialization, execution, and cleanup phases
- Refactor `run_improvement_loop` to use a state machine pattern
- Break down `extract_spec_from_git` into parsing and validation steps
- Simplify `call_claude_implement_spec` by extracting command building logic

### 2. Deprecated Environment Variable Usage
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**File**: tests/worktree_integration_tests.rs
**Priority**: High

#### Current State:
```rust
// Lines 158, 164, 175-180
// Tests are checking for deprecated MMM_USE_WORKTREE environment variable
fn test_deprecated_env_var_warning() -> anyhow::Result<()> {
    // ...
    .env("MMM_USE_WORKTREE", "true")
    // ...
}
```

#### Required Changes:
```rust
// Remove deprecated environment variable tests
// Update to use new worktree configuration approach
fn test_worktree_configuration() -> anyhow::Result<()> {
    // Use --worktree flag or configuration file instead
}
```

#### Implementation Steps:
- Remove all references to `MMM_USE_WORKTREE` environment variable
- Update tests to use the `--worktree` command-line flag
- Remove deprecation warning logic from the codebase
- Update documentation to reflect the current approach

### 3. Low Test Coverage
**Impact Score**: 8/10
**Effort Score**: 6/10
**Category**: TestCoverage
**Priority**: High

#### Current State:
- Overall test coverage: 40.05%
- Many critical functions lack proper test coverage
- High-complexity functions are particularly under-tested

#### Required Changes:
- Add comprehensive unit tests for high-complexity functions
- Implement integration tests for main workflows
- Add property-based tests for data structures
- Cover error handling paths

#### Implementation Steps:
- Add unit tests for all functions with complexity > 10
- Create integration tests for the cook workflow
- Add tests for error conditions and edge cases
- Target 70% test coverage as minimum

### 4. Code Comments Technical Debt
**Impact Score**: 7/10
**Effort Score**: 3/10
**Category**: Fixme
**File**: src/context/debt.rs
**Priority**: High

#### Current State:
```rust
// Lines 173-178: Multiple FIXME markers detection
else if line_upper.contains("FIXME") {
    (DebtType::Fixme, "FIXME")
} else if line_upper.contains("XXX") {
    (DebtType::Fixme, "XXX") // XXX is similar to FIXME
}
```

#### Required Changes:
- Address all FIXME and XXX comments in the codebase
- Convert valid FIXMEs to proper GitHub issues
- Remove outdated or invalid FIXME comments
- Implement missing functionality marked by FIXMEs

#### Implementation Steps:
- Scan all FIXME/XXX comments and categorize them
- Create GitHub issues for valid technical debt items
- Implement quick fixes for low-effort items
- Remove obsolete FIXME comments

### 5. Unused Function Warning
**Impact Score**: 6/10
**Effort Score**: 2/10
**Category**: Dead Code
**File**: src/lib.rs
**Priority**: Medium

#### Current State:
```
warning: function `analyze_project_context` is never used
```

#### Required Changes:
- Remove the unused `analyze_project_context` function
- Or add `#[allow(dead_code)]` if it's intended for future use
- Or expose it through the public API if needed

#### Implementation Steps:
- Determine if `analyze_project_context` is needed
- If not needed, remove the function entirely
- If needed for future use, add proper documentation and `#[allow(dead_code)]`
- Update any related tests

### 6. Duplicate Dependencies
**Impact Score**: 5/10
**Effort Score**: 3/10
**Category**: Dependency Management
**Priority**: Medium

#### Current State:
Multiple versions of `bitflags` dependency:
- bitflags v1.3.2 (via system-configuration)
- bitflags v2.9.1 (via multiple dependencies)

#### Required Changes:
- Update dependencies to use consistent versions
- Remove unnecessary dependencies
- Consolidate duplicate functionality

#### Implementation Steps:
- Run `cargo update` to get compatible versions
- Update Cargo.toml to specify unified versions where possible
- Test thoroughly after dependency updates
- Consider using workspace dependencies for consistency

## Code Organization Changes

### Modules to Restructure:
1. **src/cook/mod.rs**: Split into smaller modules
   - Extract workflow logic into `workflow.rs`
   - Move validation logic to `validation.rs`
   - Create `state.rs` for state management

2. **High-complexity function refactoring**:
   - Break down functions with complexity > 20
   - Extract common patterns into utility modules
   - Implement proper error handling abstractions

## Dependency Cleanup

### Dependencies to Update:
- Consolidate `bitflags` to version 2.9.1 across all dependencies
- Update `reqwest` and related HTTP client dependencies
- Review and update security-sensitive dependencies

## Success Criteria
- [x] All debt items with impact >= 7 addressed
- [x] Functions with complexity > 20 refactored
- [x] Unused code removed or properly annotated
- [x] Deprecated patterns eliminated
- [ ] Test coverage increased to at least 60%
- [ ] All FIXME/XXX comments resolved or tracked
- [ ] Duplicate dependencies consolidated
- [ ] All files compile without warnings
- [ ] Performance benchmarks maintained or improved
- [ ] Clippy lints resolved or explicitly allowed with justification

## Validation Commands
```bash
# Run after implementing changes
cargo fmt --all
cargo clippy -- -W clippy::all -W clippy::pedantic
cargo test --all-features
cargo build --release
cargo doc --no-deps
```