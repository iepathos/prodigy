# Iteration 3: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Unwrap Usage in Test Files
**Severity**: Medium
**Category**: Error Handling
**File**: src/analyzer/language.rs
**Line**: 199, 219

#### Current Code:
```rust
let language = detector.detect(&structure).unwrap();
assert_eq!(language, Language::Rust);
```

#### Required Change:
```rust
let language = detector.detect(&structure).expect("Language detection should succeed for test");
assert_eq!(language, Language::Rust);
```

#### Implementation Notes:
- Replace `unwrap()` with `expect()` in test files to provide meaningful error messages
- This improves test debugging when failures occur

### 2. Excessive Clone Operations
**Severity**: Medium
**Category**: Performance
**File**: src/analyzer/structure.rs, src/analyzer/build.rs, src/analyzer/framework.rs
**Line**: Multiple locations

#### Current Code:
```rust
dirs.push(path.clone());
```

#### Required Change:
```rust
dirs.push(path.to_path_buf());
```

#### Implementation Notes:
- Replace unnecessary `clone()` calls on PathBuf with `to_path_buf()` where appropriate
- Review if cloning is actually needed or if borrowing would suffice
- Focus on hot paths in the analyzer modules

### 3. Large Error Enum
**Severity**: Low
**Category**: Code Organization
**File**: src/error.rs
**Line**: 1-137

#### Current Code:
```rust
#[derive(Error, Debug)]
pub enum Error {
    // 30+ variants including many plugin-specific errors
    #[error("Plugin error: {0}")]
    Plugin(String),
    // ... many more variants
}
```

#### Required Change:
```rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    // Core errors only, remove unused plugin errors
    // Group related errors together
}
```

#### Implementation Notes:
- Remove unused plugin-related error variants
- The project has been simplified to not use plugins
- Keep only actively used error types

### 4. Missing Unit Tests
**Severity**: High
**Category**: Testing
**File**: src/improve/mod.rs, src/analyzer/health.rs
**Line**: N/A

#### Current Code:
No unit tests for core business logic in improve module

#### Required Change:
Add unit tests for:
- `extract_spec_from_git` function
- `calculate_health_score` function
- Git command parsing logic

#### Implementation Notes:
- Create test modules with `#[cfg(test)]` 
- Test edge cases for spec extraction
- Mock external command calls where appropriate

### 5. TODO Comments Without Tracking
**Severity**: Low
**Category**: Documentation
**File**: src/analyzer/health.rs
**Line**: 176

#### Current Code:
```rust
// TODO: Implement actual dependency freshness check
// For now, assume dependencies are updated
true
```

#### Required Change:
```rust
// NOTE: Dependency freshness check not implemented
// This would require parsing lock files and checking against registries
// Returns true as a safe default for now
true
```

#### Implementation Notes:
- Convert TODO to NOTE with explanation
- Or implement basic dependency checking if straightforward
- Document why the current behavior is acceptable

### 6. Unsafe Expect Usage
**Severity**: Medium
**Category**: Error Handling
**File**: src/analyzer/quality.rs, src/simple_state/cache.rs
**Line**: Multiple

#### Current Code:
```rust
.expect("Failed to acquire lock")
```

#### Required Change:
```rust
.map_err(|e| anyhow::anyhow!("Failed to acquire lock: {}", e))?
```

#### Implementation Notes:
- Replace expect() with proper error propagation
- Provide context for lock acquisition failures
- Use anyhow for better error context

## Success Criteria
- [ ] All unwrap() calls in non-test code replaced with proper error handling
- [ ] Unnecessary clone() operations reduced
- [ ] Error enum simplified by removing unused variants
- [ ] Unit tests added for core business logic
- [ ] TODO comments addressed or converted to NOTE
- [ ] All expect() calls reviewed and replaced where appropriate
- [ ] All files compile without warnings
- [ ] Tests pass