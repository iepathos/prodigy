# Specification 32: CLI Help as Default Behavior

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

Currently, when running `mmm` without any arguments, the CLI automatically executes the `improve` command with default values. This behavior contradicts standard Unix CLI conventions where running a command without arguments typically displays help information to guide users on available options and usage.

The current implementation (in `src/main.rs` lines 101-104) treats the absence of a subcommand as an implicit request to run `improve`:

```rust
None => {
    // Default to improve command with default values
    run_improve(false, None, None, 10, false).await
}
```

This anti-pattern can lead to:
1. Unexpected behavior for new users who expect help information
2. Accidental execution of code improvements when users just want to explore the tool
3. Deviation from established CLI best practices and user expectations
4. Confusion about available commands and options

## Objective

Modify the MMM CLI to display help information when invoked without arguments, aligning with Unix CLI conventions and improving user experience.

## Requirements

### Functional Requirements
- Running `mmm` without arguments displays help information
- Help output includes all available commands and their descriptions
- Help output shows global options (like `--verbose`)
- Existing behavior of explicit commands remains unchanged
- `mmm improve` still works exactly as before
- Help can still be accessed via `mmm --help` or `mmm -h`

### Non-Functional Requirements
- No performance impact on command execution
- Maintain backward compatibility for scripts using explicit commands
- Clear and informative help text
- Consistent with clap's built-in help formatting

## Acceptance Criteria

- [ ] Running `mmm` with no arguments displays help text
- [ ] Help text shows all available subcommands (improve, worktree)
- [ ] Help text includes global options (--verbose)
- [ ] `mmm improve` continues to work as expected
- [ ] `mmm --help` and `mmm -h` continue to work
- [ ] Subcommand help works (e.g., `mmm improve --help`)
- [ ] No regression in existing functionality
- [ ] Documentation updated to reflect new behavior

## Technical Details

### Implementation Approach

1. **Modify Main Function**
   - Remove the default behavior of running `improve` when no subcommand is provided
   - Instead, print help information using clap's built-in help functionality
   - Ensure proper exit code (0 for help display)

2. **Leverage Clap's Help System**
   - Use clap's `print_help()` or similar method
   - Maintain consistent formatting with other help outputs
   - Ensure all commands and options are properly documented

3. **Error Handling**
   - Gracefully handle the case of no subcommand
   - Provide clear guidance to users on how to proceed

### Architecture Changes

The change is minimal and localized to the main function in `src/main.rs`:

```rust
let result = match cli.command {
    Some(Commands::Improve { ... }) => run_improve(...).await,
    Some(Commands::Worktree { command }) => run_worktree_command(command).await,
    None => {
        // Display help instead of defaulting to improve
        let mut cmd = Cli::command();
        cmd.print_help()?;
        println!(); // Add blank line for better formatting
        return Ok(());
    }
};
```

### Data Structures
No changes to data structures required.

### APIs and Interfaces
No changes to APIs or interfaces. Only the default behavior when no command is specified changes.

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/main.rs`: Main entry point
- **External Dependencies**: 
  - Existing clap dependency (already in use)

## Testing Strategy

- **Unit Tests**: 
  - Test that no subcommand results in help display
  - Verify exit code is 0 when displaying help
- **Integration Tests**: 
  - Test full CLI invocation without arguments
  - Verify help content includes all commands
  - Test that explicit commands still work
- **Performance Tests**: 
  - Ensure help display is instantaneous
- **User Acceptance**: 
  - Help text is clear and informative
  - New users can understand how to use the tool
  - Follows expected CLI patterns

## Documentation Requirements

- **Code Documentation**: 
  - Update comments in main.rs to reflect new behavior
  - Document the rationale for following CLI conventions
- **User Documentation**: 
  - Update README.md to show `mmm --help` in examples
  - Note that running `mmm` alone shows help
  - Update any quickstart guides
- **Architecture Updates**: 
  - None required

## Implementation Notes

1. **Help Content Quality**: Ensure all commands have clear, concise descriptions
2. **Subcommand Documentation**: Review and improve help text for all subcommands
3. **Examples**: Consider adding examples to help output if clap supports it
4. **Exit Codes**: Follow standard convention (0 for successful help display)
5. **Backward Compatibility**: This change only affects the no-argument case
6. **User Communication**: Consider adding a hint like "Use 'mmm improve' to start improving your code"

## Migration and Compatibility

- No migration required as this only changes default behavior
- Scripts and automation using explicit commands are unaffected
- Users who relied on the default behavior will need to explicitly use `mmm improve`
- Consider adding a note in the next release about this behavior change