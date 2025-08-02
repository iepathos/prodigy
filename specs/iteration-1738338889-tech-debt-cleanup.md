# Iteration 1: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

## Debt Items to Address

### 1. High Complexity in count_match_arms Function
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: src/context/debt.rs
**Priority**: High

#### Current State:
```rust
fn count_match_arms(&self, line: &str) -> u32 {
    // Function has cyclomatic complexity of 49
    // This is significantly above the recommended threshold of 10
}
```

#### Required Changes:
- Break down the function into smaller, focused helper functions
- Extract pattern matching logic into separate methods
- Consider using a visitor pattern or state machine approach

#### Implementation Steps:
- Extract match arm detection into dedicated methods
- Create separate functions for different match arm patterns
- Add comprehensive unit tests for each extracted function
- Verify complexity reduction using cargo clippy

### 2. High Complexity in run Function
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: src/subprocess/runner.rs
**Priority**: High

#### Current State:
```rust
pub async fn run(&self, command: ProcessCommand) -> Result<ProcessOutput>
// Function has cyclomatic complexity of 32
```

#### Required Changes:
- Split the function into logical phases: validation, execution, output handling
- Extract timeout handling into a separate function
- Simplify error handling paths

#### Implementation Steps:
- Create `validate_command` helper function
- Extract `execute_with_timeout` method
- Separate stdin/stdout handling logic
- Add integration tests for edge cases

### 3. High Complexity in save_analysis_with_commit
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: src/context/mod.rs
**Priority**: High

#### Current State:
```rust
pub async fn save_analysis_with_commit(...) -> Result<()>
// Function has cyclomatic complexity of 28
```

#### Required Changes:
- Separate analysis saving from git operations
- Extract commit message generation logic
- Simplify error handling flow

#### Implementation Steps:
- Create `prepare_analysis_data` function
- Extract `generate_commit_message` helper
- Implement `perform_git_operations` method
- Add rollback mechanism for failures

### 4. Code Duplication Hotspots
**Impact Score**: 7/10
**Effort Score**: 4/10
**Category**: Duplication
**File**: Multiple files
**Priority**: High

#### Duplication Summary:
- Total duplicate blocks: 100
- Total duplicate lines: 4,675
- Files with duplication: 98

#### Required Changes:
- Extract common error handling patterns into shared utilities
- Create trait implementations for repeated patterns
- Consolidate test helper functions

#### Implementation Steps:
- Identify and extract common error handling macros
- Create shared test utilities module
- Implement derive macros for boilerplate reduction
- Remove redundant implementations

### 5. Deprecated Code Markers
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**Files**: src/context/debt.rs, src/main.rs
**Priority**: High

#### Current State:
```rust
// Multiple instances of DEPRECATED comments found
// Lines 57, 219, 220, 238 in debt.rs
// Line 174 in main.rs
```

#### Required Changes:
- Remove or update deprecated code sections
- Replace with modern implementations
- Update documentation

#### Implementation Steps:
- Review each deprecated section for current alternatives
- Update implementations to use modern patterns
- Remove obsolete code paths
- Update related tests

### 6. FIXME Comments
**Impact Score**: 7/10
**Effort Score**: 3/10
**Category**: Fixme
**File**: src/context/debt.rs
**Priority**: High

#### Notable Issues:
- Line 831: "This is a hack"
- Line 836: "This needs review"
- Line 990: "This can panic"

#### Required Changes:
- Address panic-prone code with proper error handling
- Replace hacks with proper implementations
- Complete pending reviews

#### Implementation Steps:
- Replace unwrap/expect with proper Result handling
- Implement proper error propagation
- Add defensive programming checks
- Write tests for edge cases

### 7. Security Warning - Unmaintained Dependency
**Impact Score**: 9/10
**Effort Score**: 2/10
**Category**: Security
**Dependency**: atty 0.2.14
**Priority**: Critical

#### Current State:
- RUSTSEC-2024-0375: `atty` is unmaintained
- RUSTSEC-2021-0145: Potential unaligned read

#### Required Changes:
- Replace atty with is-terminal or similar maintained alternative
- Update all usage points

#### Implementation Steps:
- Add is-terminal to Cargo.toml
- Replace atty::is(Stream::Stdout) with std::io::IsTerminal
- Remove atty dependency
- Run cargo audit to verify fix

## Dependency Cleanup

### Dependencies to Update:
- atty: Remove and replace with is-terminal
- Consider updating other dependencies flagged by cargo outdated

## Code Organization Changes

### Module Restructuring:
- src/cook/mod_old.rs → Consider removing if truly deprecated
- Consolidate test utilities scattered across multiple test files
- Extract common patterns from hotspot files

## Success Criteria
- [ ] All debt items with impact >= 7 addressed
- [ ] Cyclomatic complexity reduced below 20 for all functions
- [ ] Security vulnerabilities resolved (cargo audit clean)
- [ ] Code duplication reduced by at least 50%
- [ ] All FIXME and DEPRECATED comments addressed
- [ ] All files compile without warnings
- [ ] Tests pass with same or improved coverage
- [ ] Performance benchmarks maintained or improved
- [ ] Clippy lints resolved or explicitly allowed with justification