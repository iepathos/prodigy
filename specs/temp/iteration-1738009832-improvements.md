# Iteration 1: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Uninlined Format Arguments
**Severity**: Low
**Category**: Code Quality
**File**: src/improve/mod.rs
**Line**: 201, 206

#### Current Code:
```rust
eprintln!("Warning: Failed to commit .mmm/state.json: {}", stderr);
```

#### Required Change:
```rust
eprintln!("Warning: Failed to commit .mmm/state.json: {stderr}");
```

#### Implementation Notes:
- Use inline format arguments as suggested by clippy
- Apply to both occurrences on lines 201 and 206

### 2. Missing Error Handling Documentation
**Severity**: Medium
**Category**: Documentation
**File**: src/improve/mod.rs
**Line**: Multiple

#### Current Code:
Functions like `call_claude_code_review`, `call_claude_implement_spec`, and `call_claude_lint` have basic error handling but lack comprehensive documentation about error scenarios.

#### Required Change:
Add documentation comments explaining:
- Specific error conditions that can occur
- How errors are propagated
- Recovery strategies for transient failures

#### Implementation Notes:
- Add /// documentation comments above each function
- Include error conditions in doc comments
- Document retry behavior and transient error handling

### 3. Potential Race Condition in Git Operations
**Severity**: High
**Category**: Thread Safety
**File**: src/improve/mod.rs
**Line**: 37

#### Current Code:
```rust
/// # Thread Safety
/// This function performs git operations sequentially and is not designed for concurrent
/// execution. If running multiple instances, ensure they operate on different repositories
/// to avoid git conflicts.
pub async fn run(cmd: command::ImproveCommand) -> Result<()> {
```

#### Required Change:
Implement file-based locking mechanism to prevent concurrent executions on the same repository:
```rust
use std::fs::File;
use std::io::prelude::*;

// At the beginning of run():
let lock_path = Path::new(".mmm").join("improve.lock");
let lock_file = File::create(&lock_path)
    .context("Failed to create lock file")?;

// Use file locking or check for lock existence
// Clean up lock file on exit
```

#### Implementation Notes:
- Create a lock file in .mmm directory
- Check for existing lock before proceeding
- Clean up lock file on both success and error paths
- Consider using a crate like `fs2` for proper file locking

### 4. Hardcoded Retry Count
**Severity**: Medium
**Category**: Configuration
**File**: src/improve/mod.rs
**Line**: 234, 290, 321

#### Current Code:
```rust
execute_with_retry(cmd, "Claude code review", 2, verbose).await?;
```

#### Required Change:
Make retry count configurable through constants:
```rust
const DEFAULT_CLAUDE_RETRIES: u32 = 2;

// Then use:
execute_with_retry(cmd, "Claude code review", DEFAULT_CLAUDE_RETRIES, verbose).await?;
```

#### Implementation Notes:
- Define a constant at module level
- Consider making it configurable in the future
- Apply to all three Claude command calls

### 5. Missing Test Coverage for Core Functions
**Severity**: High
**Category**: Testing
**File**: src/improve/mod.rs
**Line**: Various

#### Current Code:
Core functions like `call_claude_code_review`, `extract_spec_from_git`, etc. lack unit tests.

#### Required Change:
Add unit tests covering:
- Success scenarios
- Error scenarios
- Edge cases (empty spec, malformed commit messages)

#### Implementation Notes:
- Add tests in the existing tests module
- Mock subprocess calls for testing
- Test error handling paths
- Test git operation parsing

### 6. Insufficient Input Validation
**Severity**: Medium
**Category**: Security
**File**: src/improve/mod.rs
**Line**: 279

#### Current Code:
```rust
async fn call_claude_implement_spec(spec_id: &str, verbose: bool) -> Result<bool> {
    println!("🔧 Running /mmm-implement-spec {spec_id}...");
```

#### Required Change:
Validate spec_id format before using in commands:
```rust
// Validate spec_id matches expected pattern
if !spec_id.starts_with("iteration-") || !spec_id.ends_with("-improvements") {
    return Err(anyhow!("Invalid spec ID format: {}", spec_id));
}
```

#### Implementation Notes:
- Add validation at the start of function
- Use regex or pattern matching for validation
- Prevent potential command injection

## Success Criteria
- [ ] All clippy warnings resolved
- [ ] Thread safety issue addressed with proper locking
- [ ] Retry counts made configurable
- [ ] Unit tests added for core functions
- [ ] Input validation implemented
- [ ] All files compile without warnings
- [ ] Tests pass