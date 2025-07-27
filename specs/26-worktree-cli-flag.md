# Specification 26: Replace Worktree Environment Variable with CLI Flag

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: 24

## Context

The current implementation uses an environment variable `MMM_USE_WORKTREE=true` to enable worktree mode for parallel sessions. This violates CLI best practices:
- Not discoverable through `--help`
- Requires extra typing and cognitive load
- Inconsistent with other CLI options
- Makes command history less clear

A command-line flag would be more intuitive, discoverable, and follow standard CLI conventions.

## Objective

Replace the `MMM_USE_WORKTREE` environment variable with a `--worktree` (or `-w`) flag on the improve command, making parallel execution more user-friendly and discoverable.

## Requirements

### Functional Requirements
- Add `--worktree` flag (with `-w` short form) to the improve command
- Remove dependency on `MMM_USE_WORKTREE` environment variable
- Maintain backward compatibility temporarily with deprecation warning
- Update all documentation and examples
- Preserve all existing worktree functionality

### Non-Functional Requirements
- Flag should be discoverable in help text
- Clear error messages if worktree operations fail
- Consistent with other CLI flags in the project

## Acceptance Criteria

- [ ] `mmm improve --worktree` creates and uses a worktree
- [ ] `mmm improve -w` works as short form
- [ ] Flag appears in `mmm improve --help`
- [ ] Environment variable shows deprecation warning if used
- [ ] All documentation updated with new syntax
- [ ] Tests updated to use flag instead of env var
- [ ] Examples in specs and README use new syntax

## Technical Details

### Implementation Approach

1. **Update ImproveCommand struct**
   ```rust
   #[derive(Debug, Args, Clone)]
   pub struct ImproveCommand {
       // ... existing fields ...
       
       /// Run in an isolated git worktree for parallel execution
       #[arg(short = 'w', long)]
       pub worktree: bool,
   }
   ```

2. **Update improve/mod.rs**
   ```rust
   // Check flag first, then env var with deprecation warning
   let use_worktree = if cmd.worktree {
       true
   } else if std::env::var("MMM_USE_WORKTREE").map(|v| v == "true" || v == "1").unwrap_or(false) {
       eprintln!("Warning: MMM_USE_WORKTREE is deprecated. Use --worktree or -w flag instead.");
       true
   } else {
       false
   };
   ```

3. **Remove env var after grace period**
   - Keep backward compatibility for 2-3 releases
   - Then remove environment variable support entirely

### Migration Path

1. **Phase 1 (This release)**
   - Add `--worktree` flag
   - Keep env var with deprecation warning
   - Update all documentation

2. **Phase 2 (Next release)**
   - Continue deprecation warning
   - Add note in release notes about upcoming removal

3. **Phase 3 (Future release)**
   - Remove environment variable support
   - Flag becomes the only way to enable worktree mode

### Example Usage

**Before:**
```bash
MMM_USE_WORKTREE=true mmm improve --focus "testing"
MMM_USE_WORKTREE=true mmm improve --focus "security" --max-iterations 5
```

**After:**
```bash
mmm improve --worktree --focus "testing"
mmm improve -w --focus "security" --max-iterations 5

# Or with short forms
mmm improve -w --focus "testing"
```

### Help Text Update

```
mmm improve --help

Options:
  --target <TARGET>                    Target quality score [default: 8.0]
  --show-progress                      Show detailed progress
  --focus <FOCUS>                      Focus directive for initial analysis
  -w, --worktree                       Run in isolated git worktree for parallel execution
  -c, --config <CONFIG>                Path to configuration file
  -n, --max-iterations <MAX>           Maximum iterations [default: 10]
  -h, --help                           Print help
```

## Dependencies

- **Prerequisites**: 
  - Spec 24 (Git worktree isolation) - Base functionality to update
- **Affected Components**: 
  - `src/improve/command.rs` - Add worktree flag
  - `src/improve/mod.rs` - Check flag instead of env var
  - All documentation files
  - All tests using worktree functionality

## Testing Strategy

- **Unit Tests**: 
  - Flag parsing correctly sets worktree mode
  - Deprecation warning shown for env var
  - Flag takes precedence over env var
- **Integration Tests**: 
  - Worktree created when flag is used
  - No worktree when flag is absent
  - Backward compatibility with env var
- **Documentation Tests**: 
  - All examples use new flag syntax
  - No remaining references to env var (except deprecation note)

## Documentation Requirements

- **Update all examples** in:
  - README.md
  - Spec files
  - Code comments
  - Help text
- **Add migration guide** showing how to update commands
- **Update architecture docs** to reflect flag-based activation

## Success Metrics

- Zero user confusion about how to enable worktree mode
- Increased usage of parallel sessions due to discoverability
- Clean migration from env var to flag
- Consistent CLI experience

## Risks and Mitigation

- **Risk**: Breaking existing scripts using env var
  - **Mitigation**: Deprecation period with clear warnings
- **Risk**: Users not noticing the change
  - **Mitigation**: Clear release notes and migration guide