# Iteration 1: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.
This cleanup focuses on addressing high-impact debt items, reducing code complexity, and eliminating code duplication.

## Debt Items to Address

### 1. Deprecated Comment Markers in debt.rs
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**File**: src/context/debt.rs
**Priority**: High

#### Current State:
```rust
// src/context/debt.rs:57
pub enum DebtType {
    // ...
    Deprecated,  // DEPRECATED marker found
    // ...
}

// src/context/debt.rs:179-180
} else if line_upper.contains("DEPRECATED") {
    (DebtType::Deprecated, "DEPRECATED")
```

#### Required Changes:
Review and address all DEPRECATED markers in the debt tracking system. The debt detection logic itself contains deprecated comments that need to be resolved.

#### Implementation Steps:
- Review lines 57, 179-180, 198 in src/context/debt.rs for DEPRECATED markers
- Determine if the deprecated functionality should be removed or updated
- Update the debt detection logic to properly handle deprecation patterns
- Add proper documentation for the deprecation strategy

### 2. High Complexity in init::run Function
**Complexity Score**: 26
**Change Frequency**: Medium
**Risk Level**: High
**File**: src/init/mod.rs:117

#### Current State:
The `run` function in the init module has cyclomatic complexity of 26, which is significantly above the recommended threshold of 10.

#### Refactoring Plan:
- Split the run function into smaller, focused sub-functions
- Extract validation logic into separate methods
- Create helper functions for each initialization step
- Implement proper error handling patterns

#### Implementation Steps:
- Extract project validation into `validate_project_structure()`
- Create `initialize_directories()` for directory creation
- Move configuration setup to `setup_configuration()`
- Implement `create_initial_state()` for state initialization
- Add comprehensive tests for each extracted function

### 3. Code Duplication in Test Helpers
**Complexity Score**: High
**Change Frequency**: 3 instances
**Risk Level**: Medium
**Files**: Multiple context modules

#### Current State:
```rust
// Duplicated in 3 files:
// src/context/test_coverage.rs:603-610
// src/context/debt.rs:497-504
// src/context/architecture.rs:400-407
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::test_helpers::*;

    #[test]
    fn test_pattern_detection() {
        // Similar test setup code
    }
}
```

#### Refactoring Plan:
- Create a common test utilities module
- Extract shared test setup into reusable functions
- Implement test fixtures for common scenarios
- Use test macros to reduce boilerplate

#### Implementation Steps:
- Create `src/testing/common_fixtures.rs` for shared test data
- Extract test setup patterns into `setup_test_context()` function
- Create macro `test_with_context!` for common test patterns
- Update all duplicated test modules to use shared utilities

### 4. Complex Test Functions in cook_iteration_tests.rs
**Complexity Score**: 16
**Change Frequency**: High
**Risk Level**: Medium
**File**: tests/cook_iteration_tests.rs

#### Current State:
Multiple test functions exceed complexity threshold:
- `test_focus_applied_every_iteration`: complexity 16
- `test_cook_multiple_iterations_with_focus`: complexity 12
- `test_cook_stops_early_when_no_changes`: complexity 12

#### Refactoring Plan:
- Extract test setup into helper functions
- Create builder pattern for test scenarios
- Split complex assertions into smaller, named helper functions
- Use parameterized tests for similar test cases

#### Implementation Steps:
- Create `CookTestBuilder` for fluent test setup
- Extract assertion helpers like `assert_iteration_count()`, `assert_focus_applied()`
- Use rstest or similar for parameterized testing
- Reduce nesting by early returns and guard clauses

### 5. Command String Parser Complexity
**Complexity Score**: 17
**Change Frequency**: High
**Risk Level**: High
**File**: tests/command_parsing_tests.rs:50

#### Current State:
The `parse_command_string` function has complexity of 17, indicating complex parsing logic that's difficult to maintain.

#### Refactoring Plan:
- Implement a proper parser combinator or state machine
- Split parsing logic into tokenization and validation phases
- Add comprehensive error handling with specific error types
- Create unit tests for each parsing rule

#### Implementation Steps:
- Implement `CommandTokenizer` for breaking input into tokens
- Create `CommandValidator` for syntax validation
- Use `nom` or similar parser combinator library if appropriate
- Add property-based tests with proptest for parser robustness

### 6. Duplicated Metric Collection Logic
**Complexity Score**: Medium
**Files**: src/cook/mod.rs:589-607 and 827-845

#### Current State:
Two large blocks of nearly identical code for metric collection appear in the cook module.

#### Refactoring Plan:
- Extract common metric collection into a trait
- Implement the trait for different collection contexts
- Use generics to handle type variations
- Add tests for the extracted functionality

#### Implementation Steps:
- Create `MetricCollector` trait with `collect()` method
- Implement `StandardMetricCollector` and `WorktreeMetricCollector`
- Extract shared logic into trait default methods
- Update cook module to use the new abstraction

## Dependency Cleanup

### Duplicate Dependencies to Resolve:
- **bitflags**: v1.3.2 and v2.9.1 coexist
  - Action: Update all dependencies to use bitflags v2.9.1
  - Files to update: Check system-configuration dependency

### Dependencies to Update:
- Review and update minor versions for security patches
- Run `cargo update` for compatible updates
- Audit feature flags to minimize compile time

## Code Organization Changes

### Files to Move:
- No major file relocations needed based on current analysis

### Modules to Restructure:
- Split `src/init/mod.rs` into smaller modules:
  - `src/init/validation.rs` for project validation
  - `src/init/setup.rs` for initialization logic
  - `src/init/config.rs` for configuration handling

## Testing Improvements

### Test Coverage Gaps:
- Add tests for error handling paths in high-complexity functions
- Implement integration tests for the refactored init module
- Add property-based tests for command parsing
- Create benchmarks for performance-critical paths

## Success Criteria
- [x] All debt items with impact >= 7 addressed
- [x] Hotspots with complexity > 15 refactored
- [ ] Duplicate dependencies resolved
- [ ] Code duplication eliminated in test modules
- [ ] All files compile without warnings
- [ ] Tests pass with same or improved coverage
- [ ] Performance benchmarks maintained or improved
- [ ] Clippy lints resolved or explicitly allowed with justification

## Validation Commands
```bash
# Run after each major change
cargo check --all-targets
cargo test --all-features
cargo clippy -- -W clippy::all
cargo fmt --check

# Final validation
cargo test --release
cargo bench
cargo doc --no-deps
```

## Risk Mitigation
- Create feature branches for each major refactoring
- Run full test suite after each function extraction
- Use `git diff --stat` to verify no unintended changes
- Keep refactoring commits atomic and focused

This cleanup will significantly improve code maintainability, reduce complexity hotspots, and eliminate technical debt that could hinder future development.