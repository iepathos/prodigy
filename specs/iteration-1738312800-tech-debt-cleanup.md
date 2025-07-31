# Iteration 1: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

## Debt Items to Address

### 1. High Complexity in run_improvement_loop
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: /Users/glen/.mmm/worktrees/mmm/session-bf81d599-57ad-41ff-939c-24da86d14a6e/src/cook/mod.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 56 - this is extremely high and makes the code difficult to understand, test, and maintain.

#### Required Changes:
- Break down the function into smaller, focused subfunctions
- Extract iteration logic into separate methods
- Separate error handling from business logic
- Use the State pattern or similar for managing workflow states

#### Implementation Steps:
- Extract worktree setup logic into `setup_worktree_session()` method
- Create `execute_single_iteration()` method for iteration logic
- Extract metrics collection into `collect_iteration_metrics()` method
- Create `handle_iteration_result()` for result processing
- Add unit tests for each extracted function

### 2. High Complexity in run_without_worktree_with_vars
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: /Users/glen/.mmm/worktrees/mmm/session-bf81d599-57ad-41ff-939c-24da86d14a6e/src/cook/mod.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 45 - significantly above acceptable thresholds.

#### Required Changes:
- Split into workflow preparation, execution, and cleanup phases
- Extract environment variable handling into dedicated function
- Separate spec file handling logic

#### Implementation Steps:
- Create `prepare_workflow_environment()` method
- Extract `execute_workflow_steps()` method
- Implement `cleanup_workflow_resources()` method
- Add comprehensive error handling with proper context

### 3. High Complexity in run_with_mapping
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: /Users/glen/.mmm/worktrees/mmm/session-bf81d599-57ad-41ff-939c-24da86d14a6e/src/cook/mod.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 40 - difficult to reason about and test.

#### Required Changes:
- Extract command mapping logic into separate module
- Create dedicated handlers for different command types
- Implement command validation separately

#### Implementation Steps:
- Create `CommandMapper` struct to handle mapping logic
- Implement `validate_command()` method
- Extract `execute_mapped_command()` method
- Add unit tests for command mapping scenarios

### 4. Deprecated Comments Without Context
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**File**: /Users/glen/.mmm/worktrees/mmm/session-bf81d599-57ad-41ff-939c-24da86d14a6e/src/context/debt.rs
**Priority**: High

#### Current State:
Multiple deprecated comments at lines 57, 179, 180, 198 without proper context or migration path.

#### Required Changes:
- Remove or update deprecated comments
- Add proper deprecation notices with migration guidance
- Document reasons for deprecation

#### Implementation Steps:
- Review each deprecated comment location
- Either remove if no longer relevant or add proper deprecation attributes
- Update documentation to reflect current best practices

### 5. Code Duplication in Test Modules
**Impact Score**: 6/10
**Effort Score**: 3/10
**Category**: Duplication
**File**: Multiple files (test_coverage.rs:603-610, debt.rs:497-504, architecture.rs:400-407)
**Priority**: Medium

#### Current State:
Identical code blocks exist across multiple test modules, violating DRY principles.

#### Required Changes:
```rust
// Current duplicated code in multiple files:
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    // ... identical setup code
}
```

#### Implementation Steps:
- Create a shared test utilities module in `src/testing/test_helpers.rs`
- Extract common test setup into reusable functions
- Update all test modules to use the shared utilities
- Remove duplicated code blocks

### 6. Missing Test Coverage
**Impact Score**: 7/10
**Effort Score**: 4/10
**Category**: TestCoverage
**File**: Overall project
**Priority**: High

#### Current State:
- Test coverage: 40.22% (should be > 70%)
- Doc coverage: 21.18% (should be > 50%)
- Many critical functions lack tests

#### Required Changes:
- Add unit tests for all public APIs
- Increase integration test coverage
- Add documentation tests for examples

#### Implementation Steps:
- Generate coverage report with `cargo tarpaulin`
- Prioritize untested critical functions
- Add tests for error handling paths
- Document all public functions with examples

### 7. Component Dependency Issues
**Impact Score**: 6/10
**Effort Score**: 3/10
**Category**: Architecture
**File**: src/cook
**Priority**: Medium

#### Current State:
The `cook` component has 13 dependencies, indicating potential god object antipattern.

#### Required Changes:
- Refactor to reduce coupling
- Extract specialized functionality into separate modules
- Apply Single Responsibility Principle

#### Implementation Steps:
- Identify core responsibilities of cook module
- Extract git operations into dedicated service
- Move workflow logic to workflow module
- Reduce direct dependencies to < 8

### 8. Missing Public Interfaces
**Impact Score**: 4/10
**Effort Score**: 3/10
**Category**: Architecture
**File**: src/abstractions
**Priority**: Low

#### Current State:
The abstractions module has no public interfaces, making it unclear what functionality it provides.

#### Required Changes:
- Define clear public API
- Add documentation for module purpose
- Consider if module is necessary

#### Implementation Steps:
- Review module contents and purpose
- Either expose necessary interfaces or remove module
- Add module-level documentation

## Dependency Cleanup

### Dependencies to Review:
- `atty` - Deprecated, replace with `std::io::IsTerminal`
- `pest` and `pest_derive` - Check if actually used for parsing
- `gray_matter` - Verify usage in markdown processing
- `tera` - Confirm template usage requirements

### Dependencies to Update:
- Check all dependencies with `cargo outdated`
- Update patch versions for security fixes
- Review and update major versions where breaking changes are acceptable

## Code Organization Changes

### Module Restructuring:
- Move test utilities from individual modules to shared `testing` module
- Consolidate error types into dedicated error module
- Group related functionality (e.g., all git operations)

## Success Criteria
- [x] All debt items with impact >= 7 addressed
- [ ] Hotspots with cyclomatic complexity > 30 refactored
- [ ] Unused dependencies removed from Cargo.toml
- [ ] Code organization follows project conventions
- [ ] All files compile without warnings
- [ ] Tests pass with same or improved coverage
- [ ] Performance benchmarks maintained or improved
- [ ] Clippy lints resolved or explicitly allowed with justification

## Validation Commands
```bash
# Run after implementing changes
cargo fmt
cargo clippy -- -W clippy::all -W clippy::pedantic
cargo test --all-features
cargo tarpaulin --out Html
cargo bench
```

## Next Steps
1. Start with highest impact items (complexity in cook module)
2. Extract and refactor one function at a time
3. Add tests for each refactored component
4. Update documentation as code changes
5. Run validation commands after each major change