# Iteration: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

## Debt Items to Address

### 1. Deprecated Code Markers Need Removal
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**File**: src/context/debt.rs:57
**Priority**: High

#### Current State:
```rust
// Line 57 in src/context/debt.rs contains:
// DEPRECATED: No description provided
```

#### Required Changes:
- Review the deprecated code marker and either remove the deprecated code or update it to current standards
- If the code is still needed, remove the DEPRECATED marker and modernize the implementation
- If obsolete, remove the entire deprecated section

#### Implementation Steps:
- Examine src/context/debt.rs:57 to understand what code is marked as deprecated
- Determine if the functionality is still required in the codebase
- Either remove or modernize the deprecated code
- Run `cargo test` to ensure no functionality is broken

### 2. FIXME Comment in Technical Debt Module
**Impact Score**: 7/10
**Effort Score**: 3/10
**Category**: Fixme
**File**: src/context/debt.rs:55
**Priority**: High

#### Current State:
```rust
// Line 55 in src/context/debt.rs contains:
// FIXME: No description provided
```

#### Required Changes:
- Address the FIXME comment by implementing the required fix
- Remove the FIXME comment once the issue is resolved
- Add proper error handling if needed

#### Implementation Steps:
- Investigate src/context/debt.rs:55 to understand what needs fixing
- Implement the necessary fix based on the context
- Add unit tests to verify the fix works correctly
- Remove the FIXME comment

### 3. Redundant Else Blocks (Clippy Warnings)
**Impact Score**: 5/10
**Effort Score**: 2/10
**Category**: Code Quality
**File**: Multiple locations
**Priority**: Medium

#### Current State:
```rust
// src/cook/workflow.rs:266
} else {
    return Err(anyhow!(error_msg));
}

// src/worktree/manager.rs:360
} else {
    anyhow::bail!(
        "Merge verification failed - branch '{}' is not merged into '{}'. \
        The merge may have been aborted or failed silently.",
        worktree_branch,
        target
    );
}
```

#### Required Changes:
```rust
// src/cook/workflow.rs:266
}
return Err(anyhow!(error_msg));

// src/worktree/manager.rs:360
}
anyhow::bail!(
    "Merge verification failed - branch '{}' is not merged into '{}'. \
    The merge may have been aborted or failed silently.",
    worktree_branch,
    target
);
```

#### Implementation Steps:
- Remove redundant else blocks as suggested by clippy
- Apply the same pattern to similar cases throughout the codebase
- Run `cargo clippy` to verify all instances are fixed

### 4. Replace unwrap() with Proper Error Handling
**Impact Score**: 7/10
**Effort Score**: 5/10
**Category**: Error Handling
**File**: Multiple locations (20+ instances found)
**Priority**: High

#### Current State:
```rust
// Examples of problematic unwrap() usage:
// src/abstractions/claude.rs:172
if version_check.is_err() || !version_check.unwrap().status.success() {

// src/analyze/command.rs:23
.unwrap_or_else(|| std::env::current_dir().unwrap());

// src/config/loader.rs:67
let mut config = self.config.write().unwrap();
```

#### Required Changes:
```rust
// Replace with proper error propagation:
// src/abstractions/claude.rs:172
match version_check {
    Ok(output) if output.status.success() => { /* continue */ }
    _ => { /* handle error */ }
}

// src/analyze/command.rs:23
.unwrap_or_else(|| std::env::current_dir()
    .context("Failed to get current directory")?)

// src/config/loader.rs:67
let mut config = self.config.write()
    .map_err(|_| anyhow!("Failed to acquire write lock"))?;
```

#### Implementation Steps:
- Replace unwrap() calls with proper error handling using ? operator or match
- Add context to errors using anyhow's .context() method
- For test code, unwrap() can remain but should use expect() with descriptive messages
- Run all tests to ensure error propagation works correctly

### 5. Missing Documentation Backticks
**Impact Score**: 3/10
**Effort Score**: 1/10
**Category**: Documentation
**File**: src/abstractions/claude.rs:38
**Priority**: Low

#### Current State:
```rust
/// Real implementation of ClaudeClient
```

#### Required Changes:
```rust
/// Real implementation of `ClaudeClient`
```

#### Implementation Steps:
- Add backticks around type names in documentation
- Search for similar cases and fix them as well
- Run `cargo doc --no-deps` to verify documentation builds correctly

### 6. Cook Component Has Too Many Dependencies
**Impact Score**: 6/10
**Effort Score**: 7/10
**Category**: Architecture
**File**: cook module
**Priority**: Medium

#### Current State:
The cook component has 13 dependencies, which suggests it might be doing too much and violating the single responsibility principle.

#### Required Changes:
- Consider splitting the cook module into smaller, more focused modules
- Extract common functionality into separate utility modules
- Reduce coupling by using dependency injection or traits

#### Implementation Steps:
- Analyze the cook module to identify distinct responsibilities
- Create separate modules for each major responsibility
- Move related functionality to the new modules
- Update imports and module declarations
- Ensure all tests still pass

### 7. TODO Comments for Missing Functionality
**Impact Score**: 4/10
**Effort Score**: 3/10
**Category**: Todo
**File**: Multiple locations
**Priority**: Low

#### Current State:
```rust
// src/context/dependencies.rs:289 - TODO: Parse exports
// src/context/dependencies.rs:290 - TODO: Parse Cargo.toml
// src/context/debt.rs:163 - TODO: /FIXME/HACK comments
```

#### Required Changes:
- Implement the missing functionality described in TODO comments
- Remove TODO comments once implemented

#### Implementation Steps:
- For parsing exports: Implement export parsing in the dependency analyzer
- For Cargo.toml parsing: Add proper Cargo.toml parsing logic
- For comment detection: Enhance the debt analyzer to detect FIXME/HACK comments
- Add tests for each new functionality

### 8. Test Coverage Improvement
**Impact Score**: 8/10
**Effort Score**: 6/10
**Category**: Testing
**Priority**: High

#### Current State:
Overall test coverage is only 40.27%, which is below industry standards.

#### Required Changes:
- Add unit tests for untested functions
- Increase integration test coverage
- Target at least 70% coverage

#### Implementation Steps:
- Run `cargo tarpaulin` to identify uncovered code paths
- Write unit tests for critical business logic
- Add integration tests for main workflows
- Focus on error paths and edge cases

## Dependency Cleanup

### Dependencies to Audit:
- Review all dependencies in Cargo.toml for actual usage
- Run `cargo audit` to check for security vulnerabilities
- Use `cargo outdated` to identify outdated dependencies

## Code Organization Changes

### Modules to Restructure:
- Consider splitting the cook module due to high dependency count
- Organize test utilities into a common test module

## Success Criteria
- [ ] All debt items with impact >= 7 addressed
- [ ] Redundant else blocks removed (clippy warnings resolved)
- [ ] unwrap() usage replaced with proper error handling
- [ ] FIXME and DEPRECATED comments addressed
- [ ] All files compile without warnings
- [ ] Tests pass with same or improved coverage
- [ ] Performance benchmarks maintained or improved
- [ ] Clippy lints resolved or explicitly allowed with justification