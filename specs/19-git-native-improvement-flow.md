# Spec 19: Git-Native Improvement Flow

## Overview
Replace JSON-based communication between Claude commands with git-native commits and spec files. Each step in the improvement loop creates commits that can be tracked and audited.

## Current Problem
The previous approach relied on:
- Complex JSON parsing between commands
- Subprocess stdout/stderr handling  
- Memory-based information passing
- Fragile inter-command communication

## New Git-Native Flow

### 1. Code Review → Spec Generation & Commit
```
mmm improve calls: claude /mmm-code-review
↓
/mmm-code-review:
  - Analyzes code
  - Creates specs/temp/iteration-{timestamp}-improvements.md
  - Commits spec file: "review: generate improvement spec for iteration {N}"
  - Exits (no JSON output needed)
```

### 2. Extract Spec ID from Git History
```
mmm improve:
  - Runs: git log -1 --pretty=format:"%s" 
  - Parses commit message to extract spec ID
  - Locates spec file in specs/temp/
```

### 3. Implementation → File Changes & Commit  
```
mmm improve calls: claude /mmm-implement-spec {spec-id}
↓
/mmm-implement-spec:
  - Reads specs/temp/{spec-id}.md
  - Applies fixes to files
  - Commits changes: "fix: apply improvements from spec {spec-id}"
  - Exits
```

### 4. Linting & Validation → Commit
```
mmm improve calls: claude /mmm-lint
↓  
/mmm-lint:
  - Runs cargo fmt, clippy, test
  - Fixes any formatting/lint issues
  - Commits if changes: "style: apply automated formatting and lint fixes"
  - Exits
```

### 5. Re-analysis → Loop Decision
```
mmm improve:
  - Re-analyzes project health score
  - Compares with target
  - If target reached or max iterations: END
  - Otherwise: Loop back to step 1
```

## Updated Loop Implementation

```rust
async fn run_improvement_loop(&mut self) -> Result<SessionResult> {
    let mut iteration = 1;
    
    while !self.is_target_reached() && iteration <= 10 {
        println!("🔄 Iteration {}/10...", iteration);
        
        // 1. Generate review spec and commit
        self.call_claude_code_review().await?;
        
        // 2. Extract spec ID from latest commit  
        let spec_id = self.extract_spec_from_git().await?;
        
        if spec_id.is_empty() {
            println!("No issues found - stopping iterations");
            break;
        }
        
        // 3. Implement fixes and commit
        self.call_claude_implement_spec(&spec_id).await?;
        
        // 4. Run linting/formatting and commit
        self.call_claude_lint().await?;
        
        // 5. Re-analyze project
        let new_score = self.reanalyze_project().await?;
        self.update_score(new_score);
        
        iteration += 1;
    }
    
    Ok(self.create_session_result())
}

async fn call_claude_code_review(&self) -> Result<()> {
    Command::new("claude")
        .arg("--dangerously-skip-permissions")
        .arg("/mmm-code-review")
        .status()
        .await?;
    Ok(())
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
            return Ok(spec_part[..spec_end].to_string());
        } else {
            return Ok(spec_part.to_string());
        }
    }
    
    Ok(String::new()) // No spec found
}

async fn call_claude_implement_spec(&self, spec_id: &str) -> Result<()> {
    Command::new("claude")
        .arg("--dangerously-skip-permissions") 
        .arg("/mmm-implement-spec")
        .arg(spec_id)
        .status()
        .await?;
    Ok(())
}

async fn call_claude_lint(&self) -> Result<()> {
    Command::new("claude")
        .arg("--dangerously-skip-permissions")
        .arg("/mmm-lint")
        .status()
        .await?;
    Ok(())
}
```

## Updated Claude Commands

### /mmm-code-review Changes
- Remove JSON output entirely
- Focus on creating and committing spec files
- Commit message format: `review: generate improvement spec for iteration-{timestamp}-improvements`
- Exit after commit (no stdout parsing needed)

### /mmm-implement-spec Changes  
- No JSON output needed
- Focus on applying fixes and committing changes
- Commit message format: `fix: apply improvements from spec {spec-id}`
- List modified files in commit body

### /mmm-lint (New Command)
- Run cargo fmt, clippy, test
- Fix any automated issues
- Commit if changes made: `style: apply automated formatting and lint fixes`
- Handle test failures gracefully

## Git History Example

```
* style: apply automated formatting and lint fixes
* fix: apply improvements from spec iteration-1708123789-improvements  
* review: generate improvement spec for iteration-1708123789-improvements
* style: apply automated formatting and lint fixes
* fix: apply improvements from spec iteration-1708123456-improvements
* review: generate improvement spec for iteration-1708123456-improvements
* feat: initial project setup
```

## Benefits

### 1. Git-Native Workflow
- Every step creates a commit
- Full audit trail of what happened
- Easy to understand progression
- Can inspect/revert individual changes

### 2. Simplified Implementation
- No JSON parsing
- No complex subprocess stdout handling
- Simple git log parsing
- Commands are independent

### 3. Debuggable
- Each spec file is committed and visible
- Git history shows exact progression
- Can manually inspect/replay any step
- Clear failure points

### 4. Resumable
- Can resume from any point in git history
- Commands are idempotent
- State is in git, not memory

## Error Handling

### Command Failures
- If /mmm-code-review fails: Stop iteration, report error
- If /mmm-implement-spec fails: Continue (may be partial improvement)
- If /mmm-lint fails: Continue (fixes may still be valuable)

### Git Operations
- Always check git status before starting
- Ensure clean working directory
- Handle git conflicts gracefully
- Provide clear error messages

## Implementation Steps

1. **Update /mmm-code-review** to commit specs instead of JSON output
2. **Create /mmm-lint** command for formatting/linting
3. **Update /mmm-implement-spec** commit behavior
4. **Implement git log parsing** in improvement loop
5. **Remove JSON parsing** from session.rs
6. **Update error handling** for git-based flow

This approach is much simpler, more reliable, and creates a beautiful git history that tells the story of the improvement process.