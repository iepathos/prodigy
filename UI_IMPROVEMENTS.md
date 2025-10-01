# Prodigy Terminal UI Improvements - Detailed Mockups

## 1. `prodigy worktree ls` - List Worktrees

### Current Implementation
```
Active Prodigy worktrees:
Name                                     Branch                         Created
------------------------------------------------------------------------------------------
prodigy-session-abc123                   prodigy/session-abc123         2025-01-15 14:32:45
prodigy-session-def456                   prodigy/session-def456         2025-01-15 15:18:22
mr-job-xyz789-agent-1                    mapreduce/job-xyz789           2025-01-15 16:45:10
```

**Issues:**
- Plain text, no visual hierarchy
- No status indicators
- Hard to see age at a glance
- No color coding
- Doesn't show uncommitted changes or branch state

---

### Proposed Enhancement (with comfy-table + console)

```
╭─────────────────────────────────────┬──────────────────────────┬─────────┬────────────┬──────────╮
│ Worktree                            │ Branch                   │ Status  │ Age        │ Changes  │
├─────────────────────────────────────┼──────────────────────────┼─────────┼────────────┼──────────┤
│ prodigy-session-abc123              │ prodigy/session-abc123   │ ✓ Clean │ 2h 15m     │ 0        │
│ prodigy-session-def456              │ prodigy/session-def456   │ ⚠ Dirty │ 1h 3m      │ 5 files  │
│ mr-job-xyz789-agent-1               │ mapreduce/job-xyz789     │ ● Active│ 15m        │ 2 files  │
│ mr-job-xyz789-agent-2               │ mapreduce/job-xyz789     │ ✓ Clean │ 15m        │ 0        │
╰─────────────────────────────────────┴──────────────────────────┴─────────┴────────────┴──────────╯

Summary: 4 worktrees (1 active, 2 clean, 1 with changes)
Tip: Use 'prodigy worktree merge <name>' to merge clean worktrees
```

**With `--detailed` flag:**
```
╭─ prodigy-session-abc123 ─────────────────────────────────────────────────────────────╮
│ Branch:        prodigy/session-abc123                                                 │
│ Created:       2025-01-15 14:32:45 (2 hours ago)                                      │
│ Status:        ✓ Clean (ready to merge)                                               │
│ Original:      feature/new-ui                                                         │
│ Commits ahead: 3                                                                      │
│ Path:          ~/.prodigy/worktrees/prodigy/prodigy-session-abc123                    │
│                                                                                        │
│ Recent commits:                                                                        │
│   • 2h ago: Implement new dashboard layout                                            │
│   • 2h ago: Add progress indicators                                                   │
│   • 2h ago: Update styling                                                            │
╰───────────────────────────────────────────────────────────────────────────────────────╯

╭─ prodigy-session-def456 ─────────────────────────────────────────────────────────────╮
│ Branch:        prodigy/session-def456                                                 │
│ Created:       2025-01-15 15:18:22 (1 hour ago)                                       │
│ Status:        ⚠ Uncommitted changes                                                  │
│ Original:      main                                                                    │
│ Commits ahead: 1                                                                      │
│ Path:          ~/.prodigy/worktrees/prodigy/prodigy-session-def456                    │
│                                                                                        │
│ Uncommitted files:                                                                     │
│   M src/main.rs                                                                        │
│   M src/lib.rs                                                                         │
│   ?? tests/new_test.rs                                                                │
│                                                                                        │
│ Recent commits:                                                                        │
│   • 1h ago: Fix bug in parser                                                         │
╰───────────────────────────────────────────────────────────────────────────────────────╯

Summary: 2 worktrees (1 clean, 1 dirty)
```

**Color scheme (using console crate):**
- Status: Green (✓), Yellow (⚠), Blue (●)
- Age: Dim gray for relative time
- Changes: Red if > 0, green if 0
- Headers: Bold cyan

---

## 2. `prodigy worktree merge` - Interactive Merge

### Current Implementation
```
Merge prodigy-session-abc123 to main? [y/N]
```
*(User types 'y' or 'n')*

**Issues:**
- No preview of what will be merged
- No conflict detection before attempting
- Simple yes/no with no context

---

### Proposed Enhancement (with dialoguer)

