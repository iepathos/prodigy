# Specification 35: Unified Improve Command with Mapping

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [21, 28, 33]

## Context

Currently, MMM has two separate commands for different improvement workflows:
- `mmm improve` - Runs the standard code review → implement → lint cycle
- `mmm implement` - Batch implements pre-written specifications without code review

This separation creates unnecessary complexity and duplicates functionality. Since the improve command already supports configurable workflows via config files, we should unify these capabilities into a single, more flexible improve command.

The key insight is that both commands essentially run improvement loops - they just differ in:
1. What files/inputs they process
2. What sequence of Claude commands they execute

By adding a `--map` flag to the improve command, we can support batch processing of multiple inputs while maintaining a single, coherent interface.

## Objective

Unify the improve and implement commands by:
1. Adding a `--map` flag to improve that accepts file patterns
2. Running a separate improvement loop for each matched file
3. Enhancing config files to support parameterized commands
4. Removing the implement subcommand entirely

This creates a more flexible system where users can define any workflow in a config file and apply it to multiple inputs.

## Requirements

### Functional Requirements

1. **Map Flag for Improve Command**
   - Add `--map <pattern>` flag that accepts glob patterns
   - Each matched file triggers a separate improvement loop
   - Pass the matched file path as an argument to configured commands
   - Support multiple patterns (e.g., `--map "specs/*.md" --map "docs/*.md"`)

2. **Enhanced Command Arguments in Config**
   - Support passing mapped file paths to commands
   - Allow variable substitution in command arguments
   - Support both positional and named arguments
   - Example: `{ command = "mmm-implement-spec", args = ["$FILE"] }`

3. **Parallel Processing Options**
   - When using --map with --worktree, create separate worktrees for each file
   - Support concurrent processing of multiple files
   - Maintain clear progress tracking across all loops

4. **Backward Compatibility**
   - Existing improve command behavior unchanged when --map not used
   - Existing config files continue to work
   - Clear migration path from implement command

### Non-Functional Requirements

1. **Performance**
   - Efficient file pattern matching
   - Minimal overhead when not using --map
   - Smart batching for large file sets

2. **Usability**
   - Clear progress indication for multi-file processing
   - Intuitive command-line interface
   - Helpful error messages for pattern matching

3. **Flexibility**
   - Support any workflow via config files
   - Allow custom variable substitution
   - Enable complex multi-step processes

## Acceptance Criteria

- [ ] Remove implement subcommand and its module
- [ ] Add --map flag to improve command
- [ ] Implement file pattern matching and iteration
- [ ] Support $FILE variable substitution in command configs
- [ ] Create example config for specification implementation workflow
- [ ] Update documentation to show unified approach
- [ ] Maintain all existing improve command functionality
- [ ] Add tests for mapping functionality
- [ ] Support concurrent execution with worktrees

## Technical Details

### Implementation Approach

1. **Remove Implement Module**
   - Delete src/implement/ directory
   - Remove implement command from main.rs
   - Update lib.rs exports

2. **Enhance Improve Command**
   ```rust
   pub struct ImproveCommand {
       // Existing fields...
       
       /// File patterns to map over
       #[arg(long, value_name = "PATTERN")]
       pub map: Vec<String>,
   }
   ```

3. **Variable Substitution in Commands**
   ```rust
   pub enum CommandArg {
       Literal(String),
       Variable(String), // e.g., "$FILE", "$INDEX", "$TOTAL"
   }
   ```

4. **Example Implement Config**
   ```toml
   # implement.toml - Replaces mmm implement functionality
   [workflow]
   commands = [
       {
           name = "mmm-implement-spec",
           args = ["$FILE"],
           extract_spec_id = true
       },
       {
           name = "mmm-lint"
       }
   ]
   max_iterations = 1  # Each file processed once
   ```

