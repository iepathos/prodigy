---
number: 105
title: Consolidate Cook Command into Run
category: compatibility
priority: medium
status: draft
dependencies: []
created: 2025-01-19
---

# Specification 105: Consolidate Cook Command into Run

**Category**: compatibility
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

Currently, Prodigy has two commands that essentially do the same thing: `cook` and `run`. The `run` command was introduced as an alias for `cook` with better semantics, but currently lacks several features that `cook` has, most notably the `--dry-run` flag. This duplication creates confusion for users and increases maintenance burden. The `run` command is more standard for CLI tooling and should be the primary interface.

The current implementation shows that `run` is already internally converted to a `CookCommand`, confirming that they share the same underlying execution logic. However, `run` is missing several important flags that `cook` supports:
- `--dry-run`: Preview commands without executing them
- `--fail-fast`: Stop on first failure when processing multiple files
- `--metrics`: Enable metrics tracking
- `--resume`: Resume an interrupted session
- `--map`: File patterns to map over

## Objective

Consolidate all functionality from the `cook` command into the `run` command, making `run` the primary interface while maintaining `cook` as a deprecated alias for backward compatibility. This will provide a cleaner, more standard CLI interface while preserving all existing functionality.

## Requirements

### Functional Requirements

1. **Feature Parity**: The `run` command must support all features currently available in `cook`:
   - Dry-run mode (`--dry-run`)
   - Fail-fast mode (`--fail-fast`)
   - Metrics tracking (`--metrics`)
   - Session resumption (`--resume`)
   - File mapping patterns (`--map`)

2. **Backward Compatibility**:
   - Keep `cook` as a deprecated alias that maps to `run`
   - Display deprecation warning when `cook` is used
   - All existing `cook` command invocations must continue to work

3. **Command Naming**:
   - `run` becomes the primary command
   - Remove the "alias for cook" description from `run`
   - Update `cook` description to indicate it's deprecated

4. **Help Text Updates**:
   - Update `run` command help to be comprehensive
   - Add deprecation notice to `cook` help text
   - Ensure all documentation reflects the new primary command

### Non-Functional Requirements

1. **User Experience**:
   - Clear migration path for users
   - Helpful deprecation messages with suggested alternatives
   - No breaking changes to existing workflows

2. **Code Quality**:
   - Remove code duplication between commands
   - Maintain single source of truth for command logic
   - Clean separation between deprecated and current code

## Acceptance Criteria

- [ ] `prodigy run --dry-run` works correctly and shows command preview
- [ ] `prodigy run --fail-fast` stops on first failure
- [ ] `prodigy run --metrics` enables metrics tracking
- [ ] `prodigy run --resume <session>` resumes interrupted sessions
- [ ] `prodigy run --map <pattern>` supports file mapping
- [ ] `prodigy cook` still works but shows deprecation warning
- [ ] Help text for `run` is complete and doesn't reference `cook`
- [ ] Help text for `cook` indicates deprecation and suggests `run`
- [ ] All tests pass with both `run` and `cook` commands
- [ ] Documentation is updated to use `run` as primary command

## Technical Details

### Implementation Approach

1. **Phase 1: Add Missing Flags to Run**
   - Add all missing command-line arguments to the `Run` variant in the CLI enum
   - Ensure proper argument parsing and validation
   - Update help text and descriptions

2. **Phase 2: Update Command Routing**
   - Modify the `Run` command handler to accept all parameters
   - Ensure it creates a complete `CookCommand` with all options
   - Remove any special-case handling that differs between `run` and `cook`

3. **Phase 3: Deprecate Cook Command**
   - Add deprecation warning function
   - Update `cook` command to display warning before execution
   - Update help text to indicate deprecation

4. **Phase 4: Documentation Updates**
   - Update all documentation to use `run` as primary example
   - Add migration guide for users
   - Update CLI help text

### Architecture Changes

No significant architecture changes required. This is primarily a CLI interface update that:
- Extends the `Run` command struct with additional fields
- Updates command routing logic
- Adds deprecation warnings

### Data Structures

Update the `Commands::Run` struct in `src/main.rs`:

```rust
Run {
    // Existing fields
    workflow: PathBuf,
    path: Option<PathBuf>,
    max_iterations: u32,
    worktree: bool,
    args: Vec<String>,
    auto_accept: bool,

    // New fields from Cook
    map: Vec<String>,
    fail_fast: bool,
    metrics: bool,
    resume: Option<String>,
    dry_run: bool,
}
```

### APIs and Interfaces

No external API changes. Internal changes include:
- Updated CLI argument parsing
- Deprecation warning system
- Unified command handling

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - CLI argument parser (`src/main.rs`)
  - Cook command module (`src/cook/`)
  - Tests that use either command
  - Documentation files
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test all flags work correctly with `run` command
  - Test deprecation warning appears for `cook` command
  - Test backward compatibility of `cook` command

- **Integration Tests**:
  - Test dry-run mode with complex workflows
  - Test session resumption with both commands
  - Test MapReduce workflows with file patterns
  - Verify metrics collection works correctly

- **Performance Tests**:
  - Ensure no performance regression from consolidation
  - Verify dry-run mode has minimal overhead

- **User Acceptance**:
  - Test migration from `cook` to `run` commands
  - Verify all existing scripts continue to work
  - Test help text clarity and completeness

## Documentation Requirements

- **Code Documentation**:
  - Document deprecation in `cook` command code
  - Update inline comments to reference `run` as primary

- **User Documentation**:
  - Update README.md to use `run` in examples
  - Update CLAUDE.md with new command structure
  - Create migration guide for existing users
  - Update all workflow examples

- **Architecture Updates**:
  - Update command structure documentation
  - Document deprecation timeline and policy

## Implementation Notes

1. **Deprecation Strategy**:
   - Show warning on every `cook` invocation
   - Include suggestion to use `run` instead
   - Plan for removal in future major version

2. **Testing Considerations**:
   - Maintain tests for both commands during transition
   - Add specific tests for deprecation warnings
   - Ensure CI/CD uses `run` command

3. **Migration Timeline**:
   - Immediate: Add features to `run`, deprecate `cook`
   - 3 months: Update all documentation and examples
   - 6 months: Consider removing `cook` in major version

## Migration and Compatibility

### Breaking Changes
None - this specification ensures full backward compatibility.

### Migration Path
1. Users can immediately switch from `cook` to `run`
2. Existing scripts using `cook` continue to work with deprecation warning
3. Documentation and examples updated to guide users to `run`

### Compatibility Matrix
| Command | Current Version | After Implementation | Future Version |
|---------|----------------|---------------------|----------------|
| `run`   | Limited flags  | Full functionality  | Primary command |
| `cook`  | Full functionality | Deprecated, works | May be removed |

### User Communication
- Deprecation warning in CLI output
- Update in release notes
- Documentation migration guide
- Community announcement if applicable