# Iteration 1: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Missing `force` parameter in test cleanup_session calls
**Severity**: High
**Category**: Compilation Error
**File**: src/worktree/test_state.rs, src/worktree/tests.rs
**Line**: Multiple (68, 102, 152, 153, 195 in test_state.rs; 72, 89, 127, 161, 179 in tests.rs)

#### Current Code:
```rust
manager.cleanup_session(&session.name)?;
```

#### Required Change:
```rust
manager.cleanup_session(&session.name, false)?;
```

#### Implementation Notes:
- The `cleanup_session` method signature changed to include a `force: bool` parameter
- All test calls need to be updated to pass `false` as the second argument
- This is a non-functional change that maintains existing test behavior

### 2. Uninlined format args in tests
**Severity**: Medium
**Category**: Code Quality (Clippy Warning)
**File**: tests/edge_case_tests.rs
**Line**: 134, 137, 209-213, 217-221, 242, 261-265, 288, 289

#### Current Code:
```rust
let filename = format!("concurrent-{}.txt", i);
```

#### Required Change:
```rust
let filename = format!("concurrent-{i}.txt");
```

#### Implementation Notes:
- Use inline format arguments as recommended by clippy
- This improves code readability and follows Rust best practices
- Apply to all occurrences: format!("concurrent-{}.txt", i) → format!("concurrent-{i}.txt")
- Also fix multi-line assert! statements to use inline format args

### 3. Potential Resource Leak in Concurrent Tests
**Severity**: Medium
**Category**: Resource Management
**File**: tests/edge_case_tests.rs
**Line**: 132-143

#### Current Code:
```rust
thread::spawn(move || {
    let filename = format!("concurrent-{}.txt", i);
    let file_path = repo_path.join(&filename);
    let write_result = fs::write(&file_path, format!("Content from thread {}", i));
    let mut results = results.lock().unwrap();
    results.push((i, write_result.is_ok()));
})
```

#### Required Change:
```rust
thread::spawn(move || {
    let filename = format!("concurrent-{i}.txt");
    let file_path = repo_path.join(&filename);
    let write_result = fs::write(&file_path, format!("Content from thread {i}"));
    let mut results = results.lock().unwrap();
    results.push((i, write_result.is_ok()));
})
```

#### Implementation Notes:
- Besides fixing format args, ensure proper cleanup of test files
- Consider adding cleanup in test teardown

### 4. Architecture Documentation Missing
**Severity**: Low
**Category**: Documentation
**File**: N/A (missing files)

#### Current Code:
- No PROJECT.md, CONVENTIONS.md, or ROADMAP.md files exist

#### Required Change:
- Create basic documentation files as referenced in ARCHITECTURE.md

#### Implementation Notes:
- Create minimal PROJECT.md explaining the project purpose
- Create CONVENTIONS.md with basic coding standards
- Create ROADMAP.md with future improvement plans
- These files are referenced in the architecture but don't exist

## Success Criteria
- [ ] All test files compile without errors
- [ ] All clippy warnings are resolved
- [ ] cargo test passes successfully
- [ ] cargo clippy shows no warnings
- [ ] Basic documentation files are created