# Specification 22: Configurable Iteration Limit

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: [21-configurable-workflow]

## Context

Currently, MMM has a hardcoded maximum of 10 iterations when running the `mmm improve` command. This limit is defined as a constant in the codebase and cannot be changed by users. Different projects and improvement scenarios may benefit from different iteration limits - some may need more iterations for complex refactoring, while others may want fewer iterations for quick fixes.

## Objective

Add a command-line option to the `mmm improve` command that allows users to specify the maximum number of iterations to run, while maintaining the current default of 10 iterations for backward compatibility.

## Requirements

### Functional Requirements
- Add `--max-iterations` (or `-n`) flag to the `mmm improve` command
- Accept positive integer values (1 or greater)
- Default to 10 iterations if not specified
- Display the configured max iterations in verbose output
- Stop improvement loop when either target score is reached OR max iterations is hit

### Non-Functional Requirements
- Maintain backward compatibility - existing commands should work unchanged
- Clear error messages for invalid iteration values
- Update help text to document the new option

## Acceptance Criteria

- [ ] Command `mmm improve --max-iterations 5` limits the run to 5 iterations
- [ ] Command `mmm improve -n 20` allows up to 20 iterations
- [ ] Command `mmm improve` (without flag) still defaults to 10 iterations
- [ ] Invalid values (0, negative, non-numeric) produce clear error messages
- [ ] Help text (`mmm improve --help`) shows the new option with description
- [ ] Verbose output displays "Iteration X/Y" where Y is the configured maximum
- [ ] Early termination message distinguishes between "target reached" and "max iterations reached"

## Technical Details

### Implementation Approach
1. Add `max_iterations` field to `ImproveCommand` struct in `src/improve/command.rs`
2. Pass the value through to the improvement loop in `src/improve/mod.rs`
3. Update loop counter display to show current/max iterations
4. Modify termination conditions to check both target score and iteration count

### Architecture Changes
- No architectural changes required
- Simple addition to existing command structure

### Data Structures
```rust
// In src/improve/command.rs
pub struct ImproveCommand {
    /// Target quality score (default: 8.0)
    #[arg(long, default_value = "8.0")]
    pub target: f32,

    /// Show detailed progress
    #[arg(long)]
    pub show_progress: bool,

    /// Focus directive for improvements
    #[arg(long)]
    pub focus: Option<String>,
    
    /// Maximum number of iterations to run (default: 10)
    #[arg(short = 'n', long, default_value = "10")]
    pub max_iterations: u32,
}
```

### APIs and Interfaces
- CLI interface extended with new flag
- No changes to internal APIs

## Dependencies

- **Prerequisites**: None (can be implemented independently)
- **Affected Components**: 
  - `src/improve/command.rs` - Add new CLI argument
  - `src/improve/mod.rs` - Use configurable limit in loop
  - `src/improve/workflow.rs` - Pass through max iterations if using workflow config
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Test argument parsing with various values
  - Test validation of iteration limits
- **Integration Tests**: 
  - Run improvement with different iteration limits
  - Verify early termination at configured limit
- **Performance Tests**: Not required
- **User Acceptance**: 
  - Verify help text is clear
  - Test common use cases (quick fix with 1-2 iterations, deep refactor with 20+ iterations)

## Documentation Requirements

- **Code Documentation**: Document the new field and its purpose
- **User Documentation**: 
  - Update README.md with example usage
  - Add to command help text
- **Architecture Updates**: None required

## Implementation Notes

- Consider showing a warning if max_iterations is set very high (e.g., > 50)
- The iteration counter in output should be 1-based for user friendliness
- If workflow configuration also specifies max_iterations, command-line should take precedence
- Consider adding a `--no-limit` flag for unlimited iterations (set max to u32::MAX)

## Migration and Compatibility

- No breaking changes - default behavior remains the same
- No migration required for existing users
- Future enhancement: Could also add this to workflow configuration file (`.mmm/workflow.toml`)