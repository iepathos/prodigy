---
number: 77
title: Shell Completions
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-17
---

# Specification 77: Shell Completions

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Modern CLI tools provide shell completions for improved user experience, reducing typing errors and accelerating command discovery. Prodigy lacks shell completion support, requiring users to manually type commands and remember syntax.

## Objective

Implement comprehensive shell completion support for bash, zsh, fish, PowerShell, and elvish, providing intelligent context-aware completions for commands, options, file paths, and workflow names.

## Requirements

### Functional Requirements
- Generate completion scripts for bash, zsh, fish, PowerShell, elvish
- Complete command names and subcommands
- Complete option flags (--help, --verbose, etc.)
- Complete file paths for workflow files (*.yaml)
- Complete workflow names from .prodigy/ directory
- Provide contextual completions based on command
- Support dynamic completions for custom values
- Include completion descriptions/hints
- Auto-install completions with cargo install
- Provide manual installation instructions
- Support completion updates without reinstalling

### Non-Functional Requirements
- Completions respond within 50ms
- No performance impact when completions not used
- Support shell-specific features optimally
- Maintain completions across Prodigy updates
- Work in restricted/sandboxed environments

## Acceptance Criteria

- [ ] Completion scripts generated for all 5 shells
- [ ] Tab completion works for all commands and subcommands
- [ ] Option completions include descriptions
- [ ] File path completion filters for .yaml files
- [ ] Dynamic completions work for workflow names
- [ ] Installation automatically sets up completions
- [ ] Manual installation documented for each shell
- [ ] Completions update when CLI changes
- [ ] Performance meets <50ms response requirement
- [ ] Completions work in common terminal emulators

## Technical Details

### Implementation Approach
1. Integrate clap_complete for static completions
2. Implement dynamic completion handlers
3. Create installation scripts for each shell
4. Add completion generation to build process
5. Implement completion testing framework

### Architecture Changes
- Add completions module using clap_complete
- Create shell-specific completion generators
- Implement dynamic completion handlers
- Add installation logic to cargo install

### Shell-Specific Features
```rust
use clap_complete::{generate, Generator, Shell};

// Static completions
fn generate_completions<G: Generator>(shell: G, app: &mut Command) {
    generate(shell, app, "prodigy", &mut io::stdout());
}

// Dynamic completions for workflow names
fn complete_workflow_names() -> Vec<String> {
    // Read .yaml files from current directory
    // Read workflow history from .prodigy/
    // Return sorted, deduplicated list
}
```

### Installation Paths
```bash
# Bash
~/.bash_completion.d/prodigy

# Zsh
/usr/local/share/zsh/site-functions/_prodigy

# Fish
~/.config/fish/completions/prodigy.fish

# PowerShell
$PROFILE\Scripts\prodigy.ps1

# Elvish
~/.elvish/lib/prodigy.elv
```

### APIs and Interfaces
- `prodigy completions bash` - Generate bash completions
- `prodigy completions zsh` - Generate zsh completions
- `prodigy completions fish` - Generate fish completions
- `prodigy completions powershell` - Generate PowerShell completions
- `prodigy completions elvish` - Generate elvish completions
- `prodigy completions --install` - Auto-install for detected shell

## Dependencies

- **Prerequisites**: None
- **Affected Components**: CLI parser, build system
- **External Dependencies**:
  - clap_complete (for static completions)
  - clap_complete_fig (for Fig support)
  - Shell detection libraries

## Testing Strategy

- **Unit Tests**: Test completion generation logic
- **Integration Tests**: Verify completions in each shell
- **Performance Tests**: Measure completion response time
- **User Acceptance**: Test with power users in each shell

## Documentation Requirements

- **Code Documentation**: Document completion system
- **User Documentation**: Add installation instructions to README
- **Architecture Updates**: Document dynamic completion system

## Implementation Notes

- Use clap's built-in completion generation where possible
- Cache dynamic completions for performance
- Handle edge cases (spaces in names, special characters)
- Consider Fig support for enhanced terminal experience
- Test in Docker containers for each shell
- Provide uninstall instructions

## Migration and Compatibility

- No breaking changes to existing functionality
- Completions are optional enhancement
- Support gradual rollout per shell
- Maintain backward compatibility with older shells