# Iteration 1754101200: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Regex creation in hot path without caching
**Severity**: High
**Category**: Performance
**File**: src/config/command_parser.rs
**Line**: 98

#### Current Code:
```rust
fn expand_string(s: &str, variables: &std::collections::HashMap<String, String>) -> String {
    let mut result = s.to_string();

    // Find all ${VAR_NAME} patterns
    let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();

    for cap in re.captures_iter(s) {
```

#### Required Change:
```rust
use once_cell::sync::Lazy;

static VAR_REGEX: Lazy<regex::Regex> = Lazy::new(|| {
    regex::Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex pattern")
});

fn expand_string(s: &str, variables: &std::collections::HashMap<String, String>) -> String {
    let mut result = s.to_string();

    // Find all ${VAR_NAME} patterns
    for cap in VAR_REGEX.captures_iter(s) {
```

#### Implementation Notes:
- Move regex compilation out of the function to avoid recompiling on every call
- Use `once_cell::sync::Lazy` for thread-safe lazy initialization
- This is a significant performance improvement for a function that could be called frequently

### 2. Inefficient string replacement in expand_string
**Severity**: Medium
**Category**: Performance
**File**: src/config/command_parser.rs
**Line**: 95-110

#### Current Code:
```rust
fn expand_string(s: &str, variables: &std::collections::HashMap<String, String>) -> String {
    let mut result = s.to_string();

    // Find all ${VAR_NAME} patterns
    let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();

    for cap in re.captures_iter(s) {
        if let Some(var_name) = cap.get(1) {
            if let Some(value) = variables.get(var_name.as_str()) {
                let pattern = format!("${{{}}}", var_name.as_str());
                result = result.replace(&pattern, value);
            }
        }
    }

    result
}
```

#### Required Change:
```rust
fn expand_string(s: &str, variables: &std::collections::HashMap<String, String>) -> String {
    VAR_REGEX.replace_all(s, |caps: &regex::Captures| {
        caps.get(1)
            .and_then(|m| variables.get(m.as_str()))
            .map(String::as_str)
            .unwrap_or_else(|| caps.get(0).unwrap().as_str())
    }).into_owned()
}
```

#### Implementation Notes:
- Use `replace_all` which is more efficient than multiple string replacements
- Avoids creating intermediate strings for each replacement
- Handles missing variables by keeping the original placeholder

### 3. Deprecated environment variable without proper migration path
**Severity**: Medium
**Category**: User Experience
**File**: src/improve/mod.rs
**Line**: 48

#### Current Code:
```rust
eprintln!("Warning: MMM_USE_WORKTREE is deprecated. Use --worktree or -w flag instead.");
```

#### Required Change:
```rust
eprintln!("⚠️  Warning: MMM_USE_WORKTREE environment variable is deprecated and will be removed in v0.2.0");
eprintln!("   Please use --worktree or -w flag instead");
eprintln!("   Example: mmm improve --worktree");
```

#### Implementation Notes:
- Provide clearer deprecation notice with version information
- Include example of correct usage
- Better formatting with warning emoji

### 4. Missing documentation for public module
**Severity**: Medium
**Category**: Documentation
**File**: src/lib.rs
**Line**: 1

#### Current Code:
```rust
pub mod config;
pub mod improve;
pub mod simple_state;
pub mod worktree;
```

#### Required Change:
```rust
//! MMM (Memento Mori Manager) - Automatic code quality improvement tool
//! 
//! This library provides the core functionality for iterative code improvement
//! using Claude CLI integration. It supports automated code review, implementation
//! of fixes, and quality checks in a git-native workflow.

/// Configuration management for workflows and commands
pub mod config;

/// Core improvement loop implementation
pub mod improve;

/// Simple state management for tracking improvement progress
pub mod simple_state;

/// Git worktree management for parallel improvement sessions
pub mod worktree;
```

#### Implementation Notes:
- Add crate-level documentation
- Add module-level documentation for each public module
- This improves API documentation and helps users understand the structure

### 5. Potential panic in test without proper error context
**Severity**: Low
**Category**: Test Quality
**File**: src/simple_state/tests.rs
**Line**: 11, 12, 20, etc.

#### Current Code:
```rust
let temp_dir = TempDir::new().unwrap();
let state_mgr = StateManager::with_root(temp_dir.path().to_path_buf()).unwrap();
```

#### Required Change:
```rust
let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
let state_mgr = StateManager::with_root(temp_dir.path().to_path_buf())
    .expect("Failed to create StateManager for test");
```

#### Implementation Notes:
- Replace `unwrap()` with `expect()` in tests to provide better error messages
- This helps debug test failures more easily
- Apply to all test files consistently

### 6. Missing error context in worktree operations
**Severity**: Medium
**Category**: Error Handling
**File**: src/improve/mod.rs
**Line**: 97

#### Current Code:
```rust
let worktree_manager =
    WorktreeManager::new(std::env::current_dir()?.parent().unwrap().to_path_buf())?;
```

#### Required Change:
```rust
let worktree_manager = WorktreeManager::new(
    std::env::current_dir()
        .context("Failed to get current directory")?
        .parent()
        .ok_or_else(|| anyhow!("Current directory has no parent"))?
        .to_path_buf()
)?;
```

#### Implementation Notes:
- Add proper error context for directory operations
- Replace `unwrap()` with proper error handling
- Provides better error messages when operations fail

### 7. Inconsistent error message formatting
**Severity**: Low
**Category**: User Experience
**File**: src/main.rs
**Line**: 143, 212, 252, 265

#### Current Code:
```rust
println!("No active MMM worktrees found.");
// ... other places ...
eprintln!("Error: Either --all or a worktree name must be specified");
```

#### Required Change:
```rust
println!("No active MMM worktrees found");
// ... other places ...
eprintln!("Error: Either --all or a worktree name must be specified");
```

#### Implementation Notes:
- Remove trailing periods from status messages for consistency
- Keep periods only for multi-sentence messages
- Maintains consistency with CLI best practices

### 8. Boolean flag list could be configuration-driven
**Severity**: Low
**Category**: Maintainability
**File**: src/config/command_parser.rs
**Line**: 41-44

#### Current Code:
```rust
// Heuristic: if the key suggests it's a boolean flag, treat it as one
// Common boolean flags that don't take values
let boolean_flags = [
    "verbose", "help", "version", "debug", "quiet", "force", "dry-run",
];
```

#### Required Change:
```rust
// Heuristic: if the key suggests it's a boolean flag, treat it as one
// Common boolean flags that don't take values
static BOOLEAN_FLAGS: &[&str] = &[
    "verbose", "help", "version", "debug", "quiet", "force", "dry-run",
    "all", "fix", "check", "watch", "interactive",
];
```

#### Implementation Notes:
- Make the boolean flags list a static constant
- Add more common boolean flags
- Consider making this configurable in the future

## Success Criteria
- [ ] All performance improvements are implemented
- [ ] Error handling is improved with proper context
- [ ] Documentation is added for public modules
- [ ] Test error messages are descriptive
- [ ] All files compile without warnings
- [ ] Tests pass