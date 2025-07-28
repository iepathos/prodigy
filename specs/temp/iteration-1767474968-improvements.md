# Iteration 1767474968: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Fix Clippy Warnings
**Severity**: Medium
**Category**: Code Quality
**File**: src/improve/git_ops.rs
**Line**: 140

#### Current Code:
```rust
            .args(&["init"])
```

#### Required Change:
```rust
            .args(["init"])
```

#### Implementation Notes:
- Remove unnecessary borrow on array literal
- Apply same fix to line 897 in src/improve/mod.rs

### 2. Fix Format String Issues
**Severity**: Medium  
**Category**: Code Style
**File**: src/improve/git_ops.rs
**Line**: 147

#### Current Code:
```rust
        assert!(output.status.success(), "git init failed: {:?}", output);
```

#### Required Change:
```rust
        assert!(output.status.success(), "git init failed: {output:?}");
```

#### Implementation Notes:
- Use inline format variables throughout codebase
- Fix similar issues in src/improve/mod.rs lines 865, 885, 974, 988, 994

### 3. Simplify Boolean Logic
**Severity**: Low
**Category**: Code Simplification
**File**: src/improve/mod.rs
**Line**: 827-836

#### Current Code:
```rust
let use_worktree = if cmd.worktree {
    true
} else if std::env::var("MMM_USE_WORKTREE")
    .map(|v| v == "true" || v == "1")
    .unwrap_or(false)
{
    true
} else {
    false
};
```

#### Required Change:
```rust
let use_worktree = cmd.worktree || std::env::var("MMM_USE_WORKTREE")
    .map(|v| v == "true" || v == "1")
    .unwrap_or(false);
```

#### Implementation Notes:
- Simplify if-else chain to single boolean expression
- Keep deprecation warning logic intact

### 4. Replace unwrap() in Test Code
**Severity**: Low
**Category**: Error Handling
**File**: Multiple test files

#### Current Code:
```rust
let temp_dir = TempDir::new().unwrap();
```

#### Required Change:
```rust
let temp_dir = TempDir::new()?;
```

#### Implementation Notes:
- Only in test code, so lower priority
- Consider using expect() with descriptive messages instead of bare unwrap()
- This is acceptable in tests but should be documented

### 5. Fix Formatting Issues
**Severity**: Low
**Category**: Code Formatting
**File**: Multiple files

#### Current Code:
Various formatting inconsistencies detected by cargo fmt

#### Required Change:
Apply cargo fmt to all files

#### Implementation Notes:
- Run `cargo fmt` to apply all formatting fixes
- Files affected: src/improve/mod.rs, src/worktree/manager.rs, src/worktree/test_state.rs

### 6. Add Missing Error Handling Trait
**Severity**: High
**Category**: Architecture
**File**: src/error.rs (missing)

#### Current Code:
No centralized error handling module exists despite ARCHITECTURE.md specifying one

#### Required Change:
Create src/error.rs with centralized error types:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MmmError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Git operation failed: {0}")]
    Git(String),
    
    #[error("Claude CLI error: {0}")]
    ClaudeCli(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Worktree error: {0}")]
    Worktree(String),
}

pub type Result<T> = std::result::Result<T, MmmError>;
```

#### Implementation Notes:
- Architecture specifies centralized error handling but module is missing
- Currently using anyhow::Result throughout
- Migration can be gradual, starting with new code

## Success Criteria
- [ ] All Clippy warnings resolved
- [ ] Format string issues fixed
- [ ] Boolean logic simplified
- [ ] Code formatted with cargo fmt
- [ ] All files compile without warnings
- [ ] Tests pass