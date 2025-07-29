# Implementation Summary: Spec 41 - Auto-Accept Flag

## Changes Made

### 1. Added auto_accept flag to CookCommand
- Added `-y/--yes` flag to `src/cook/command.rs`
- Short form: `-y`, long form: `--yes`
- Boolean field with appropriate help text

### 2. Updated merge and deletion prompt logic
- Modified `src/cook/mod.rs` to check `auto_accept` flag before prompting
- When flag is set, automatically accepts worktree merge and deletion
- Logs "Auto-accepting worktree merge/deletion (--yes flag set)" for transparency
- Updated condition to run prompts when either in TTY or auto_accept is true

### 3. Fixed compilation issues
- Updated `src/main.rs` to include `auto_accept: false` in CookCommand creation
- Updated all test files to include the new field:
  - `src/cook/tests.rs`
  - `src/cook/mod.rs` (test helper)
  - `tests/cook_tests.rs`

### 4. Updated documentation
- Updated `.mmm/PROJECT.md` to reflect spec 41 completion
- Updated `.mmm/ROADMAP.md` to mark spec 41 as completed
- Added ADR-031 to `.mmm/DECISIONS.md` documenting the decision
- Updated `.mmm/SPEC_INDEX.md` to move spec 41 to implemented features

## Usage Examples

```bash
# Fully automated workflow - no prompts
mmm cook -y --worktree --focus "security"

# Long form also works
mmm cook --yes --worktree --max-iterations 5

# In scripts or CI/CD
#!/bin/bash
mmm cook -y -w --focus "performance"
```

## Testing

- Project builds successfully with `cargo build`
- All tests pass (one flaky test unrelated to this change)
- No clippy warnings with `cargo clippy`
- Code formatted with `cargo fmt`

## Key Design Decisions

1. **Flag naming**: Used `-y/--yes` to match Unix conventions (apt-get, yum, etc.)
2. **Safety**: Only auto-accepts on successful completion, never on failure
3. **Transparency**: Always logs when auto-accepting for audit trail
4. **Backward compatibility**: No breaking changes, flag is optional