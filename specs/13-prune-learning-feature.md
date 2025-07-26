# Spec 13: Prune Learning Feature

## Overview
Remove the over-engineered learning system to focus on core functionality and dead simple developer experience.

## Motivation
The learning manager adds unnecessary complexity:
- Aspirational features that aren't actually used
- Complex pattern tracking that doesn't improve the core loop
- Additional state management overhead
- Confuses the simple JSON state story

## What to Remove

### 1. Remove LearningManager entirely
- Delete `src/simple_state/learning.rs`
- Remove all learning-related types from `types.rs`
- Remove learning references from state adapter

### 2. Simplify State Types
Remove from `types.rs`:
- `Learning` struct
- `PatternInfo` struct  
- `FailureInfo` struct
- `Preferences` struct
- All learning-related fields

### 3. Simplify SessionRecord
Keep only essential fields:
- `session_id`
- `started_at`
- `completed_at`
- `initial_score`
- `final_score`
- `files_changed`
- `improvements` (simplified)

### 4. Simplify Improvement
Reduce to:
```rust
pub struct Improvement {
    pub file: String,
    pub description: String,
}
```

No need for:
- `improvement_type` categorization
- `impact` scoring
- `line` numbers

### 5. Update State Adapter
Remove from `state_adapter.rs`:
- `learning_mgr` field
- All learning-related methods
- `get_suggestions()`
- `learning_summary()`

## What to Keep

### Essential State Management
- StateManager for session history
- CacheManager for temporary data (project analysis)
- Simple session recording
- Basic improvement tracking (just for history)

### Core Workflow
1. Analyze project → cache result
2. Run improvements → record session
3. Save history for debugging/audit
4. That's it!

## Implementation Steps

1. **Delete learning.rs**
2. **Simplify types.rs** - remove all learning types
3. **Update state.rs** - remove learning references
4. **Update state_adapter.rs** - remove learning integration
5. **Update tests** - remove learning tests
6. **Update improve command** - remove suggestion features

## Benefits
- Removes ~500 lines of aspirational code
- Simplifies mental model
- Focuses on working features
- Makes state truly "dead simple"
- Reduces maintenance burden

## Migration
No migration needed - learning.json files can be ignored/deleted.

## Success Criteria
- All learning code removed
- Tests still pass
- State management remains functional
- Improve command works without suggestions
- Code is significantly simpler