**Interactive prompt with preview:**
```
╭─ Merge Preview ───────────────────────────────────────────────────────────────────────╮
│ Source:        prodigy-session-abc123                                                 │
│ Target:        feature/new-ui                                                         │
│ Commits:       3 commits will be merged                                               │
│ Files changed: 8 files modified                                                       │
│ Status:        ✓ No conflicts detected                                                │
╰───────────────────────────────────────────────────────────────────────────────────────╯

Commits to merge:
  • 2h ago: Implement new dashboard layout
  • 2h ago: Add progress indicators
  • 2h ago: Update styling

Files to merge:
  M src/dashboard.rs          (+145 -23)
  M src/progress.rs           (+89 -12)
  M src/styles.css            (+67 -5)
  A src/components/status.rs  (+120)
  M tests/dashboard_test.rs   (+45 -8)
  + 3 more files...

? Merge worktree to feature/new-ui? ›
  ▸ Yes, merge now
    No, cancel
    Show full diff
    View all commits
    Merge with custom message
```

**If conflicts detected:**
```
╭─ Merge Preview ───────────────────────────────────────────────────────────────────────╮
│ Source:        prodigy-session-abc123                                                 │
│ Target:        feature/new-ui                                                         │
│ Commits:       3 commits will be merged                                               │
│ Files changed: 8 files modified                                                       │
│ Status:        ⚠ 2 potential conflicts detected                                       │
╰───────────────────────────────────────────────────────────────────────────────────────╯

Potential conflicts:
  ⚠ src/dashboard.rs       (lines 45-67 modified in both branches)
  ⚠ src/config.rs          (lines 12-15 modified in both branches)

? How do you want to proceed? ›
  ▸ Attempt merge anyway (manual resolution required)
    Show conflict details
    Cancel merge
    View full diff
```

---

## 3. `prodigy worktree clean` - Interactive Cleanup

### Current Implementation
```
Cleaning all worktrees
All worktrees cleaned successfully
```

**Issues:**
- No safety check (besides --force flag)
- No preview of what will be deleted
- Can't select specific ones to clean

---

### Proposed Enhancement (with dialoguer multi-select)

```
╭─ Worktree Cleanup ────────────────────────────────────────────────────────────────────╮
│ Found 4 worktrees. Select which ones to clean:                                        │
╰───────────────────────────────────────────────────────────────────────────────────────╯

? Select worktrees to clean (space to toggle, enter to confirm) ›
  ☑ prodigy-session-abc123        ✓ Clean     2h old     [safe to delete]
  ☑ prodigy-session-def456        ⚠ Dirty     1h old     [has uncommitted changes]
  ☐ mr-job-xyz789-agent-1         ● Active    15m old    [currently in use]
  ☑ prodigy-session-old999        ✓ Merged    5d old     [safe to delete]

Selected: 3 worktrees
  • 2 are safe to delete
  • 1 has uncommitted changes (will be lost!)
  • 0 are currently active (skipped)

Press space to toggle selection, enter to continue, esc to cancel
```

**Confirmation with summary:**
```
╭─ Confirm Cleanup ─────────────────────────────────────────────────────────────────────╮
│ You are about to delete 3 worktrees:                                                  │
│                                                                                        │
│   ✓ prodigy-session-abc123    (clean, 2h old)                                         │
│   ⚠ prodigy-session-def456    (5 uncommitted files - will be lost!)                   │
│   ✓ prodigy-session-old999    (clean, 5d old)                                         │
│                                                                                        │
│ Total disk space to be freed: ~145 MB                                                 │
╰───────────────────────────────────────────────────────────────────────────────────────╯

? Are you sure you want to continue? ›
  ▸ Yes, delete selected worktrees
    No, cancel
    Review selection again
```

---

## 4. `prodigy dlq list` - Dead Letter Queue

### Current Implementation
```
Listing DLQ (Dead Letter Queue) items...
```
*(Not fully implemented)*

---

### Proposed Enhancement (with comfy-table)

