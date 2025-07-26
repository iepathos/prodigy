# MMM Improvement Loop - User Flow & Architecture

## User Experience

```
$ mmm improve
Analyzing project...
✓ Rust project detected (Score: 6.8/10)

Running improvements...
🔄 Iteration 1/10...
✓ Fixed error handling in src/main.rs  
✓ Added unit tests for database module
Score: 6.8 → 7.3

🔄 Iteration 2/10...
✓ Optimized parser loops with iterators
✓ Improved documentation coverage  
Score: 7.3 → 7.9

✓ Improvements complete!
Score: 6.8 → 7.9 (+1.1)
Files changed: 6
Iterations: 2
```

## Technical Architecture Flow

```
┌─────────────────┐
│ User runs       │
│ mmm improve     │
└─────────┬───────┘
          │
          ▼
┌─────────────────┐
│ 1. Project      │
│ Analysis        │
│ (Language,      │
│ Framework,      │
│ Health Score)   │
└─────────┬───────┘
          │
          ▼
┌─────────────────┐
│ 2. Start        │
│ Session         │
│ (Record initial │
│ score & state)  │
└─────────┬───────┘
          │
          ▼
    ┌─────────────┐
    │ ITERATION   │
    │ LOOP        │
    │ (Max 10x)   │
    └─────┬───────┘
          │
          ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│ 3. Call Claude  │    │ Generate         │    │ Save Spec File  │
│ /mmm-code-review│───▶│ Temporary Spec   │───▶│ specs/temp/     │
│ --format=json   │    │ (if issues found)│    │ iteration-*.md  │
└─────────┬───────┘    └──────────────────┘    └─────────────────┘
          │
          ▼
┌─────────────────┐
│ 4. Parse JSON   │
│ Response        │
│ Extract:        │
│ - issues[]      │
│ - generated_spec│
│ - overall_score │
└─────────┬───────┘
          │
          ▼
     ┌──────────┐
     │ Issues   │
     │ Found?   │
     └────┬─────┘
          │
    ┌─────▼─────┐         ┌─────────────┐
    │    YES    │         │     NO      │
    │           │         │             │
    │     ▼     │         │      ▼      │
┌─────────────────┐       │  ┌─────────────────┐
│ 5. Call Claude  │       │  │ 8. END LOOP     │
│ /mmm-implement- │       │  │ (Target reached │
│ spec {spec_id}  │       │  │ or no issues)   │
└─────────┬───────┘       │  └─────────────────┘
          │               │            │
          ▼               │            ▼
┌─────────────────┐       │  ┌─────────────────┐
│ 6. Apply        │       │  │ 9. Complete     │
│ File Changes    │       │  │ Session         │
│ - Modified:     │       │  │ (Save final     │
│ - Created:      │       │  │ score & summary)│
│ - Deleted:      │       │  └─────────────────┘
└─────────┬───────┘       │
          │               │
          ▼               │
┌─────────────────┐       │
│ 7. Re-analyze   │       │
│ Project         │       │
│ (Get new score) │       │
└─────────┬───────┘       │
          │               │
          ▼               │
     ┌──────────┐         │
     │ Score >=  │         │
     │ Target?   │         │
     └────┬─────┘         │
          │               │
    ┌─────▼─────┐         │
    │    NO     │         │
    │           │         │
    │     ▼     │         │
    └───────────┘         │
          │               │
          └───────────────┘
                (Loop back to iteration)
```

## Detailed Information Flow

### Phase 1: Setup
```
mmm improve --target 8.0
├── Analyze project structure
├── Detect language (Rust) & framework  
├── Calculate initial health score (6.8)
├── Load/create state (.mmm/state.json)
└── Start improvement session
```

### Phase 2: Iteration Loop (Repeat until target reached)
```
Iteration N:
├── Call: claude --dangerously-skip-permissions /mmm-code-review --format=json
│   ├── Claude analyzes current code
│   ├── Finds issues (error handling, missing tests, etc.)
│   ├── Generates: specs/temp/iteration-1708123456-improvements.md
│   └── Returns: {"generated_spec": "iteration-1708123456-improvements", ...}
│
├── Parse JSON response
│   ├── Extract spec identifier
│   ├── Check if issues found
│   └── Get new score estimate
│
├── Call: claude --dangerously-skip-permissions /mmm-implement-spec iteration-1708123456-improvements  
│   ├── Claude reads the temporary spec
│   ├── Applies specific fixes listed in spec
│   ├── Modifies actual files
│   └── Reports: "Modified: src/main.rs", "Created: tests/db_test.rs"
│
├── Re-analyze project
│   ├── Run project analyzer again
│   ├── Calculate new health score
│   └── Update session state
│
└── Check termination conditions
    ├── Score >= target (8.0)? → END
    ├── No issues found? → END  
    ├── Max iterations (10)? → END
    └── Otherwise → CONTINUE
```

### Phase 3: Completion
```
Session Complete:
├── Save final session record to .mmm/history/
├── Update .mmm/state.json with new score
├── Display results to user
└── Exit
```

## File System State Changes

### During Operation:
```
.mmm/
├── state.json                    # Updated with current score
├── history/
│   └── 20250126_143052_abc123.json  # Session record  
└── cache/
    └── project_analysis.json    # Cached analysis

specs/temp/
├── iteration-1708123456-improvements.md  # Generated by review
└── iteration-1708123789-improvements.md  # Next iteration
```

### Example Temporary Spec Generated:
```markdown
# Iteration 1: Code Quality Improvements

## Issues to Address

### 1. Fix Error Handling in src/main.rs:42
**Severity**: High
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
**File**: src/database.rs
**Description**: Create comprehensive test coverage

#### Implementation:
Create `tests/database_test.rs` with connection and error tests.

## Success Criteria
- [ ] Error handling fixed in src/main.rs
- [ ] Unit tests added for database module
- [ ] All files compile without warnings
```

## Error Handling & Edge Cases

### No Issues Found:
```
Review → No issues → Skip implement → Re-analyze → Complete
```

### Claude CLI Failures:
```
Review fails → Log error → Try once more → Fail gracefully
Implement fails → Rollback changes → Continue loop
```

### Target Already Reached:
```
Initial score 8.2 → Target 8.0 → Skip loop → Complete immediately
```

## Key Benefits of This Design

1. **Clean Information Flow**: Review generates specs, implement consumes specs
2. **Debuggable**: Temporary specs are human-readable files  
3. **Auditable**: Complete paper trail of what was done
4. **Resumable**: State persisted at each step
5. **Safe**: Each iteration is contained and can be rolled back
6. **Self-Sufficient**: No manual intervention required

This architecture creates a true self-sufficient improvement loop that actually modifies code and tracks progress through Claude CLI integration.