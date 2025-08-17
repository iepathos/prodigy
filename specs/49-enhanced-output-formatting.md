---
number: 49
title: Enhanced Output Formatting and Verbosity Control
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-08-17
---

# Specification 49: Enhanced Output Formatting and Verbosity Control

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The current MMM output during `cook` command execution uses simple line-based output with emoji prefixes (ℹ️, 🔄, ✅, ❌, ⚠️) to indicate different message types. While functional, the output doesn't clearly delineate iteration boundaries, making it difficult to visually distinguish when one iteration ends and another begins. Additionally, there's no configurable verbosity control for shell command stdout/stderr or Claude command output, which can make debugging difficult when issues arise.

Currently, the display implementation in `src/cook/interaction/display.rs` provides basic progress display methods, but the iteration boundaries blend together in the output stream. Shell commands and Claude commands execute without showing their output unless there's an error, making it hard to understand what's happening during execution.

## Objective

Enhance the visual presentation of MMM cook output to make iterations clearly distinguishable and add configurable verbosity levels (-v, -vv, -vvv) to control the display of shell command stdout/stderr and Claude command output for improved debugging and monitoring capabilities.

## Requirements

### Functional Requirements
- Visual iteration separators that clearly mark iteration boundaries
- Iteration headers that stand out with iteration number and progress
- Summary boxes at the end of each iteration showing key statistics
- Configurable verbosity levels via CLI flags (-v, -vv, -vvv)
- Progressive detail levels: minimal (default), verbose (-v), debug (-vv), trace (-vvv)
- Shell command output streaming based on verbosity level
- Claude command output display based on verbosity level
- Preserve colored output when appropriate
- Support for both interactive and non-interactive terminals

### Non-Functional Requirements
- Minimal performance impact from output formatting
- Backward compatibility with existing output parsing scripts
- Clean fallback for environments without Unicode support
- Memory-efficient output buffering for large command outputs
- Thread-safe output handling for concurrent operations

## Acceptance Criteria

- [ ] Iterations are visually separated with clear boundaries
- [ ] Each iteration has a prominent header showing iteration number and total
- [ ] Iteration completion shows a summary box with duration and results
- [ ] -v flag shows shell command names and exit codes
- [ ] -vv flag additionally shows shell command stdout/stderr
- [ ] -vvv flag additionally shows Claude command full output
- [ ] Output remains clean and readable at default verbosity
- [ ] Progress indicators work correctly at all verbosity levels
- [ ] Non-interactive mode (CI/CD) produces appropriate output
- [ ] Output can be parsed by existing automation tools

## Technical Details

### Implementation Approach

1. **Enhanced Display Trait**
   ```rust
   pub trait ProgressDisplay: Send + Sync {
       // Existing methods...
       
       // New methods for iteration boundaries
       fn iteration_start(&self, current: u32, total: u32);
       fn iteration_end(&self, current: u32, duration: Duration, success: bool);
       fn step_start(&self, step: u32, total: u32, description: &str);
       fn step_end(&self, step: u32, success: bool);
       
       // Verbosity-aware output
       fn command_output(&self, output: &str, verbosity: VerbosityLevel);
       fn debug_output(&self, message: &str, min_verbosity: VerbosityLevel);
   }
   ```