5. **Usage Examples**
   ```bash
   # Replace: mmm implement specs/*.md
   mmm improve --config implement.toml --map "specs/*.md"
   
   # With worktrees for parallel execution
   mmm improve --config implement.toml --map "specs/*.md" --worktree
   
   # Multiple patterns
   mmm improve --config security.toml --map "src/**/*.rs" --map "tests/**/*.rs"
   ```

### Architecture Changes

1. **Improve Module Updates**
   - Add mapping logic to improve/mod.rs
   - Enhance workflow executor for variable substitution
   - Update progress tracking for multi-file operations

2. **Config Module Updates**
   - Extend command parsing for variables
   - Add validation for variable references
   - Support new command argument format

3. **Removal of Implement Module**
   - Delete all implement-related code
   - Update imports and exports
   - Clean up unused dependencies

### Data Structures

```rust
// Enhanced workflow execution context
pub struct WorkflowContext {
    pub variables: HashMap<String, String>,
    pub current_file: Option<PathBuf>,
    pub file_index: usize,
    pub total_files: usize,
}

// Command with variable support
pub struct WorkflowCommand {
    pub name: String,
    pub args: Vec<CommandArg>,
    pub options: HashMap<String, CommandArg>,
}
```

## Dependencies

- **Prerequisites**: 
  - Spec 21 (Configurable Workflows) - For workflow system
  - Spec 28 (Structured Commands) - For command objects
  - Spec 33 (Batch Implementation) - Functionality to replace
- **Affected Components**: 
  - Improve module (major changes)
  - Config module (command parsing)
  - Main CLI (remove implement)
- **Breaking Changes**: 
  - Removal of mmm implement command
  - Users must migrate to new approach

## Testing Strategy

- **Unit Tests**: 
  - File pattern matching logic
  - Variable substitution in commands
  - Multi-file workflow execution
- **Integration Tests**: 
  - End-to-end mapping workflows
  - Worktree integration with mapping
  - Config file parsing with variables
- **Migration Tests**: 
  - Verify implement configs work with new approach
  - Test backward compatibility

## Documentation Requirements

- **Migration Guide**: 
  - How to convert from implement to improve --map
  - Example configurations for common use cases
- **User Documentation**: 
  - Update README with unified approach
  - Remove implement command documentation
  - Add mapping examples
- **Architecture Updates**: 
  - Remove implement module from ARCHITECTURE.md
  - Document variable substitution system

## Implementation Notes

### Variable System
- `$FILE` - Current file being processed
- `$FILE_STEM` - Filename without extension
- `$FILE_NAME` - Filename with extension
- `$INDEX` - Current file index (1-based)
- `$TOTAL` - Total number of files
- Custom variables from workflow context

### Progress Reporting
When processing multiple files, show clear progress:
```
Processing 5 files with workflow 'implement'...
[1/5] specs/30-feature.md ✓ (2m 15s)
[2/5] specs/31-enhancement.md ✓ (1m 42s)
[3/5] specs/32-bugfix.md ⚡ (in progress...)
[4/5] specs/33-refactor.md ⏸ (pending)
[5/5] specs/34-docs.md ⏸ (pending)
```

### Error Handling
- By default, continue processing remaining files on error
- Add --fail-fast flag to stop on first error
- Clear reporting of which files succeeded/failed

## Migration and Compatibility

### Migration Path
1. Users of `mmm implement specs/*.md` should use:
   `mmm improve --config implement.toml --map "specs/*.md"`

2. Provide built-in implement.toml config that replicates implement behavior

3. Clear deprecation notice in next release before removal

### Config File Evolution
Support both old and new command formats:
```toml
# Old format (still supported)
commands = ["mmm-code-review", "mmm-implement-spec", "mmm-lint"]

# New format with arguments
commands = [
    { name = "mmm-implement-spec", args = ["$FILE"] },
    { name = "mmm-lint" }
]
```

## Future Considerations

This unified approach opens possibilities for:
- Custom workflows for different file types
- Conditional command execution based on file patterns
- Aggregated reporting across multiple files
- Pipeline-style processing with intermediate results