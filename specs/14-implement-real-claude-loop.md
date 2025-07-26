# Spec 14: Implement Real Claude Self-Sufficient Loop

## Overview
Replace simulation placeholders with actual Claude CLI integration to create a working self-sufficient improvement loop.

## Current Problem
The `mmm improve` command is all scaffolding with no real functionality:
- `simulate_review()` returns fake data
- `simulate_improvements()` returns fake changes
- No actual Claude CLI calls
- No actual file modifications
- No real iterative improvement

## Solution
Implement the actual loop that calls Claude CLI commands and applies changes.

## Core Loop Implementation

### 1. Replace session.run() with real loop
```rust
pub async fn run(&mut self) -> Result<SessionResult> {
    while !self.is_good_enough() && self.iterations.len() < 10 {
        // 1. Call claude mmm-code-review with project context
        let review = self.call_claude_review().await?;
        
        // 2. If issues found, call claude mmm-implement-spec 
        if !review.issues.is_empty() {
            let changes = self.call_claude_implement(review).await?;
            
            // 3. Apply changes to actual files
            self.apply_changes(changes).await?;
            
            // 4. Re-analyze project to get new score
            let new_score = self.reanalyze_project().await?;
            self.state.current_score = new_score;
        }
        
        self.iterations.push(create_iteration_record());
    }
}
```

### 2. Implement Claude CLI integration
- `call_claude_review()` - Execute `claude mmm-code-review` command
- `call_claude_implement()` - Execute `claude mmm-implement-spec` command  
- Parse structured output from Claude CLI commands
- Handle Claude CLI errors gracefully

### 3. Implement file modification
- `apply_changes()` - Actually modify files based on Claude's suggestions
- Create backup before changes
- Validate changes don't break syntax
- Track which files were modified

### 4. Implement project re-analysis
- Re-run project analyzer after changes
- Calculate new health score
- Update focus areas based on improvements

## Implementation Details

### Claude CLI Command Execution
```rust
async fn call_claude_review(&self) -> Result<ReviewResult> {
    let context = self.build_review_context();
    let output = Command::new("claude")
        .arg("mmm-code-review")
        .arg("--")  
        .arg(&context)
        .output()
        .await?;
    
    self.parse_review_output(&output.stdout)
}
```

### File Change Application
```rust
async fn apply_changes(&mut self, changes: Vec<Change>) -> Result<()> {
    for change in changes {
        // Create backup
        self.backup_file(&change.file)?;
        
        // Apply change
        match change.change_type {
            ChangeType::Modify => self.modify_file(&change)?,
            ChangeType::Add => self.create_file(&change)?,
            ChangeType::Delete => self.delete_file(&change)?,
        }
        
        // Validate syntax
        self.validate_file_syntax(&change.file)?;
    }
}
```

## Remove Simulation Code

### Delete from session.rs:
- `simulate_review()`
- `simulate_improvements()` 
- All hardcoded fake data
- Placeholder comments about "real implementation"

### Simplify data structures:
- Remove complex Issue/Change types if not needed
- Focus on essential data only

## Integration with Existing Claude Commands

### Use existing .claude/commands/:
- `mmm-code-review.md` - for code analysis
- `mmm-implement-spec.md` - for applying improvements
- Ensure commands return structured output for parsing

### Command output format:
Commands should return JSON with:
```json
{
  "score": 7.5,
  "issues": [...],
  "improvements": [...],
  "files_to_change": [...]
}
```

## Error Handling
- Handle Claude CLI not being installed
- Handle Claude API failures gracefully
- Rollback changes if syntax errors occur
- Continue loop even if individual iteration fails

## Success Criteria
- `mmm improve` makes real code changes
- Uses actual Claude CLI commands
- Iterates until target score reached
- Files are actually modified on disk
- Score genuinely improves through iterations
- No more simulation/placeholder code