```
╭─ Dead Letter Queue ───────────────────────────────────────────────────────────────────╮
│ Job: debtmap-analysis-xyz789                                                          │
│ Failed items: 12                                                                      │
│ Last updated: 2025-01-15 16:45:23                                                     │
╰───────────────────────────────────────────────────────────────────────────────────────╯

╭─────┬─────────────────────┬────────────────────────────┬──────────┬────────────────────╮
│ #   │ Item                │ Error                      │ Retries  │ Failed At          │
├─────┼─────────────────────┼────────────────────────────┼──────────┼────────────────────┤
│ 1   │ src/parser.rs       │ CompilationError           │ 2/3      │ 15m ago            │
│ 2   │ src/lexer.rs        │ TestTimeout (30s)          │ 1/3      │ 12m ago            │
│ 3   │ src/ast.rs          │ OutOfMemory                │ 3/3 ⚠    │ 10m ago            │
│ 4   │ src/codegen.rs      │ ClaudeApiError: 529        │ 1/3      │ 8m ago             │
│ 5   │ tests/integration…  │ ProcessKilled              │ 2/3      │ 5m ago             │
│ ...                                                                                     │
│ 12  │ docs/README.md      │ ValidationFailed           │ 0/3      │ 1m ago             │
╰─────┴─────────────────────┴────────────────────────────┴──────────┴────────────────────╯

Summary: 12 failed items
  • 3 eligible for retry (haven't hit max retries)
  • 9 need manual intervention
  • 1 exhausted all retries

Actions:
  prodigy dlq retry xyz789                    # Retry eligible items
  prodigy dlq inspect xyz789 --item 3         # View details for item #3
  prodigy dlq analyze xyz789                  # Analyze failure patterns
```

**With grouping by error type:**
```
╭─ DLQ Summary by Error Type ──────────────────────────────────────────────────────────╮
│                                                                                        │
│  CompilationError                                      5 items    █████░░░░░ 42%      │
│  TestTimeout                                           3 items    ███░░░░░░░ 25%      │
│  ClaudeApiError                                        2 items    ██░░░░░░░░ 17%      │
│  OutOfMemory                                           1 item     █░░░░░░░░░  8%      │
│  ProcessKilled                                         1 item     █░░░░░░░░░  8%      │
│                                                                                        │
╰───────────────────────────────────────────────────────────────────────────────────────╯

Most common error: CompilationError (42% of failures)
Recommendation: Review compilation environment and dependencies
```

---

## 5. `prodigy dlq retry` - Interactive Retry

### Current Implementation
```
Retrying failed DLQ items...
```

---

### Proposed Enhancement (with dialoguer + indicatif)

**Item selection:**
```
╭─ DLQ Retry ───────────────────────────────────────────────────────────────────────────╮
│ Job: debtmap-analysis-xyz789                                                          │
│ Found 3 items eligible for retry                                                      │
╰───────────────────────────────────────────────────────────────────────────────────────╯

? Select items to retry (space to toggle, enter to confirm) ›
  ☑ src/parser.rs              CompilationError      (2/3 retries)
  ☑ src/lexer.rs               TestTimeout           (1/3 retries)
  ☐ src/codegen.rs             ClaudeApiError: 529   (1/3 retries)

? Maximum parallel retries ›
  ▸ 5 (recommended)
    10
    Custom value

? Retry strategy ›
  ▸ Exponential backoff (1s, 2s, 4s...)
    Fixed delay (5s between retries)
    Immediate retry
```

**Live progress during retry:**
```
╭─ Retrying Items ──────────────────────────────────────────────────────────────────────╮
│                                                                                        │
│  Overall Progress    [████████████░░░░░░░░░░] 2/3  66%    ETA: 45s                   │
│                                                                                        │
│  ✓ src/parser.rs     Completed in 32s                                                 │
│  ⏳ src/lexer.rs      Running tests... [════════░░] 15/20  75%                        │
│  ⏸ src/codegen.rs    Waiting to start...                                             │
│                                                                                        │
│  Stats:  Success: 1  •  Running: 1  •  Pending: 1  •  Failed: 0                      │
│                                                                                        │
╰───────────────────────────────────────────────────────────────────────────────────────╯
```

