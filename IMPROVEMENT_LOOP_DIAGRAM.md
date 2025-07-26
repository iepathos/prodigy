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

## Git-Native Architecture Flow

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
│ 3. Call Claude  │    │ Generate Spec &  │    │ Git Commit:     │
│ /mmm-code-review│───▶│ Write to         │───▶│ "review: gen    │
│                 │    │ specs/temp/      │    │ spec iteration-*│
└─────────┬───────┘    └──────────────────┘    └─────────────────┘
          │
          ▼
┌─────────────────┐
│ 4. Parse Git    │
│ Log for Spec    │
│ git log -1      │
│ --pretty=%s     │
│ Extract spec ID │
└─────────┬───────┘
          │
          ▼
     ┌──────────┐
     │ Spec ID  │
     │ Found?   │
     └────┬─────┘
          │
    ┌─────▼─────┐         ┌─────────────┐
    │    YES    │         │     NO      │
    │           │         │             │
    │     ▼     │         │      ▼      │
┌─────────────────┐       │  ┌─────────────────┐
│ 5. Call Claude  │       │  │ 8. END LOOP     │
│ /mmm-implement- │       │  │ (No issues or   │
│ spec {spec_id}  │       │  │ target reached) │
└─────────┬───────┘       │  └─────────────────┘
          │               │            │
          ▼               │            ▼
┌─────────────────┐       │  ┌─────────────────┐
│ 6. Apply Fixes  │       │  │ 10. Complete    │
│ & Git Commit:   │       │  │ Session         │
│ "fix: apply     │       │  │ (Save final     │
│ spec {spec_id}" │       │  │ score & summary)│
└─────────┬───────┘       │  └─────────────────┘
          │               │
          ▼               │
┌─────────────────┐       │
│ 7. Call Claude  │       │
│ /mmm-lint       │       │
│ & Git Commit:   │       │
│ "style: format  │       │
│ and lint fixes" │       │
└─────────┬───────┘       │
          │               │
          ▼               │
┌─────────────────┐       │
│ 8. Re-analyze   │       │
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

### Phase 2: Git-Native Iteration Loop (Repeat until target reached)
```
Iteration N:
├── Call: claude --dangerously-skip-permissions /mmm-code-review
│   ├── Claude analyzes current code
│   ├── Finds issues (error handling, missing tests, etc.)
│   ├── Generates: specs/temp/iteration-1708123456-improvements.md
│   └── Commits: "review: generate improvement spec for iteration-1708123456-improvements"
│
├── Parse git log for spec ID
│   ├── Run: git log -1 --pretty=format:"%s"
│   ├── Extract spec ID from commit message
│   └── Check if spec was generated (or no issues found)
│
├── Call: claude --dangerously-skip-permissions /mmm-implement-spec iteration-1708123456-improvements  
│   ├── Claude reads the temporary spec
│   ├── Applies specific fixes listed in spec
│   ├── Modifies actual files
│   └── Commits: "fix: apply improvements from spec iteration-1708123456-improvements"
│
├── Call: claude --dangerously-skip-permissions /mmm-lint
│   ├── Runs cargo fmt, clippy, test
│   ├── Fixes any automated issues
│   └── Commits: "style: apply automated formatting and lint fixes" (if changes)
│
├── Re-analyze project
│   ├── Run project analyzer again
│   ├── Calculate new health score
│   └── Update session state
│
└── Check termination conditions
    ├── Score >= target (8.0)? → END
    ├── No spec generated (no issues)? → END  
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

## File System & Git State Changes

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

Git History:
* style: apply automated formatting and lint fixes
* fix: apply improvements from spec iteration-1708123789-improvements  
* review: generate improvement spec for iteration-1708123789-improvements
* style: apply automated formatting and lint fixes
* fix: apply improvements from spec iteration-1708123456-improvements
* review: generate improvement spec for iteration-1708123456-improvements
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