2. **Verbosity Levels**
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
   pub enum VerbosityLevel {
       Quiet = 0,      // Minimal output (errors only)
       Normal = 1,     // Default: progress + results
       Verbose = 2,    // -v: command names + exit codes
       Debug = 3,      // -vv: + stdout/stderr
       Trace = 4,      // -vvv: + Claude output + internal details
   }
   ```

3. **Visual Formatting Elements**
   ```
   ╔════════════════════════════════════════════════════════════╗
   ║ ITERATION 1/10                                            ║
   ╚════════════════════════════════════════════════════════════╝
   
   [Step 1/5] shell: cargo build --release
   [Step 2/5] shell: cargo tarpaulin
   [Step 3/5] claude: /debtmap
   
   ┌─ Iteration 1 Summary ──────────────────────────────────────┐
   │ Duration: 12m 31s                                          │
   │ Steps Completed: 5/5                                       │
   │ Tests: ✅ Passed                                           │
   │ Commits: 2                                                 │
   └────────────────────────────────────────────────────────────┘
   ```

4. **CLI Argument Extension**
   ```rust
   #[derive(Debug, Args, Clone)]
   pub struct CookCommand {
       // Existing fields...
       
       /// Increase output verbosity (-v verbose, -vv debug, -vvv trace)
       #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count)]
       pub verbosity: u8,
       
       /// Decrease output verbosity (opposite of -v)
       #[arg(short = 'q', long = "quiet", conflicts_with = "verbosity")]
       pub quiet: bool,
   }
   ```

### Architecture Changes

- Extend `ProgressDisplay` trait with iteration and verbosity methods
- Create `OutputFormatter` module for consistent formatting
- Add `VerbosityFilter` to control output based on level
- Modify command executors to stream output based on verbosity
- Update orchestrator to use new iteration display methods

### Data Structures

```rust
pub struct OutputConfig {
    pub verbosity: VerbosityLevel,
    pub use_unicode: bool,
    pub use_color: bool,
    pub interactive: bool,
}

pub struct IterationSummary {
    pub number: u32,
    pub duration: Duration,
    pub steps_completed: u32,
    pub steps_total: u32,
    pub tests_passed: bool,
    pub commits_created: u32,
    pub files_changed: u32,
}

pub struct CommandOutput {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration: Duration,
}
```

### APIs and Interfaces

- `--verbose/-v` flag (can be repeated for increased verbosity)
- `--quiet/-q` flag for minimal output
- Environment variable `MMM_VERBOSITY` for default level
- Output remains parseable in all verbosity modes

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/cook/interaction/display.rs` - Core display implementation
  - `src/cook/command.rs` - CLI argument additions
  - `src/cook/orchestrator.rs` - Iteration display calls
  - `src/cook/workflow/executor.rs` - Step execution output
  - `src/subprocess/runner.rs` - Command output streaming
  - `src/commands/handlers/claude.rs` - Claude output handling
- **External Dependencies**: 
  - Consider `indicatif` for better progress bars
  - Consider `console` for terminal detection

## Testing Strategy

- **Unit Tests**: 
  - Test verbosity level parsing from CLI args
  - Test output filtering at each verbosity level
  - Test formatting with and without Unicode support
  - Test thread-safe output handling
- **Integration Tests**: 
  - Run cook with different verbosity levels
  - Verify output contains expected information at each level
  - Test output parsing by automation scripts
  - Test in non-interactive environments
- **Performance Tests**: 
  - Measure overhead of output formatting
  - Test with large command outputs
  - Verify no memory leaks with streaming output
- **User Acceptance**: 
  - Visual review of formatted output
  - Usability testing with different terminal sizes
  - Feedback on verbosity level usefulness

## Documentation Requirements

- **Code Documentation**: 
  - Document verbosity levels and their effects
  - Explain output formatting decisions
  - Document environment variables
- **User Documentation**: 
  - Update README with verbosity flag examples
  - Document output format for parsing
  - Provide troubleshooting guide for output issues
- **CLI Help**: 
  - Clear description of -v/-q flags
  - Examples of different verbosity levels
  - Note about MMM_VERBOSITY environment variable

## Implementation Notes

- Default verbosity should remain clean and concise
- Use box-drawing characters with ASCII fallback
- Consider terminal width for formatting
- Buffer output appropriately to avoid interleaving
- Respect NO_COLOR and TERM environment variables
- Stream large outputs instead of buffering entirely
- Consider progress bars for long-running operations
- Preserve ANSI color codes when appropriate
- Handle broken pipe errors gracefully
- Support both Unix and Windows terminals

## Migration and Compatibility

- Existing scripts parsing output should continue to work
- Default output format remains similar (just better organized)
- New formatting only applies to interactive terminals by default
- Environment variable to force old format if needed
- Gradual rollout with feature flag if concerns arise