**Summary after completion:**
```
╭─ Retry Complete ──────────────────────────────────────────────────────────────────────╮
│                                                                                        │
│  ✓ Success:     2 items                                                               │
│  ✗ Failed:      1 item                                                                │
│  ⏱ Duration:    1m 45s                                                                │
│                                                                                        │
│  Successful:                                                                           │
│    ✓ src/parser.rs       (32s)                                                        │
│    ✓ src/lexer.rs        (58s)                                                        │
│                                                                                        │
│  Still failing:                                                                        │
│    ✗ src/codegen.rs      ClaudeApiError: 529 (transient error, will retry later)     │
│                           Retries remaining: 2/3                                      │
│                                                                                        │
╰───────────────────────────────────────────────────────────────────────────────────────╯

Remaining DLQ items: 11
Next action: prodigy dlq analyze xyz789 --error "ClaudeApiError"
```

---

## 6. `prodigy run` - MapReduce Execution

### Current Implementation
```
🔄 Processing items...
✅ Successfully processed item 1
✅ Successfully processed item 2
...
```

---

### Proposed Enhancement (live dashboard with indicatif MultiProgress)

**Setup Phase:**
```
╭─ Workflow: debtmap-analysis ──────────────────────────────────────────────────────────╮
│ Mode: MapReduce  •  Max Parallel: 10  •  Items: 156                                   │
╰───────────────────────────────────────────────────────────────────────────────────────╯

⏳ Setup Phase
  ✓ Loaded workflow configuration
  ✓ Created job ID: debtmap-xyz789
  ⏳ Generating work items from items.json...
  ✓ Extracted 156 items using JSONPath: $.items[*]
  ✓ Applied filters: 142 items remaining
  ⏳ Sorting by priority (descending)...
  ✓ Ready to process 142 items
```

**Map Phase (live updates):**
```
╭─ Map Phase Progress ──────────────────────────────────────────────────────────────────╮
│                                                                                        │
│  Overall    [███████████████████░░░░░░░░░░░]  67/142  47%   ⏱ 5m 32s  ETA: 6m 15s   │
│                                                                                        │
│  Active Agents: 10                                     Success: 65  •  Failed: 2      │
│                                                                                        │
│  Agent-1  [████████░░] src/parser.rs          Running tests    35s                    │
│  Agent-2  [██████████] src/lexer.rs           ✓ Complete       28s                    │
│  Agent-3  [████░░░░░░] src/ast.rs             Fixing issues    42s                    │
│  Agent-4  [██████████] tests/unit.rs          ✓ Complete       31s                    │
│  Agent-5  [██████████] src/codegen.rs         ✓ Complete       45s                    │
│  Agent-6  [████░░░░░░] docs/api.md            Analyzing        19s                    │
│  Agent-7  [███░░░░░░░] src/optimizer.rs       Compiling        52s                    │
│  Agent-8  [████████░░] tests/integration.rs   Running tests    38s                    │
│  Agent-9  [██████████] src/utils.rs           ✓ Complete       22s                    │
│  Agent-10 [██░░░░░░░░] src/validator.rs       Starting         5s                     │
│                                                                                        │
│  Throughput: 2.3 items/sec  •  CPU: 78%  •  Memory: 2.1 GB / 8 GB                     │
│                                                                                        │
│  Recent completions:                                                                   │
│    ✓ src/lexer.rs          28s                                                        │
│    ✓ tests/unit.rs         31s                                                        │
│    ✗ src/broken.rs         Failed: CompilationError → DLQ                             │
│                                                                                        │
╰───────────────────────────────────────────────────────────────────────────────────────╯

Press 'q' to gracefully stop, 'p' to pause, 'l' to view logs
```

**Reduce Phase:**
```
╭─ Reduce Phase ────────────────────────────────────────────────────────────────────────╮
│                                                                                        │
│  Aggregating results from 140 successful items...                                     │
│                                                                                        │
│  [████████████████████████░░░░░░] 4/5  80%                                            │
│                                                                                        │
│  ✓ Collecting map results                                                             │
│  ✓ Validating output format                                                           │
│  ✓ Running analysis on aggregated data                                                │
│  ⏳ Generating summary report...                                                       │
│  ⏸ Committing changes (pending)                                                       │
│                                                                                        │
╰───────────────────────────────────────────────────────────────────────────────────────╯
```

