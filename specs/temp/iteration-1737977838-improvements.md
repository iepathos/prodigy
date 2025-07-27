# Iteration 1737977838: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Clippy Lints - Uninlined Format Arguments
**Severity**: Low
**Category**: Code Style
**File**: src/worktree/manager.rs, src/main.rs
**Line**: Multiple (47, 49, 157-160, 175, 177, 180, 188, 211, 225, 166, 214, 235, 237)

#### Current Code:
```rust
// Example from src/worktree/manager.rs:47
format!("mmm-{}-{}", sanitized_focus, timestamp)
```

#### Required Change:
```rust
format!("mmm-{sanitized_focus}-{timestamp}")
```

#### Implementation Notes:
- Apply clippy's uninlined_format_args suggestion across all format! and println! macros
- This improves readability and is the modern Rust idiom
- Can be auto-fixed with `cargo clippy --fix`

### 2. Module Inception Warning
**Severity**: Low
**Category**: Code Organization
**File**: src/worktree/tests.rs
**Line**: 2-162

#### Current Code:
```rust
// In src/worktree/tests.rs
mod tests {
    // test content
}
```

#### Required Change:
```rust
// Remove the nested mod tests block, keep tests at module level
use super::*;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_worktree_creation() {
    // test content
}
```

#### Implementation Notes:
- Remove the redundant `mod tests` wrapper since the file is already a tests module
- Keep all test functions at the top level of the file
- This follows Rust's module naming conventions

### 3. Unsafe unwrap() Usage Throughout Codebase
**Severity**: High
**Category**: Error Handling
**File**: Multiple files (config/loader.rs, simple_state/*.rs, worktree/manager.rs)
**Line**: Various

#### Current Code:
```rust
// Example from src/config/loader.rs:34
let mut config = self.config.write().unwrap();
```

#### Required Change:
```rust
let mut config = self.config.write()
    .expect("Failed to acquire write lock on config");
```

#### Implementation Notes:
- Replace all `.unwrap()` calls with `.expect()` providing meaningful error messages
- For RwLock operations, use expect with lock acquisition context
- For test code, unwrap() is acceptable but production code should avoid it
- This follows the project's CONVENTIONS.md which states "Never use `unwrap()` in production code"

### 4. Missing Documentation for Public APIs
**Severity**: Medium
**Category**: Documentation
**File**: src/worktree/manager.rs, src/worktree/mod.rs
**Line**: Various public structs and methods

#### Current Code:
```rust
pub struct WorktreeManager {
    pub base_dir: PathBuf,
    pub repo_path: PathBuf,
}
```

#### Required Change:
```rust
/// Manages git worktrees for parallel MMM sessions.
/// 
/// WorktreeManager handles the creation, listing, merging, and cleanup
/// of git worktrees used to isolate concurrent improvement sessions.
pub struct WorktreeManager {
    /// Base directory where worktrees are stored (~/.mmm/worktrees/{repo-name})
    pub base_dir: PathBuf,
    /// Path to the main repository
    pub repo_path: PathBuf,
}
```

#### Implementation Notes:
- Add rustdoc comments for all public structs, methods, and fields
- Document the purpose, parameters, return values, and errors
- Include examples where appropriate
- This improves API usability as mentioned in CONVENTIONS.md

### 5. Error Context Improvements
**Severity**: Medium
**Category**: Error Handling
**File**: src/worktree/manager.rs
**Line**: Various error return points

#### Current Code:
```rust
anyhow::bail!("Failed to create worktree: {}", stderr);
```

#### Required Change:
```rust
anyhow::bail!("Failed to create git worktree '{}' at '{}': {}", 
    name, worktree_path.display(), stderr);
```

#### Implementation Notes:
- Add more context to error messages including relevant paths and values
- Use `.context()` for adding context to Result chains
- This helps with debugging and follows the error handling conventions

## Success Criteria
- [ ] All clippy warnings resolved
- [ ] Module inception warning fixed
- [ ] All unwrap() calls replaced with proper error handling
- [ ] Public APIs documented with rustdoc comments
- [ ] Error messages provide sufficient context
- [ ] All files compile without warnings
- [ ] Tests pass