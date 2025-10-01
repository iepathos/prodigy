---
number: 111
title: Interactive Worktree Commands
category: foundation
priority: high
status: draft
dependencies: [110]
created: 2025-10-01
---

# Specification 111: Interactive Worktree Commands

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [110 - Terminal UI Foundation]

## Context

Current worktree commands (`ls`, `merge`, `clean`) provide minimal output and require manual decisions without sufficient context. Users cannot preview merge changes, see uncommitted changes, or select specific worktrees for cleanup. The commands lack:

- Rich formatted output showing worktree status
- Interactive merge previews with conflict detection
- Multi-select cleanup with safety warnings
- Status indicators and relative time displays
- Detailed views showing commits and file changes

## Objective

Transform worktree commands into rich, interactive experiences with formatted tables, merge previews, multi-select cleanup, and detailed status information using the terminal UI foundation.

## Requirements

### Functional Requirements

**FR1**: Enhanced `prodigy worktree ls` output
- Display worktrees in formatted table with borders
- Show columns: Name, Branch, Status, Age, Changes
- Status indicators: ✓ Clean, ⚠ Dirty, ● Active
- Relative time display (2h 15m, 3d, etc.)
- Summary line with counts (X worktrees, Y clean, Z dirty)
- Helpful tip suggesting next actions

**FR2**: Detailed worktree view (`--detailed` flag)
- Box display for each worktree with all metadata
- Show: branch, created time, status, original branch, commits ahead
- List recent commits (last 3-5) with relative times
- Show uncommitted files if dirty
- Display worktree path
- Color-coded status boxes

**FR3**: Interactive merge preview (`prodigy worktree merge`)
- Show merge preview box with summary statistics
- Display: source, target, commit count, files changed
- Run pre-merge conflict detection
- List commits to be merged (summary + expandable)
- List files to be merged with line counts (+/-)
- Interactive menu with options:
  - Yes, merge now
  - No, cancel
  - Show full diff
  - View all commits
  - Merge with custom message

**FR4**: Conflict detection before merge
- Detect potential conflicts before attempting merge
- Show conflicting files with line ranges
- Warn user with ⚠ status
- Offer options:
  - Attempt merge anyway
  - Show conflict details
  - Cancel merge
  - View full diff

**FR5**: Interactive cleanup (`prodigy worktree clean`)
- Multi-select interface for choosing worktrees to clean
- Show status for each worktree (clean/dirty/active)
- Display safety indicators ([safe to delete] / [has uncommitted changes])
- Checkbox selection with space bar
- Summary showing selected count and warnings
- Confirmation screen with impact summary
- Show disk space to be freed

**FR6**: Safety features for cleanup
- Cannot select currently active worktrees (grayed out)
- Warn about uncommitted changes in red
- Show detailed summary before final confirmation
- Allow review and modification of selection
- Estimate disk space impact

### Non-Functional Requirements

**NFR1**: Performance - Table rendering < 100ms for 100 worktrees
**NFR2**: Usability - Intuitive keyboard navigation (space, enter, esc)
**NFR3**: Safety - Multiple confirmations for destructive operations
**NFR4**: Responsiveness - Works on terminals with 80+ column width

## Acceptance Criteria

- [ ] `prodigy worktree ls` displays formatted table with all required columns
- [ ] Status indicators (✓ ⚠ ●) display correctly with color coding
- [ ] Relative time display works (minutes, hours, days)
- [ ] Summary line shows accurate counts
- [ ] `--detailed` flag shows boxed view with all metadata
- [ ] Recent commits displayed with relative timestamps
- [ ] Uncommitted files shown for dirty worktrees
- [ ] Merge preview shows all required information
- [ ] Pre-merge conflict detection identifies conflicting files
- [ ] Interactive merge menu allows all specified actions
- [ ] Multi-select cleanup interface works with keyboard navigation
- [ ] Safety warnings displayed for dirty worktrees
- [ ] Confirmation screen shows accurate impact
- [ ] Disk space calculation works correctly
- [ ] Cannot accidentally select active worktrees
- [ ] All interactions gracefully fall back to non-interactive mode
- [ ] Works correctly on 80-column terminals

## Technical Details

### Implementation Approach

**Phase 1: Enhanced `worktree ls`**
1. Implement worktree status detection (clean/dirty/active)
2. Create table formatter using comfy-table
3. Add relative time formatting
4. Implement summary statistics
5. Add --detailed flag with boxed display

**Phase 2: Interactive Merge**
1. Implement pre-merge analysis (commits, files, conflicts)
2. Create merge preview display with comfy-table
3. Build interactive menu with dialoguer::Select
4. Implement conflict detection using git operations
5. Add diff viewing functionality

**Phase 3: Interactive Cleanup**
1. Implement multi-select interface with dialoguer::MultiSelect
2. Add safety classification (safe/warning/blocked)
3. Create confirmation screen with impact summary
4. Implement disk space calculation
5. Add review capability

### Module Structure

```rust
src/cli/commands/worktree/
├── mod.rs              // Command routing
├── list.rs            // Enhanced ls implementation
├── merge.rs           // Interactive merge
├── clean.rs           // Interactive cleanup
└── display.rs         // Shared display utilities

src/worktree/
├── status.rs          // Worktree status detection
├── analysis.rs        // Merge analysis and conflict detection
└── metrics.rs         // Disk space and statistics
```

### Key Data Structures