**Final Summary:**
```
╭─ Workflow Complete ───────────────────────────────────────────────────────────────────╮
│                                                                                        │
│  ✓ Job: debtmap-xyz789                                                                │
│  ⏱ Total duration: 11m 47s                                                            │
│                                                                                        │
│  Map Phase:                                                                            │
│    • Total items:     142                                                             │
│    • Successful:      140  (98.6%)                                                    │
│    • Failed:          2    (1.4%) → DLQ                                               │
│    • Avg time/item:   4.8s                                                            │
│    • Peak throughput: 3.1 items/sec                                                   │
│                                                                                        │
│  Reduce Phase:                                                                         │
│    • Duration:        45s                                                             │
│    • Status:          ✓ Success                                                       │
│                                                                                        │
│  Resource Usage:                                                                       │
│    • Peak CPU:        82%                                                             │
│    • Peak Memory:     2.3 GB                                                          │
│    • Worktrees used:  10                                                              │
│                                                                                        │
│  Output:                                                                               │
│    • Results saved to: output/debtmap-results.json                                    │
│    • DLQ items:       2 (use 'prodigy dlq list debtmap-xyz789')                       │
│    • Events logged:   ~/.prodigy/events/prodigy/debtmap-xyz789/                       │
│                                                                                        │
╰───────────────────────────────────────────────────────────────────────────────────────╯

Next steps:
  • Review DLQ items:        prodigy dlq list debtmap-xyz789
  • Retry failures:          prodigy dlq retry debtmap-xyz789
  • View detailed events:    prodigy events ls debtmap-xyz789
  • Clean up worktrees:      prodigy worktree clean --mapreduce
```

---

## 7. `prodigy events follow` - Live Event Streaming

### Current Implementation
```
Following events in real-time...
```
*(Not implemented)*

---

### Proposed Enhancement (tail -f style with colors)

```
╭─ Following Events: debtmap-xyz789 ────────────────────────────────────────────────────╮
│ Streaming live events (press Ctrl+C to stop)                                          │
╰───────────────────────────────────────────────────────────────────────────────────────╯

16:45:23.123  AGENT_STARTED    agent-1        worktree: mr-debtmap-xyz789-agent-1
16:45:23.456  AGENT_STARTED    agent-2        worktree: mr-debtmap-xyz789-agent-2
16:45:24.789  ITEM_START       agent-1        item: src/parser.rs
16:45:24.890  ITEM_START       agent-2        item: src/lexer.rs
16:45:52.123  ITEM_SUCCESS     agent-1        item: src/parser.rs (27.3s)
16:45:52.234  ITEM_START       agent-1        item: src/ast.rs
16:45:53.456  ITEM_FAILED      agent-2        item: src/lexer.rs (28.5s)
                                               error: TestTimeout (30s)
                                               → moved to DLQ
16:45:53.567  ITEM_START       agent-2        item: src/codegen.rs
16:46:05.890  ITEM_SUCCESS     agent-2        item: src/codegen.rs (12.3s)
16:46:15.234  CHECKPOINT       ---            saved: 15 items processed, 1 failed
16:46:35.567  ITEM_SUCCESS     agent-1        item: src/ast.rs (43.3s)
▮

Color coding:
  • Green:  AGENT_STARTED, ITEM_SUCCESS, CHECKPOINT
  • Yellow: AGENT_FINISHED, PHASE_COMPLETE
  • Red:    ITEM_FAILED, AGENT_ERROR
  • Cyan:   ITEM_START
  • Dim:    Timestamps
```

**With filters:**
```bash
prodigy events follow debtmap-xyz789 --event-type ITEM_FAILED
```

```
╭─ Following Events: debtmap-xyz789 (filter: ITEM_FAILED) ─────────────────────────────╮
│ Streaming live events (press Ctrl+C to stop)                                          │
╰───────────────────────────────────────────────────────────────────────────────────────╯

16:45:53.456  ITEM_FAILED      agent-2        item: src/lexer.rs
                                               error: TestTimeout (30s)
                                               correlation_id: corr-123abc
                                               retries: 1/3
                                               → moved to DLQ

16:47:12.789  ITEM_FAILED      agent-5        item: src/broken.rs
                                               error: CompilationError
                                               stderr: error[E0308]: mismatched types
                                               correlation_id: corr-456def
                                               retries: 2/3
                                               → moved to DLQ
▮
```

