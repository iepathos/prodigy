---
number: 49
title: Change worktree list to ls
category: compatibility
priority: low
status: draft
dependencies: []
created: 2025-08-04
---

# Specification 49: Change worktree list to ls

**Category**: compatibility
**Priority**: low
**Status**: draft
**Dependencies**: None

## Context

The current `mmm worktree` command uses `list` as the subcommand to list active MMM worktrees. However, following common Unix/Linux conventions, many command-line tools use `ls` as the command for listing items (e.g., `ls` for listing files, `docker ls` for listing containers, `git branch` or `git worktree list`). While Git itself uses `git worktree list`, the shorter `ls` convention is more common in modern CLI tools and would provide a more concise and familiar interface for users.

## Objective

Change the `mmm worktree list` command to `mmm worktree ls` to follow common CLI conventions and provide a more concise command interface.

## Requirements

### Functional Requirements
- Replace the `list` subcommand with `ls` in the worktree command structure
- Maintain backward compatibility by supporting `list` as an alias to `ls`
- Preserve all existing functionality of the list command
- Update all references to `worktree list` in the codebase

### Non-Functional Requirements
- No changes to the command's output format or behavior
- Minimal impact on existing users who may have scripts using `worktree list`
- Clear deprecation path if we choose to remove the `list` alias in the future

## Acceptance Criteria

- [ ] `mmm worktree ls` successfully lists all active MMM worktrees
- [ ] `mmm worktree list` continues to work as an alias for backward compatibility
- [ ] Help text shows `ls` as the primary command with `list` as an alias
- [ ] All tests pass with the new command structure
- [ ] Documentation is updated to use `ls` instead of `list`
- [ ] No breaking changes for existing users

## Technical Details

### Implementation Approach
1. Update the `WorktreeCommands` enum in `src/main.rs` to change `List` to `Ls`
2. Add `alias = "list"` attribute to maintain backward compatibility
3. Update all code references from `WorktreeCommands::List` to `WorktreeCommands::Ls`
4. Update tests to use the new `ls` command while ensuring `list` alias works

### Architecture Changes
No architectural changes required. This is a simple command name change.

### Data Structures
The `WorktreeCommands` enum in `src/main.rs` will be modified:
```rust
#[derive(Subcommand)]
enum WorktreeCommands {
    /// List active MMM worktrees
    #[command(alias = "list")]
    Ls,
    // ... other commands remain unchanged
}
```

### APIs and Interfaces
The CLI interface changes from `mmm worktree list` to `mmm worktree ls` with `list` remaining as an alias.

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - CLI parser in `src/main.rs`
  - Worktree command handling
  - Tests that use `worktree list`
  - Documentation and README
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Update existing tests to use `ls` command
- **Integration Tests**: 
  - Test that both `ls` and `list` produce identical output
  - Verify help text shows correct command and alias
- **Performance Tests**: Not applicable for this change
- **User Acceptance**: Ensure no disruption to existing workflows

## Documentation Requirements

- **Code Documentation**: Update inline comments referencing the command
- **User Documentation**: 
  - Update README.md to use `mmm worktree ls` in examples
  - Update any command reference documentation
  - Add note about `list` being supported as an alias
- **Architecture Updates**: None required

## Implementation Notes

- Use clap's `alias` attribute to maintain backward compatibility
- Consider adding a deprecation notice for the `list` alias in a future version
- Ensure all error messages reference the new `ls` command

## Migration and Compatibility

- **Breaking Changes**: None - the `list` alias ensures full backward compatibility
- **Migration Path**: Users can continue using `list` or switch to `ls` at their convenience
- **Deprecation Strategy**: Consider deprecating `list` alias in a future major version with appropriate notice period