```rust
// Worktree status
pub enum WorktreeStatus {
    Clean,        // No uncommitted changes, ready to merge
    Dirty,        // Has uncommitted changes
    Active,       // Currently in use
}

// Worktree info for display
pub struct WorktreeDisplayInfo {
    pub name: String,
    pub branch: String,
    pub status: WorktreeStatus,
    pub age: Duration,
    pub uncommitted_files: usize,
    pub commits_ahead: usize,
    pub original_branch: Option<String>,
    pub disk_usage: u64,
}

// Merge preview
pub struct MergePreview {
    pub source_branch: String,
    pub target_branch: String,
    pub commits: Vec<CommitInfo>,
    pub files_changed: Vec<FileChange>,
    pub conflicts: Vec<ConflictInfo>,
    pub has_conflicts: bool,
}

// Cleanup selection
pub struct CleanupSelection {
    pub worktree: String,
    pub status: WorktreeStatus,
    pub uncommitted_files: usize,
    pub disk_usage: u64,
    pub safe_to_delete: bool,
}
```

### Display Formats

**Table Style:**
```rust
use comfy_table::presets::UTF8_FULL;
use comfy_table::*;

let mut table = Table::new();
table.load_preset(UTF8_FULL);
table.set_header(vec!["Worktree", "Branch", "Status", "Age", "Changes"]);
```

**Box Style:**
```rust
// Use console crate for box drawing
println!("╭─ {} ─────────────────────────────────────────────────────────╮", name);
println!("│ Branch:        {:<50}│", branch);
// ...
println!("╰──────────────────────────────────────────────────────────────╯");
```

### Git Operations

**Status Detection:**
```bash
# Check for uncommitted changes
git status --porcelain

# Check for commits ahead
git rev-list --count @{u}..HEAD

# Detect conflicts (during merge preview)
git merge-tree --write-tree main feature
```

### Keyboard Navigation

Dialoguer default keybindings:
- `Space` - Toggle selection (multi-select)
- `Enter` - Confirm selection
- `Esc` - Cancel operation
- `↑↓` - Navigate options
- `a` - Select all (custom addition)

## Dependencies

- **Prerequisites**: [110 - Terminal UI Foundation]
- **Affected Components**:
  - `src/cli/commands/worktree.rs` - Complete rewrite
  - `src/worktree/manager.rs` - Add status detection methods
  - Git operations abstraction
- **External Dependencies**:
  - Inherits from Spec 110: console, dialoguer, comfy-table

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_worktree_status_detection() {
    // Test clean, dirty, active status detection
}

#[test]
fn test_relative_time_formatting() {
    // Test "2h 15m", "3d", etc.
}

#[test]
fn test_merge_conflict_detection() {
    // Test pre-merge conflict analysis
}

#[test]
fn test_cleanup_safety_classification() {
    // Test safe/warning/blocked classification
}

#[test]
fn test_disk_space_calculation() {
    // Verify disk usage calculation
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_worktree_ls_table_output() {
    // Create test worktrees
    // Run ls command
    // Verify table format and content
}

#[tokio::test]
async fn test_merge_preview_with_conflicts() {
    // Create worktree with conflicts
    // Run merge with preview
    // Verify conflict detection
}

#[tokio::test]
async fn test_cleanup_multi_select() {
    // Create multiple worktrees
    // Test selection and cleanup
    // Verify only selected worktrees removed
}
```

### Manual Testing

- Test on narrow terminals (80 columns)
- Test with many worktrees (100+)
- Test merge with actual conflicts
- Test cleanup with mixed states
- Verify keyboard navigation feels smooth
- Test non-interactive fallback

## Documentation Requirements

### Code Documentation

- Document all display utilities
- Add examples for creating formatted tables
- Document git status detection algorithms

### User Documentation

- Update CLAUDE.md with new worktree command behaviors
- Add screenshots/examples of new output
- Document keyboard shortcuts for interactive mode

### Architecture Documentation

- Document display layer separation
- Explain worktree status state machine
- Document merge analysis process

## Implementation Notes

### Relative Time Formatting

```rust
fn format_relative_time(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}d", secs / 86400)
    }
}
```

### Conflict Detection

Use `git merge-tree` for dry-run conflict detection:
```bash
git merge-tree --write-tree <target> <source>
```

Parse output to identify conflicts before actual merge.

### Non-Interactive Fallback

When not interactive:
- `worktree ls` - Output simple table format
- `worktree merge` - Use default "no" (require --yes flag)
- `worktree clean` - Require explicit names or --all flag

### Terminal Width Handling

Detect terminal width and adjust:
- < 80 cols: Use compact format
- 80-120 cols: Use standard format
- \> 120 cols: Use detailed format

## Migration and Compatibility

### Breaking Changes

- `worktree ls` output format changes (from simple to table)
- `worktree merge` now interactive by default

### Migration Path

1. Add `--non-interactive` or `--yes` flags for CI/CD
2. Detect non-TTY and auto-disable interactive mode
3. Preserve basic functionality for scripts

### Backward Compatibility

- `--json` flag preserves machine-readable output
- Non-interactive mode maintains script compatibility
- Exit codes remain unchanged

## Success Metrics

- Worktree list displays all information clearly
- Users can preview merges before executing
- Conflict detection prevents failed merges
- Cleanup selection reduces accidental deletions
- Interactive mode feels responsive and intuitive
- All operations gracefully degrade in non-interactive mode