---

## 8. Error Messages - Enhanced Formatting

### Current Implementation
```
❌ Failed to merge worktree 'prodigy-session-abc123': merge conflict
```

---

### Proposed Enhancement (with console styling)

```
╭─ Error ───────────────────────────────────────────────────────────────────────────────╮
│                                                                                        │
│  ✗ Failed to merge worktree                                                           │
│                                                                                        │
│  Worktree:  prodigy-session-abc123                                                    │
│  Target:    feature/new-ui                                                            │
│  Error:     Merge conflict                                                            │
│                                                                                        │
│  Conflicting files:                                                                    │
│    • src/dashboard.rs        (lines 45-67)                                            │
│    • src/config.rs           (lines 12-15)                                            │
│                                                                                        │
│  To resolve:                                                                           │
│    1. Switch to worktree:                                                             │
│       cd ~/.prodigy/worktrees/prodigy/prodigy-session-abc123                          │
│                                                                                        │
│    2. Manually resolve conflicts in the files above                                   │
│                                                                                        │
│    3. Commit the resolution:                                                           │
│       git add .                                                                        │
│       git commit -m "Resolve merge conflicts"                                         │
│                                                                                        │
│    4. Retry the merge:                                                                 │
│       prodigy worktree merge prodigy-session-abc123                                   │
│                                                                                        │
│  Or cancel and clean up:                                                               │
│    prodigy worktree clean prodigy-session-abc123                                      │
│                                                                                        │
╰───────────────────────────────────────────────────────────────────────────────────────╯
```

---

## 9. Configuration Wizard - `prodigy init`

### Current Implementation
```
Initializing Claude commands...
✅ Created .claude/commands/
```

---

### Proposed Enhancement (with dialoguer)

```
╭─ Prodigy Configuration Wizard ────────────────────────────────────────────────────────╮
│ Let's set up Prodigy for your project                                                 │
╰───────────────────────────────────────────────────────────────────────────────────────╯

? Project name › prodigy

? Storage location ›
  ▸ Global (~/.prodigy) - Recommended
    Local (.prodigy)    - Project-specific

? Maximum parallel agents for MapReduce ›
  ▸ 5  - Recommended for most systems
    10 - High-performance systems
    Custom value

? Default retry strategy for failed items ›
  ▸ Exponential backoff (1s, 2s, 4s...)
    Fixed delay (5s)
    Immediate retry

? Enable event logging ›
  ▸ Yes - Recommended for debugging
    No

? Event retention policy ›
  ▸ 30 days
    7 days
    90 days
    Forever
    Custom

? Automatically clean merged worktrees ›
  ▸ Prompt each time
    Always clean
    Never clean

? Install example workflows ›
  ▸ Yes - Includes debtmap, CI, and testing examples
    No

╭─ Configuration Summary ───────────────────────────────────────────────────────────────╮
│                                                                                        │
│  Project:              prodigy                                                         │
│  Storage:              ~/.prodigy                                                      │
│  Max parallel:         5 agents                                                        │
│  Retry strategy:       Exponential backoff                                             │
│  Event logging:        Enabled (30 day retention)                                     │
│  Auto cleanup:         Prompt each time                                                │
│  Example workflows:    Yes                                                             │
│                                                                                        │
╰───────────────────────────────────────────────────────────────────────────────────────╯

? Confirm configuration ›
  ▸ Yes, create configuration
    No, start over
    Advanced settings

✓ Created configuration: ~/.prodigy/config.toml
✓ Created .claude/commands/
✓ Installed example workflows:
    • .claude/workflows/debtmap.yml
    • .claude/workflows/ci.yml
    • .claude/workflows/test.yml

🎉 Prodigy is ready to use!

Next steps:
  • Run your first workflow:     prodigy run .claude/workflows/debtmap.yml
  • Create a custom workflow:    prodigy workflow new my-workflow
  • View documentation:          prodigy help
```

---

## 10. Real-time Dashboard (Optional Future Feature)

Using `ratatui` for a full-screen TUI:

