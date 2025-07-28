# Iteration 1753679858: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Unnecessary borrowed expression in git_ops.rs
**Severity**: Medium
**Category**: Code Quality
**File**: src/improve/git_ops.rs
**Line**: 140

#### Current Code:
```rust
let output = Command::new("git")
    .args(&["init"])
    .current_dir(temp_dir.path())
    .output()
    .await
    .unwrap();
```

#### Required Change:
```rust
let output = Command::new("git")
    .args(["init"])
    .current_dir(temp_dir.path())
    .output()
    .await
    .unwrap();
```

#### Implementation Notes:
- Remove the unnecessary borrow operator `&` from the array literal `["init"]`
- The `.args()` method can accept the array directly without borrowing

### 2. Use format! variables directly in git_ops.rs
**Severity**: Low
**Category**: Code Quality
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
- Use inline format variables instead of positional parameters
- This makes the code more readable and follows modern Rust conventions

### 3. Simplify boolean logic in improve/mod.rs
**Severity**: Medium
**Category**: Code Quality
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
- The if-else chain can be simplified to a boolean OR expression
- This removes the redundant if-same-then-else pattern
- Makes the code more concise and easier to read

### 4. Use inline format variables in improve/mod.rs tests
**Severity**: Low
**Category**: Code Quality
**File**: src/improve/mod.rs
**Line**: 865

#### Current Code:
```rust
assert!(is_valid, "Should be valid: {}", spec_id);
```

#### Required Change:
```rust
assert!(is_valid, "Should be valid: {spec_id}");
```

#### Implementation Notes:
- Use inline format variables instead of positional parameters
- Apply the same fix to line 885 and line 974

### 5. Use inline format variables in improve/mod.rs test
**Severity**: Low
**Category**: Code Quality
**File**: src/improve/mod.rs
**Line**: 988-991

#### Current Code:
```rust
let formatted = format!(
    "Command '{}' failed with exit code {:?}\nStderr: {}\nStdout: {}",
    cmd, code, stderr, stdout
);
```

#### Required Change:
```rust
let formatted = format!(
    "Command '{cmd}' failed with exit code {code:?}\nStderr: {stderr}\nStdout: {stdout}"
);
```

#### Implementation Notes:
- Use inline format variables for all parameters
- Also fix line 994: `format!("{code:?}")` instead of `format!("{:?}", code)`

### 6. Remove unnecessary borrow in improve/mod.rs test
**Severity**: Medium
**Category**: Code Quality
**File**: src/improve/mod.rs
**Line**: 897

#### Current Code:
```rust
std::process::Command::new("git")
    .args(&["init"])
    .output()
    .unwrap();
```

#### Required Change:
```rust
std::process::Command::new("git")
    .args(["init"])
    .output()
    .unwrap();
```

#### Implementation Notes:
- Remove the unnecessary borrow operator from the array literal

### 7. Fix formatting issues in retry.rs
**Severity**: Low
**Category**: Code Style
**File**: src/improve/retry.rs
**Lines**: 271, 280, 291, 302, 316, 320, 333, 340, 347, 357, 380

#### Current Code:
Various formatting issues with test functions including:
- Multi-line function calls that should be on single lines
- Unnecessary blank lines
- Inconsistent spacing

#### Required Change:
Apply proper rustfmt formatting to all test functions to ensure consistent code style.

#### Implementation Notes:
- Run `cargo fmt` to automatically fix all formatting issues
- Ensure consistent line breaks and spacing in test functions

## Success Criteria
- [ ] All clippy warnings are resolved
- [ ] Code passes `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Code formatting passes `cargo fmt --check`
- [ ] All files compile without warnings
- [ ] Tests pass