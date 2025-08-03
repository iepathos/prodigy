# Iteration 1: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

## Debt Items to Address

### 1. High Complexity in context Module Functions
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**Files**: 
- `/Users/glen/.mmm/worktrees/mmm/session-5b636df8-1832-40d6-81b0-b477a4cbc201/src/context/mod.rs`
**Priority**: Critical

#### Current State:
- `load_analysis` function has cyclomatic complexity of 26
- `save_analysis_with_commit` function has cyclomatic complexity of 24

#### Required Changes:
- Break down `load_analysis` into smaller, focused functions
- Extract commit logic from `save_analysis_with_commit` into separate functions
- Create dedicated functions for each analysis component loading

#### Implementation Steps:
- Extract analysis loading logic into separate functions for each component
- Create helper functions for file I/O operations
- Implement error handling wrapper functions
- Add comprehensive unit tests for each extracted function

### 2. Deprecated Module hybrid_coverage.rs
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**File**: `/Users/glen/.mmm/worktrees/mmm/session-5b636df8-1832-40d6-81b0-b477a4cbc201/src/context/hybrid_coverage.rs`
**Priority**: High

#### Current State:
```rust
// Line 3: This module has been replaced by the unified scoring system in `src/scoring/mod.rs`.
```

#### Required Changes:
- Remove the deprecated hybrid_coverage.rs module
- Update any remaining references to use the unified scoring system
- Clean up imports and module declarations

#### Implementation Steps:
- Search for all references to hybrid_coverage module
- Replace references with calls to the unified scoring system
- Remove the file and update mod.rs
- Run tests to ensure no functionality is broken

### 3. Excessive Use of unwrap() and expect()
**Impact Score**: 7/10
**Effort Score**: 4/10
**Category**: Error Handling
**Files**: Project-wide (788 unwrap() calls, 40 expect() calls)
**Priority**: High

#### Current State:
- 788 instances of `unwrap()` throughout the codebase
- 40 instances of `expect()` with various messages

#### Required Changes:
- Replace `unwrap()` with proper error propagation using `?`
- Convert `expect()` to context-aware error handling
- Add proper error types where missing

#### Implementation Steps:
- Start with critical paths in main.rs and cook/orchestrator.rs
- Replace unwrap() with ? operator where possible
- Use `.ok_or_else()` or `.context()` for Option types
- Add custom error messages using anyhow's `.context()`
- Focus on the most critical files first (orchestrator.rs, context modules)

### 4. Large File Refactoring
**Impact Score**: 7/10
**Effort Score**: 5/10
**Category**: Code Organization
**Files**: 
- `src/cook/orchestrator.rs` (2242 lines)
- `src/context/debt.rs` (1067 lines)
**Priority**: Medium

#### Current State:
- orchestrator.rs contains 2242 lines with multiple responsibilities
- debt.rs contains 1067 lines of debt analysis logic

#### Required Changes:
```rust
// Split orchestrator.rs into:
// - orchestrator/mod.rs (core orchestration logic)
// - orchestrator/commands.rs (command execution)
// - orchestrator/session.rs (session management)
// - orchestrator/analysis.rs (analysis coordination)
```

#### Implementation Steps:
- Create subdirectory structure for orchestrator module
- Extract command execution logic into separate module
- Move session management to dedicated module
- Extract analysis coordination logic
- Update imports and visibility modifiers

### 5. FIXME Comments in debt.rs
**Impact Score**: 7/10
**Effort Score**: 3/10
**Category**: Fixme
**File**: `/Users/glen/.mmm/worktrees/mmm/session-5b636df8-1832-40d6-81b0-b477a4cbc201/src/context/debt.rs`
**Priority**: Medium

#### Current State:
- Line 831: "This is a hack"
- Line 836: "This needs review"  
- Line 990: "This can panic"
- Multiple other FIXME markers

#### Required Changes:
- Address the hack at line 831 with proper implementation
- Review and fix the code at line 836
- Fix the panic condition at line 990 with proper error handling

#### Implementation Steps:
- Replace hack with proper pattern matching or error handling
- Review the flagged code section and implement proper solution
- Replace panic-prone code with Result-based error handling
- Remove FIXME comments after fixes are implemented

### 6. Clippy Pedantic Warnings
**Impact Score**: 6/10
**Effort Score**: 2/10
**Category**: Code Quality
**Files**: Multiple files
**Priority**: Medium

#### Current State:
- Similar names warnings in debt.rs
- Redundant else blocks in orchestrator.rs
- Various other pedantic warnings

#### Required Changes:
```rust
// Fix similar names:
// block1 -> first_block
// block2 -> second_block
// file1 -> source_file
// file2 -> target_file

// Remove redundant else blocks in orchestrator.rs
```

#### Implementation Steps:
- Run `cargo clippy --fix` for automatic fixes
- Manually fix similar names warnings with more descriptive names
- Remove redundant else blocks as suggested
- Add appropriate `#[allow()]` attributes where warnings are intentional

## Dependency Cleanup

### Dependencies to Review:
- Check for unused dependencies with `cargo +nightly udeps`
- Run `cargo outdated` to identify outdated dependencies
- Review feature flags usage to minimize compile time

## Code Organization Changes

### Files to Restructure:
- `src/cook/orchestrator.rs` → Split into multiple modules
- `src/context/debt.rs` → Extract duplication detection into separate module
- Create better module organization for large files

### Modules to Clean:
- Remove deprecated `hybrid_coverage` module
- Consolidate error handling patterns across modules
- Standardize logging approach (currently using println! in some places)

## Success Criteria
- [x] All debt items with impact >= 7 addressed
- [ ] High complexity functions refactored (complexity < 10)
- [ ] Deprecated modules removed
- [ ] unwrap() usage reduced by at least 50%
- [ ] All FIXME comments addressed or converted to proper TODOs
- [ ] Large files split into manageable modules
- [ ] All clippy warnings resolved or explicitly allowed
- [ ] Tests pass with same or improved coverage (currently 41.38%)
- [ ] No new security vulnerabilities introduced
- [ ] Build time maintained or improved