# Iteration 1: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Implement Product Management Command (Spec 31)
**Severity**: Medium
**Category**: Feature Implementation
**File**: Claude command definitions
**Line**: N/A

#### Current Code:
The project has implemented `/mmm-code-review` but is missing the `/mmm-product-enhance` command from spec 31.

#### Required Change:
Create `/mmm-product-enhance` command that analyzes code from a product management perspective, focusing on:
- User experience improvements
- Feature completeness
- API design enhancements
- Documentation gaps
- Onboarding improvements

#### Implementation Notes:
- Create command definition in Claude CLI
- Follow existing pattern from `/mmm-code-review`
- Generate specs in `specs/temp/` with format: `product: enhance {feature} for iteration-{timestamp}`
- Integrate with existing workflow system

### 2. Error Handling Pattern Inconsistency
**Severity**: Medium
**Category**: Code Quality
**File**: src/worktree/manager.rs
**Line**: 20-21

#### Current Code:
```rust
let repo_name = repo_path
    .file_name()
    .and_then(|n| n.to_str())
    .ok_or_else(|| anyhow!("Could not determine repository name"))?;
```

#### Required Change:
```rust
let repo_name = repo_path
    .file_name()
    .and_then(|n| n.to_str())
    .ok_or_else(|| anyhow!("Could not determine repository name from path: {}", repo_path.display()))?;
```

#### Implementation Notes:
- Include context in error messages throughout the codebase
- Make errors more descriptive for debugging

### 3. Missing Documentation for Public Functions
**Severity**: Low
**Category**: Documentation
**File**: src/cook/git_ops.rs
**Line**: Various

#### Current Code:
Public functions in git_ops module lack documentation.

#### Required Change:
Add rustdoc comments to all public functions in the git_ops module, explaining:
- Purpose and behavior
- Parameters and their meaning
- Return values
- Error conditions
- Thread safety guarantees

#### Implementation Notes:
- Follow Rust documentation conventions
- Include examples where appropriate
- Document the git mutex behavior

### 4. Potential Race Condition in Worktree State
**Severity**: Medium
**Category**: Concurrency
**File**: src/worktree/manager.rs
**Line**: 85-102

#### Current Code:
```rust
fn save_session_state(&self, session: &WorktreeSession) -> Result<()> {
    let state_dir = self.base_dir.join(".metadata");
    fs::create_dir_all(&state_dir)?;
    
    let state_file = state_dir.join(format!("{}.json", session.name));
    let state = WorktreeState {
        // ... state creation
    };
    
    let state_json = serde_json::to_string_pretty(&state)?;
    fs::write(&state_file, state_json)?;
    
    Ok(())
}
```

#### Required Change:
```rust
fn save_session_state(&self, session: &WorktreeSession) -> Result<()> {
    let state_dir = self.base_dir.join(".metadata");
    fs::create_dir_all(&state_dir)?;
    
    let state_file = state_dir.join(format!("{}.json", session.name));
    let temp_file = state_dir.join(format!("{}.json.tmp", session.name));
    
    let state = WorktreeState {
        // ... state creation
    };
    
    let state_json = serde_json::to_string_pretty(&state)?;
    
    // Write to temp file first, then rename atomically
    fs::write(&temp_file, &state_json)?;
    fs::rename(&temp_file, &state_file)?;
    
    Ok(())
}
```

#### Implementation Notes:
- Use atomic file operations to prevent corruption
- Write to temp file first, then rename
- This prevents partial writes from concurrent processes

### 5. Improve Test Coverage for Edge Cases
**Severity**: Low
**Category**: Testing
**File**: tests/
**Line**: N/A

#### Current Code:
Good test coverage but missing edge cases for:
- Interrupted worktree operations
- Concurrent workflow execution
- Invalid spec file formats
- Git operation failures

#### Required Change:
Add integration tests for:
```rust
#[test]
fn test_worktree_interrupted_merge() {
    // Test recovery from interrupted merge
}

#[test]
fn test_concurrent_workflow_execution() {
    // Test multiple workflows running simultaneously
}

#[test]
fn test_invalid_spec_file_handling() {
    // Test graceful handling of malformed spec files
}

#[test]
fn test_git_operation_failures() {
    // Test recovery from git command failures
}
```

#### Implementation Notes:
- Add tests to appropriate test files
- Use test fixtures for reproducible scenarios
- Test both success and failure paths

## Success Criteria
- [ ] Product management command is implemented and functional
- [ ] All error messages include proper context
- [ ] Public functions have complete documentation
- [ ] File operations are atomic to prevent corruption
- [ ] Edge case tests are added and passing
- [ ] All files compile without warnings
- [ ] Tests pass