```
╔══════════════════════════════════════════════════════════════════════════════════════╗
║ Prodigy Dashboard - Job: debtmap-xyz789                          ⏱  5m 32s  ▲ LIVE ║
╠══════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                      ║
║  Phase: Map                                                                          ║
║  Progress: [████████████████████░░░░░░░░░░░]  67/142  47%    ETA: 6m 15s           ║
║                                                                                      ║
║  Success: 65  •  Failed: 2  •  Active: 10  •  Throughput: 2.3/sec                  ║
║                                                                                      ║
╠════════════════════════════════ Active Agents ═══════════════════════════════════════╣
║                                                                                      ║
║  Agent-1   [████████░░] src/parser.rs            Running tests         35s          ║
║  Agent-2   [██████████] src/lexer.rs             ✓ Complete            28s          ║
║  Agent-3   [████░░░░░░] src/ast.rs               Fixing issues         42s          ║
║  Agent-4   [██████████] tests/unit.rs            ✓ Complete            31s          ║
║  Agent-5   [██████████] src/codegen.rs           ✓ Complete            45s          ║
║  Agent-6   [████░░░░░░] docs/api.md              Analyzing             19s          ║
║  Agent-7   [███░░░░░░░] src/optimizer.rs         Compiling             52s          ║
║  Agent-8   [████████░░] tests/integration.rs     Running tests         38s          ║
║  Agent-9   [██████████] src/utils.rs             ✓ Complete            22s          ║
║  Agent-10  [██░░░░░░░░] src/validator.rs         Starting              5s           ║
║                                                                                      ║
╠═══════════════════════════════ System Resources ═════════════════════════════════════╣
║                                                                                      ║
║  CPU:     [███████████████████████░░░░░░░░░░] 78%                                   ║
║  Memory:  [███████░░░░░░░░░░░░░░░░░░░░░░░░░] 2.1 GB / 8 GB                         ║
║  Disk I/O: ▁▂▃▅▄▃▅▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄ 45 MB/s                                      ║
║                                                                                      ║
╠═══════════════════════════════ Recent Events ════════════════════════════════════════╣
║                                                                                      ║
║  16:46:35  ✓ ITEM_SUCCESS    agent-1   src/ast.rs (43.3s)                          ║
║  16:46:15  ⚡ CHECKPOINT      ---        15 items processed                          ║
║  16:46:05  ✓ ITEM_SUCCESS    agent-2   src/codegen.rs (12.3s)                      ║
║  16:45:53  ✗ ITEM_FAILED     agent-2   src/lexer.rs (TestTimeout)                  ║
║  16:45:52  ✓ ITEM_SUCCESS    agent-1   src/parser.rs (27.3s)                       ║
║                                                                                      ║
╠═══════════════════════════════════════════════════════════════════════════════════════╣
║  [q] Quit  [p] Pause  [r] Resume  [l] Logs  [d] DLQ  [e] Events  [h] Help          ║
╚═══════════════════════════════════════════════════════════════════════════════════════╝
```

---

## Implementation Roadmap

### Libraries to Add:
```toml
[dependencies]
# Already have:
indicatif = "0.18"

# Add these:
console = "0.15"           # Terminal styling and abstractions
dialoguer = "0.11"         # Interactive prompts
comfy-table = "7"          # Beautiful tables
crossterm = "0.27"         # Terminal control (optional)
ratatui = "0.27"           # Full TUI (future dashboard, optional)
```

### Phase 1: Quick Wins (1-2 days)
1. Replace `println!` table in `worktree ls` with `comfy-table`
2. Add `console` colors to status messages
3. Use `dialoguer::Confirm` for merge/clean prompts
4. Enhance error messages with boxed formatting

### Phase 2: Interactive Features (3-5 days)
1. Multi-select for `worktree clean`
2. DLQ list with formatted tables
3. DLQ retry with progress bars
4. Merge preview with conflict detection
5. Configuration wizard for `init`

### Phase 3: Live Progress (5-7 days)
1. Enhanced MapReduce progress with `MultiProgress`
2. Real-time agent status updates
3. Live event streaming with `events follow`
4. Resource monitoring display

### Phase 4: Advanced (Future)
1. Full TUI dashboard with `ratatui`
2. Interactive log viewer
3. Visual diff viewer for merges
4. Network graph of worktree relationships
