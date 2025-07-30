# Specification 43: MMM Command Initialization System

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

MMM depends on various .claude/commands being present in a project for proper functionality, but currently lacks a mechanism to install these necessary base commands to new git projects. While users are expected to customize and adjust claude commands for their specific needs, MMM should provide a way to bootstrap new projects with the default command set required for basic MMM operations.

Without these commands, users must manually copy command files from existing projects or create them from scratch, creating friction in the onboarding process and potentially leading to incomplete or incorrectly configured MMM installations.

## Objective

Add an initialization function to MMM that installs the default .claude/commands required for core MMM functionality in git repositories, while preserving the ability for users to customize these commands after installation.

## Requirements

### Functional Requirements
- Create `mmm init` subcommand to bootstrap .claude/commands in current git repository
- Bundle default MMM commands within the binary or as embedded resources
- Detect if .claude/commands already exist and handle appropriately (prompt for overwrite/skip)
- Create .claude directory structure if it doesn't exist
- Install core MMM commands: /mmm-code-review, /mmm-implement-spec, /mmm-lint, /mmm-product-enhance, /mmm-merge-worktree
- Support optional command selection (e.g., `mmm init --commands review,implement`)
- Preserve file permissions and executable bits where applicable
- Provide clear feedback about installed commands and next steps

### Non-Functional Requirements
- Installation should be idempotent - running multiple times should be safe
- Command templates should be easily maintainable within the codebase
- Process should complete quickly (< 1 second for default installation)
- Error messages should clearly indicate what went wrong and how to fix it

## Acceptance Criteria

- [ ] `mmm init` command exists and shows in help documentation
- [ ] Running `mmm init` in a git repository creates .claude/commands/ directory structure
- [ ] All core MMM commands are installed with correct content and permissions
- [ ] Running `mmm init` when commands already exist prompts user for action
- [ ] `--force` flag overwrites existing commands without prompting
- [ ] `--commands` flag allows selective command installation
- [ ] Init process validates that current directory is a git repository
- [ ] Clear success message shows which commands were installed
- [ ] Error handling for non-git directories, permission issues, and I/O failures
- [ ] Documentation updated with init command usage and examples

## Technical Details

### Implementation Approach
1. Add new `init` subcommand to main CLI structure
2. Embed command templates as static strings or include_str! resources
3. Create initialization module in src/init/ with command installation logic
4. Use git2 crate or shell commands to verify git repository status
5. Implement conflict detection and resolution strategies

### Architecture Changes
- New module: src/init/ containing initialization logic
- New CLI subcommand in main.rs
- Command templates stored in src/init/templates/ or as embedded strings

### Data Structures
```rust
pub struct InitCommand {
    /// Force overwrite existing commands
    force: bool,
    /// Specific commands to install (comma-separated)
    commands: Option<String>,
    /// Directory to initialize (defaults to current)
    path: Option<PathBuf>,
}

pub struct CommandTemplate {
    name: &'static str,
    content: &'static str,
    description: &'static str,
}
```

### APIs and Interfaces
- CLI: `mmm init [--force] [--commands <list>] [--path <dir>]`
- Internal API: `init::run(command: InitCommand) -> Result<()>`

## Dependencies

- **Prerequisites**: None
- **Affected Components**: CLI interface, main.rs
- **External Dependencies**: Possibly git2 for repository validation

## Testing Strategy

- **Unit Tests**: 
  - Test command template loading and validation
  - Test conflict detection logic
  - Test selective command installation
- **Integration Tests**: 
  - Test full init flow in temporary git repository
  - Test overwrite scenarios
  - Test error cases (non-git directory, permissions)
- **User Acceptance**: 
  - New user can run `mmm init` and immediately use `mmm cook`
  - Existing users can update their commands with `mmm init --force`

## Documentation Requirements

- **Code Documentation**: Document all public functions in init module
- **User Documentation**: 
  - Add init command to README.md
  - Create "Getting Started" section showing init → cook workflow
  - Document command customization process
- **Architecture Updates**: Update ARCHITECTURE.md with init module description

## Implementation Notes

- Consider using include_str! macro to embed command templates at compile time
- Command templates should match the latest versions from the MMM repository
- Future enhancement: Support downloading latest commands from GitHub
- Consider adding `mmm init --check` to verify command integrity
- May want to add version tracking to detect outdated commands

## Migration and Compatibility

- Existing MMM installations continue to work without running init
- Init command is purely additive - doesn't modify existing MMM functionality
- Commands installed by init are compatible with all MMM versions
- Future versions may update command templates while maintaining compatibility