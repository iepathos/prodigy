# Spec 18: Git-Native Dynamic Spec Generation for Code Improvements

## Overview
Bridge the gap between `/mmm-code-review` and `/mmm-implement-spec` using git commits as the communication mechanism. The review command generates temporary specification files and commits them, allowing the implement command to extract spec IDs from git history.

## Current Problem
The improvement loop has a mismatch:
- `/mmm-code-review` generates dynamic runtime issues  
- `/mmm-implement-spec` expects static spec numbers
- Previous JSON-based communication was fragile and hard to debug

## Solution
Implement a git-native workflow where `/mmm-code-review` generates temporary specification files and commits them, then `mmm improve` extracts spec IDs from git commit messages to pass to `/mmm-implement-spec`.

## Implementation Flow

### 1. Enhanced /mmm-code-review (Git-Native)
The review command will:
1. Perform normal code analysis
2. **Generate a temporary spec file** for issues found
3. **Commit the spec file** with a structured commit message
4. **Exit without JSON output** (git commit is the communication)

#### Temporary Spec Generation
```
specs/temp/iteration-{timestamp}-improvements.md
```

#### Git Commit Message Format
```
review: generate improvement spec for iteration-{timestamp}-improvements
```

Example generated spec:
```markdown
# Iteration 1: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Fix Error Handling in src/main.rs:42
**Severity**: High
**Category**: Error Handling
**Description**: Replace `.unwrap()` with proper error handling
**File**: src/main.rs
**Line**: 42

#### Current Code:
```rust
let config = load_config().unwrap();
```

#### Required Change:
```rust
let config = load_config().context("Failed to load configuration")?;
```

### 2. Add Unit Tests for Database Module  
**Severity**: Medium
**Category**: Testing
**Description**: Database functions lack test coverage
**File**: src/database.rs

#### Implementation:
Create `tests/database_tests.rs` with:
- Connection handling tests
- Error condition tests  
- Data validation tests

### 3. Optimize Loop in Parser
**Severity**: Low
**Category**: Performance
**File**: src/parser.rs
**Line Range**: 23-35

#### Current Code:
```rust
let mut results = Vec::new();
for item in items {
    if item.is_valid() {
        results.push(item.process());
    }
}
```

#### Required Change:
```rust
let results: Vec<_> = items
    .into_iter()
    .filter(|item| item.is_valid())
    .map(|item| item.process())
    .collect();
```

## Success Criteria
- [ ] Error handling fixed in src/main.rs
- [ ] Unit tests added for database module
- [ ] Parser loop optimized with iterators
- [ ] All files compile without warnings
- [ ] Tests pass
```

### 2. Git-Native Improvement Loop Logic

```rust
async fn run_improvement_iteration(&mut self) -> Result<bool> {
    // 1. Call Claude CLI for review (creates and commits spec)
    let review_success = self.call_claude_code_review().await?;
    if !review_success {
        return Ok(false); // Review failed
    }
    
    // 2. Extract spec ID from latest git commit
    let spec_id = self.extract_spec_from_git().await?;
    if spec_id.is_empty() {
        return Ok(false); // No issues found
    }
    
    // 3. Call implement-spec with the generated spec
    let implement_success = self.call_claude_implement_spec(&spec_id).await?;
    
    // 4. Call linting/formatting
    let _lint_success = self.call_claude_lint().await?;
    
    // 5. Re-analyze project
    let new_score = self.reanalyze_project().await?;
    
    Ok(implement_success)
}

async fn call_claude_code_review(&self) -> Result<bool> {
    let status = Command::new("claude")
        .arg("--dangerously-skip-permissions")
        .arg("/mmm-code-review")
        .env("MMM_AUTOMATION", "true")
        .status()
        .await?;
    Ok(status.success())
}

async fn extract_spec_from_git(&self) -> Result<String> {
    let output = Command::new("git")
        .args(&["log", "-1", "--pretty=format:%s"])
        .output()
        .await?;
        
    let commit_message = String::from_utf8_lossy(&output.stdout);
    
    // Parse commit message like "review: generate improvement spec for iteration-1234567890-improvements"
    if let Some(spec_start) = commit_message.find("iteration-") {
        let spec_part = &commit_message[spec_start..];
        if let Some(spec_end) = spec_part.find(' ') {
            Ok(spec_part[..spec_end].to_string())
        } else {
            Ok(spec_part.to_string())
        }
    } else {
        Ok(String::new()) // No spec found
    }
}

async fn call_claude_implement_spec(&self, spec_id: &str) -> Result<bool> {
    let status = Command::new("claude")
        .arg("--dangerously-skip-permissions") 
        .arg("/mmm-implement-spec")
        .arg(spec_id)
        .env("MMM_AUTOMATION", "true")
        .status()
        .await?;
    Ok(status.success())
}

async fn call_claude_lint(&self) -> Result<bool> {
    let status = Command::new("claude")
        .arg("--dangerously-skip-permissions")
        .arg("/mmm-lint")
        .env("MMM_AUTOMATION", "true")
        .status()
        .await?;
    Ok(status.success())
}
```

### 3. Enhanced /mmm-implement-spec
The implement command now includes git commit behavior:
- Reads spec files from `specs/` and `specs/temp/` directories
- Follows the same implementation pattern
- Commits changes with message: `fix: apply improvements from spec {spec-id}`
- Updates context files for permanent specs only

### 4. New /mmm-lint Integration
Added a new step in the improvement loop:
- Runs `cargo fmt`, `cargo clippy --fix`, and `cargo test`
- Commits formatting/linting changes if any
- Commit message: `style: apply automated formatting and lint fixes`

### 5. Complete Git History
Each iteration creates a clear sequence of commits:
```
* style: apply automated formatting and lint fixes
* fix: apply improvements from spec iteration-1708123456-improvements  
* review: generate improvement spec for iteration-1708123456-improvements
```

## Benefits

### 1. Git-Native Architecture
- Every step creates a commit for full auditability
- No fragile JSON parsing between commands
- Simple git log parsing to extract information
- Commands are independent and stateless

### 2. Debuggable
- Complete git history shows what happened
- Temporary specs are committed and visible
- Can inspect any step in the process
- Easy to revert or replay individual steps

### 3. Auditable
- Full paper trail through git history
- Specs are version controlled automatically
- Clear progression of improvements
- Easy to understand impact of each iteration

### 4. Robust
- No subprocess stdout/stderr parsing
- Simple git commands that are reliable
- Commands can be run independently for testing
- Graceful handling of command failures

## Implementation Steps

### Phase 1: Update Commands ✓
1. Modify /mmm-code-review to commit specs instead of JSON output
2. Create /mmm-lint command for automated formatting/testing
3. Update /mmm-implement-spec to always commit changes
4. Add automation mode behavior to all commands

### Phase 2: Update Improvement Loop ✓
1. Replace JSON parsing with git log parsing
2. Add git commit extraction logic
3. Integrate /mmm-lint step into the flow
4. Handle cases where no spec is generated

### Phase 3: Testing and Validation
1. Test complete end-to-end flow
2. Verify git history is clean and readable
3. Test error handling and edge cases
4. Validate temporary spec cleanup

## Success Criteria
- Review command generates actionable specs and commits them ✓
- Implement command successfully processes generated specs and commits changes ✓  
- Improvement loop works end-to-end with git-native communication ✓
- Git log parsing correctly extracts spec IDs ✓
- Linting step integrates seamlessly ✓
- File changes are applied correctly
- Temporary specs are human-readable and useful
- Complete git history provides full audit trail

This git-native approach eliminates JSON parsing complexity while creating a robust, auditable improvement flow through git commits.