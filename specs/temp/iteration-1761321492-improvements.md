# Iteration 1761321492: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Overly Complex Error Type
**Severity**: Medium
**Category**: Code Quality
**File**: src/error.rs
**Line**: 1-100

#### Current Code:
```rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    // ... 20+ error variants ...
    #[error("Internal error: {0}")]
    Internal(String),
}
```

#### Required Change:
Replace the complex custom error type with `anyhow::Error` throughout the codebase as specified in CONVENTIONS.md.

#### Implementation Notes:
- Remove the entire custom Error enum from src/error.rs
- Replace `use crate::error::{Error, Result};` with `use anyhow::{Context, Result};` in all files
- Update error creation to use `anyhow!()` or `.context()` instead of custom variants
- This aligns with the project's stated convention: "Use `anyhow::Result<T>` throughout"

### 2. Unused Claude API Client
**Severity**: High
**Category**: Dead Code
**File**: src/claude/api.rs
**Line**: 1-170

#### Current Code:
```rust
pub struct ClaudeClient {
    client: Client,
    api_key: String,
    max_retries: u32,
    retry_delay_ms: u64,
}
// ... full implementation ...
```

#### Required Change:
Remove the entire unused Claude API client module since the project uses Claude CLI subprocess calls instead.

#### Implementation Notes:
- Delete src/claude/api.rs entirely
- Remove the module reference from src/claude/mod.rs
- The project uses direct CLI subprocess calls via `Command::new("claude")` in improve/mod.rs
- This removes ~170 lines of unused code and the reqwest dependency

### 3. Overly Complex Module Structure
**Severity**: Medium
**Category**: Architecture
**File**: src/claude/
**Line**: Multiple files

#### Current Code:
The claude module has 9 files with overlapping responsibilities:
- api.rs (unused)
- cache.rs, commands.rs, context.rs, memory.rs, models.rs, prompt.rs, response.rs, token.rs

#### Required Change:
Consolidate or remove unused Claude modules since the project uses CLI subprocess calls.

#### Implementation Notes:
- Identify which modules are actually used by the improve command
- Remove or consolidate modules that aren't needed for CLI integration
- This will significantly simplify the codebase

### 4. Missing Test Coverage
**Severity**: High
**Category**: Testing
**File**: Multiple files
**Line**: N/A

#### Current Code:
Many modules lack test coverage:
- src/claude/* - No tests
- src/config/* - No tests
- src/project/* - No tests

#### Required Change:
Add unit tests for critical functionality, especially the improve loop and CLI integration.

#### Implementation Notes:
- Add tests for the improve command flow
- Add tests for git command parsing (extract_spec_from_git)
- Add tests for subprocess error handling
- Focus on integration points and error conditions

### 5. Inconsistent Error Handling
**Severity**: Medium
**Category**: Error Handling
**File**: src/improve/mod.rs
**Line**: 117-205

#### Current Code:
```rust
let status = cmd
    .status()
    .await
    .context("Failed to execute Claude CLI for review")?;
```

#### Required Change:
Add proper error recovery and user-friendly error messages for subprocess failures.

#### Implementation Notes:
- Check if `claude` command exists before running
- Provide helpful error messages if Claude CLI is not installed
- Add retry logic for transient failures
- Log subprocess stderr for debugging

### 6. Documentation Improvements
**Severity**: Low
**Category**: Documentation
**File**: Multiple files
**Line**: N/A

#### Current Code:
Many public functions lack documentation comments.

#### Required Change:
Add rustdoc comments to all public APIs.

#### Implementation Notes:
- Add module-level documentation explaining purpose
- Document public functions with examples
- Add error conditions to function docs
- Focus on user-facing APIs in lib.rs

### 7. Potential Race Condition in Git Operations
**Severity**: Medium
**Category**: Concurrency
**File**: src/improve/mod.rs
**Line**: 143-167

#### Current Code:
```rust
let output = Command::new("git")
    .args(["log", "-1", "--pretty=format:%s"])
    .output()
    .await
```

#### Required Change:
Ensure git operations are atomic and handle concurrent modifications.

#### Implementation Notes:
- Add file locks or mutex around git operations
- Check git status before operations
- Handle cases where another process modifies git state
- Add proper error handling for git command failures

## Success Criteria
- [ ] Replace custom Error type with anyhow throughout codebase
- [ ] Remove unused Claude API client and consolidate modules
- [ ] Add test coverage for critical paths (>50% coverage)
- [ ] Improve error handling for subprocess calls
- [ ] Add documentation to public APIs
- [ ] Ensure thread-safe git operations
- [ ] All files compile without warnings
- [ ] Tests pass