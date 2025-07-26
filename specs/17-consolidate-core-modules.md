# Spec 17: Consolidate Core Modules

## Overview
Consolidate the scattered improve functionality into a single cohesive module focused on the core loop.

## Current Problem
The improve functionality is spread across too many files:
- `improve/command.rs` - CLI handling
- `improve/session.rs` - Complex session management  
- `improve/display.rs` - Fancy UI (to be removed)
- `improve/analyzer.rs` - Duplicates `analyzer/` module
- `improve/context.rs` - Context building
- `improve/state_adapter.rs` - Bridge to state system

This creates confusion and makes the core loop hard to follow.

## Consolidation Plan

### 1. Merge into improve/mod.rs
Create a single `improve/mod.rs` that contains:
- Core loop implementation
- Claude CLI integration
- File modification logic
- Basic progress output

### 2. Remove Redundant Files

#### Delete entirely:
- `improve/display.rs` (fancy UI)
- `improve/analyzer.rs` (use main `analyzer/` instead)
- `improve/context.rs` (integrate into main logic)
- `improve/state_adapter.rs` (direct state usage)

#### Keep and simplify:
- `improve/command.rs` (just CLI args)
- `improve/session.rs` (basic session data only)

### 3. Single Core Loop Structure

```rust
// improve/mod.rs
pub mod command;
pub mod session;

use crate::analyzer::ProjectAnalyzer;
use crate::simple_state::StateManager;

pub async fn run(cmd: command::ImproveCommand) -> Result<()> {
    println!("🔍 Analyzing project...");
    
    // 1. Initial analysis
    let analysis = ProjectAnalyzer::analyze(".").await?;
    let mut current_score = analysis.health_score;
    
    println!("Current score: {:.1}/10", current_score);
    
    if current_score >= cmd.target {
        println!("✅ Target already reached!");
        return Ok(());
    }
    
    // 2. State setup
    let mut state = StateManager::new()?;
    let session = state.start_session(current_score);
    
    // 3. Improvement loop
    let mut iteration = 1;
    while current_score < cmd.target && iteration <= 10 {
        println!("🔄 Iteration {}/10...", iteration);
        
        // Call Claude CLI for review and implementation
        let improved = call_claude_improve(&analysis).await?;
        if improved {
            // Re-analyze to get new score
            let new_analysis = ProjectAnalyzer::analyze(".").await?;
            current_score = new_analysis.health_score;
            println!("Score: {:.1}/10", current_score);
        }
        
        iteration += 1;
    }
    
    // 4. Completion
    state.complete_session(session, current_score, "Automated improvements")?;
    println!("✅ Complete! Final score: {:.1}/10", current_score);
    
    Ok(())
}

async fn call_claude_improve(analysis: &AnalyzerResult) -> Result<bool> {
    // Call claude mmm-code-review and mmm-implement-spec
    // Return true if changes were made
    todo!("Implement Claude CLI calls")
}
```

### 4. Simplified Command
```rust
// improve/command.rs
#[derive(Debug, Args)]
pub struct ImproveCommand {
    /// Target quality score (default: 8.0)
    #[arg(long, default_value = "8.0")]
    pub target: f32,
    
    /// Show detailed progress
    #[arg(short, long)]
    pub verbose: bool,
}
```

### 5. Minimal Session
```rust  
// improve/session.rs
pub struct SessionSummary {
    pub initial_score: f32,
    pub final_score: f32,
    pub iterations: usize,
    pub files_changed: usize,
}
```

## Benefits of Consolidation

### Clarity:
- Single file contains the core loop
- Easy to understand the flow
- No jumping between multiple abstractions

### Simplicity:
- Direct usage of analyzer and state modules
- No complex adapter layers
- Straightforward Claude CLI integration

### Maintainability:
- Less code to maintain
- Fewer abstractions to understand
- Clear separation of concerns

## Implementation Steps

1. **Create consolidated improve/mod.rs**
2. **Delete redundant files**
3. **Simplify command.rs**
4. **Simplify session.rs**
5. **Update main.rs imports**
6. **Remove old module references**

## File Structure After Consolidation
```
src/improve/
├── mod.rs          # Core loop implementation
├── command.rs      # CLI args only
└── session.rs      # Basic session data
```

## Success Criteria
- improve/ module has 3 files max
- Core loop is in single function
- No complex abstractions
- Direct integration with analyzer and state
- Easy to follow code flow
- Significant reduction in complexity