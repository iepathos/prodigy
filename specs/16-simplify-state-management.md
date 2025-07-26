# Spec 16: Simplify State Management to Essentials

## Overview
Simplify the state management system to only track what's needed for the core self-sufficient loop.

## Current Problem
Even after removing learning, the state system is still over-engineered:
- Complex session tracking with detailed metrics
- Cache system that may not be needed
- Over-detailed improvement records
- Complex state structures

## What to Keep vs Remove

### Keep (Essential):
- Basic session history for debugging/audit
- Project analysis caching (since analysis is expensive)
- Simple current score tracking

### Remove (Over-engineered):
- Detailed session metrics (tokens, duration, claude_calls)
- Complex improvement categorization
- Session "active" state tracking
- File-level change tracking
- Impact scoring
- Most of the cache statistics

## Simplified State Structure

### Core State (state.rs):
```rust
pub struct State {
    pub version: String,
    pub project_id: String,
    pub current_score: f32,
    pub last_run: Option<DateTime<Utc>>,
    pub total_runs: u32,
}
```

### Simplified Session:
```rust
pub struct SessionRecord {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub initial_score: f32,
    pub final_score: Option<f32>,
    pub summary: String, // Simple description of what was done
}
```

### Minimal Cache (cache.rs):
Keep basic caching for expensive project analysis:
```rust
pub struct CacheManager {
    root: PathBuf,
    ttl: Duration,
}

impl CacheManager {
    pub fn get<T>(&self, key: &str) -> Result<Option<T>>
    pub fn set<T>(&self, key: &str, value: &T) -> Result<()>  
    pub fn clear(&self) -> Result<()>
}
```

Remove cache statistics and complex expiration logic.

## What to Remove

### From types.rs:
- `SessionMetrics` struct
- `Improvement` struct complexity
- `ProjectAnalysis` if too detailed
- Most learning-related types (per spec 13)
- Complex state statistics

### From state.rs:
- Session history complexity
- Detailed file tracking
- Impact measurements
- Active session management

### From cache.rs:
- Statistics tracking
- Complex expiration algorithms
- Multiple cache types

### From state_adapter.rs:
- Complex session lifecycle management
- Detailed metrics tracking
- Learning integration (per spec 13)
- Suggestion systems

## Simplified Usage

### StateManager usage:
```rust
// Just track basic info
let mut state_mgr = StateManager::new()?;
state_mgr.start_session(6.5)?;
// ... do improvements ...
state_mgr.complete_session(7.2, "Fixed error handling")?;
```

### Cache usage:
```rust
// Just cache expensive analysis
let cache = CacheManager::new()?;
if let Some(analysis) = cache.get("project_analysis")? {
    // Use cached
} else {
    let analysis = expensive_analysis().await?;
    cache.set("project_analysis", &analysis)?;
}
```

## Files to Modify

1. **Simplify types.rs** - Remove complex types
2. **Simplify state.rs** - Basic session tracking only  
3. **Simplify cache.rs** - Remove statistics
4. **Simplify state_adapter.rs** - Remove complexity
5. **Update tests** - Match new simplified structure

## Benefits
- Removes another ~300 lines of over-engineering
- State files become smaller and more readable
- Easier to understand and debug
- Less prone to bugs
- Faster startup (less JSON to parse)

## Keep Core Purpose
The simplified state system should:
1. Cache project analysis (expensive operation)
2. Track session history (for debugging)
3. Remember current score (for iteration decisions)
4. That's it!

## Success Criteria
- State JSON files are small and readable
- Only essential data is tracked
- Session management is trivial
- Cache is basic but functional
- No complex metrics or statistics
- Code is significantly simpler