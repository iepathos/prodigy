# Iteration 1: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

## Debt Items to Address

### 1. High Code Duplication
**Impact Score**: 5/10 (per instance, 9014 total instances)
**Effort Score**: 2/10
**Category**: Duplication
**Priority**: Critical

#### Current State:
The codebase contains 9,014 duplication instances with a total impact score of 45,070. This represents a significant maintenance burden.

#### Required Changes:
- Extract common test setup patterns into shared test utilities
- Create generic functions for repeated error handling patterns
- Consolidate similar CLI parsing logic
- Use trait implementations to reduce boilerplate

#### Implementation Steps:
- Identify top 20 duplication hotspots using the duplication_map
- Create shared test utilities module in `tests/common/mod.rs`
- Extract repeated patterns into reusable functions
- Run tests after each extraction to ensure functionality preserved

### 2. Deprecated Comment Handling
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**File**: tests/worktree_integration_tests.rs
**Priority**: High

#### Current State:
```rust
// Lines 158, 164, 175, 176 contain deprecated markers
fn test_deprecated_env_var_warning() -> anyhow::Result<()> {
    // ... deprecated test logic
```

#### Required Changes:
- Update tests to use current API patterns
- Remove references to deprecated `MMM_USE_WORKTREE` environment variable
- Update documentation to reflect current usage patterns

#### Implementation Steps:
- Review deprecated environment variable usage in tests
- Update to use `--worktree` flag instead of env var
- Remove or update tests that rely on deprecated functionality
- Update any related documentation

### 3. High Complexity: init::run Function
**Complexity Score**: 26
**File**: src/init/mod.rs
**Priority**: High

#### Current State:
The `run` function in the init module has a cyclomatic complexity of 26, making it difficult to test and maintain.

#### Refactoring Plan:
- Extract configuration validation into separate function
- Split file creation logic into dedicated helper functions
- Create separate functions for template rendering
- Use builder pattern for complex initialization logic

#### Implementation Steps:
- Extract `validate_init_options()` function
- Create `create_project_structure()` helper
- Move template logic to `render_templates()` function
- Add unit tests for each extracted function

### 4. High Complexity: parse_command_string Function
**Complexity Score**: 17
**File**: tests/command_parsing_tests.rs
**Priority**: High

#### Current State:
The `parse_command_string` function has complexity of 17 with multiple nested conditions.

#### Refactoring Plan:
- Use a command parser library or state machine
- Extract quote handling into separate function
- Create command token struct for better organization
- Simplify escape sequence handling

### 5. Cook Module God Component
**Impact Score**: 7/10
**Effort Score**: 6/10
**Category**: Architecture
**File**: src/cook/mod.rs
**Priority**: Medium

#### Current State:
The cook module has 13 dependencies, indicating it may be doing too much.

#### Required Changes:
- Extract workflow execution into separate module
- Move git operations to dedicated service
- Separate configuration handling from execution logic
- Create cleaner interfaces between components

#### Implementation Steps:
- Analyze cook module responsibilities
- Create `workflow_executor` module for workflow logic
- Move git operations to existing `git_ops` module
- Update imports and module structure
- Ensure all tests pass after refactoring

### 6. TODO/FIXME Comments
**Impact Score**: 4-7/10
**Effort Score**: 3/10
**Category**: Todo/Fixme
**Priority**: Medium

#### Items to Address:
1. `src/context/dependencies.rs:289` - "Parse exports"
2. `src/context/dependencies.rs:290` - "Parse Cargo.toml"
3. `src/context/debt.rs:54-55` - TODO and FIXME comments

#### Implementation Steps:
- Implement export parsing logic for dependency analysis
- Add Cargo.toml parsing for better dependency insights
- Address or document the TODO/FIXME items in debt.rs

## Dependencies to Update

### Check for Outdated Dependencies:
```bash
cargo outdated
cargo audit
```

### Remove Unused Dependencies:
```bash
cargo +nightly udeps
```

## Code Organization Changes

### Module Restructuring:
- Split large test files with multiple high-complexity functions
- Ensure abstractions module exposes proper public interfaces
- Organize test utilities into shared modules

## Success Criteria
- [x] All clippy warnings resolved
- [x] Code formatted with rustfmt
- [ ] Deprecated comments addressed (4 instances)
- [ ] High-complexity functions refactored (complexity < 15)
- [ ] Cook module dependencies reduced
- [ ] TODO/FIXME comments resolved or documented
- [ ] Tests pass with same or improved coverage
- [ ] No new clippy warnings introduced
- [ ] Build time maintained or improved

## Validation Commands
```bash
cargo fmt --check
cargo clippy -- -W clippy::all
cargo test --all-features
cargo build --release
cargo bench --no-run
```