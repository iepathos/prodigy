# Spec 15: Remove Developer Experience Bloat

## Overview
Remove premature developer experience enhancements that add complexity before core functionality works.

## Current Problem
The codebase has extensive developer experience features that:
- Don't improve core functionality
- Add dependencies and complexity
- Distract from getting the basic loop working
- Are aspirational rather than essential

## What to Remove

### 1. Remove src/developer_experience/ entirely
- Beautiful progress displays with animations
- Interactive preview mode  
- Gamification and achievements
- Shell integrations
- Performance optimizations
- Error handling with rollback

All of this is premature optimization before the core loop works.

### 2. Simplify ImproveCommand arguments
Current bloat:
```rust
pub struct ImproveCommand {
    pub focus: Option<String>,
    pub target: f32,
    pub auto_commit: bool,
    pub dry_run: bool,
    pub verbose: bool,
    pub preview: bool,        // REMOVE
    pub resume: bool,         // REMOVE  
    pub conservative: bool,   // REMOVE
    pub quick: bool,          // REMOVE
}
```

Keep only essential:
```rust
pub struct ImproveCommand {
    pub target: f32,
    pub verbose: bool,
}
```

### 3. Simplify display.rs
Remove fancy progress spinners, just use:
- Simple println! statements
- Basic progress indication
- Essential feedback only

Current display.rs has:
- `ProgressSpinner` with animations
- Complex formatting
- Interactive elements

Replace with basic logging.

### 4. Remove complex session types
From session.rs, remove:
- `ImprovementType` enum (8 variants)
- Complex `Improvement` struct with old/new content
- `Interactive` features
- `Preview` mode handling
- `Resume` functionality

Keep simple session tracking for history only.

### 5. Remove aspirational CLI features
- Shell completions
- Git hook integration  
- Advanced error handling
- Rollback mechanisms
- Performance profiling

## Keep Essential Only

### Core CLI:
```rust
mmm improve [--target 8.0] [--verbose]
```

### Core display:
```
Analyzing project...
Running improvements... (iteration 1/10)
✓ Fixed error handling in src/main.rs
✓ Added tests in tests/
Score: 6.5 → 7.2
Done. Improved 3 files in 2 iterations.
```

### Core session data:
```rust
pub struct SessionRecord {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub initial_score: f32,
    pub final_score: Option<f32>,
    pub files_changed: Vec<String>,
    pub description: String, // Simple summary
}
```

## Implementation Steps

1. **Delete src/developer_experience/ module**
2. **Simplify ImproveCommand struct**
3. **Replace display.rs with basic logging**
4. **Simplify session.rs types**
5. **Remove complex CLI arguments**
6. **Update lib.rs imports**
7. **Remove unused dependencies from Cargo.toml**

## Dependencies to Remove
- `indicatif` (progress bars)
- `colored` (terminal colors)  
- `ctrlc` (signal handling)
- Any other UI/UX specific crates

Keep only:
- `clap` for basic CLI
- `anyhow` for errors
- `tokio` for async
- Core functionality crates

## Benefits
- Removes ~1000+ lines of aspirational code
- Eliminates complex UI dependencies
- Focuses on core functionality
- Makes codebase much simpler to understand
- Easier to debug and maintain

## Success Criteria
- No src/developer_experience/ module
- ImproveCommand has 2 fields max
- Basic println! output only
- No fancy progress indicators
- Core loop functionality unchanged
- Significantly reduced code complexity