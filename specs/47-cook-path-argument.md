# Specification 47: Cook Path Argument

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

Currently, the `mmm cook` command operates exclusively on the current working directory. Users must navigate to the target repository directory before running the command. This creates friction when working with multiple projects or when invoking MMM from scripts or automation tools that may not be in the target directory.

Adding a path argument would allow users to specify the repository directory to analyze and improve, making the tool more flexible and easier to integrate into various workflows.

## Objective

Add an optional path argument to the `cook` command that allows users to specify the repository directory to run in. If not provided, the command should continue to use the current working directory as it does today.

## Requirements

### Functional Requirements
- Add optional path argument to `cook` command
- Validate that the provided path exists and is a valid directory
- Ensure the path contains a git repository (required for MMM operations)
- Change to the specified directory before executing improvement workflow
- Maintain backward compatibility - no path means use current directory
- Path argument should work with all existing flags and options
- Support both absolute and relative paths
- Handle path normalization across platforms

### Non-Functional Requirements
- Clear error messages for invalid paths
- No performance impact when path is not specified
- Consistent behavior across different operating systems
- Maintain thread safety for directory operations

## Acceptance Criteria

- [ ] `mmm cook` without path argument works exactly as before (uses cwd)
- [ ] `mmm cook /path/to/repo` runs cook command in specified directory
- [ ] `mmm cook ./relative/path` correctly resolves relative paths
- [ ] `mmm cook ~/projects/myrepo` expands tilde notation correctly
- [ ] Error shown if path does not exist: "Error: Directory not found: /invalid/path"
- [ ] Error shown if path is not a directory: "Error: Path is not a directory: /path/to/file.txt"
- [ ] Error shown if path is not a git repository: "Error: Not a git repository: /path/to/non-git-dir"
- [ ] Path argument works with all flags: `mmm cook /path/to/repo --focus security --worktree`
- [ ] Worktree operations use the specified repository path correctly
- [ ] Context analysis runs in the specified directory
- [ ] All git operations execute in the correct directory
- [ ] Original working directory is preserved after command completion

## Technical Details

### Implementation Approach

1. **CLI Argument Addition**
   - Add `path: Option<PathBuf>` field to `CookCommand` struct
   - Use positional argument: `#[arg(value_name = "PATH", help = "Repository path to run in")]`
   - Argument should be optional to maintain backward compatibility

2. **Path Validation and Resolution**
   - Expand tilde notation for home directory
   - Resolve relative paths to absolute paths
   - Verify path exists and is a directory
   - Check for `.git` directory to ensure it's a repository

3. **Directory Management**
   - Save current directory before changing
   - Change to target directory early in execution
   - Ensure all subsequent operations use the new working directory
   - Consider restoring original directory on exit (optional)

4. **Integration Points**
   - Update `run_standard` and `run_with_mapping` functions
   - Ensure `WorktreeManager::new` receives correct repository path
   - Update context analysis to use specified path
   - Adjust git operations to work in correct directory

### Architecture Changes

No major architectural changes required. The modification is localized to:
- `src/cook/command.rs` - Add path field
- `src/cook/mod.rs` - Add path validation and directory change logic

### Data Structures

```rust
#[derive(Debug, Args, Clone)]
pub struct CookCommand {
    /// Repository path to run in (defaults to current directory)
    #[arg(value_name = "PATH", help = "Repository path to run in")]
    pub path: Option<PathBuf>,
    
    // ... existing fields remain unchanged
}
```

### APIs and Interfaces

No changes to external APIs. The CLI interface gains an optional positional argument.

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `cook` module (command parsing and execution)
  - Potentially affects how paths are passed to `WorktreeManager`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Path validation logic (exists, is directory, is git repo)
  - Path expansion (tilde, relative paths)
  - Error handling for invalid paths
  
- **Integration Tests**: 
  - Run cook command with various path arguments
  - Verify operations execute in correct directory
  - Test with worktree flag and path argument
  - Test with mapping and path argument
  
- **User Acceptance**: 
  - Test running MMM from outside repository directory
  - Test in automation scripts
  - Cross-platform testing (Windows, macOS, Linux)

## Documentation Requirements

- **Code Documentation**: 
  - Document path argument in command struct
  - Add examples to function documentation
  
- **User Documentation**: 
  - Update README.md with path argument usage
  - Add examples: `mmm cook ~/projects/myapp --focus performance`
  - Document in help text

- **Architecture Updates**: None required

## Implementation Notes

- Use `std::env::set_current_dir()` to change working directory
- Consider using `dunce` crate for better Windows path handling if needed
- Ensure thread safety if directory change affects concurrent operations
- Path validation should happen early to fail fast
- Consider impact on relative paths in configuration files

## Migration and Compatibility

- No breaking changes - existing usage remains unchanged
- No migration required - optional argument maintains compatibility
- Scripts using `cd` before `mmm cook` will continue to work
- New scripts can use path argument for cleaner implementation