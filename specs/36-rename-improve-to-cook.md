# Specification 36: Rename Improve Subcommand to Cook

**Category**: compatibility
**Priority**: medium
**Status**: draft
**Dependencies**: [35]

## Context

The current main command for MMM is `mmm improve`, which accurately describes what it does - improve code quality. However, for a tool named "Memento Mori" (remember death), the command name could be more evocative and memorable. The word "cook" suggests transformation, refinement, and the application of heat/pressure to create something better - all metaphors that align well with what the tool does to code.

Additionally, "cook" is shorter to type than "improve" and creates a more distinctive command-line experience. This would change the primary usage from `mmm improve` to `mmm cook`, making it more memorable and giving the tool a unique personality.

## Objective

Rename the `improve` subcommand to `cook` throughout the codebase while maintaining backward compatibility through command aliases. This creates a more memorable and distinctive CLI experience while preserving existing user workflows.

## Requirements

### Functional Requirements

1. **Primary Command Rename**
   - Change the main subcommand from `improve` to `cook`
   - Update all CLI parsing to recognize `cook` as the primary command
   - Maintain all existing functionality under the new name

2. **Backward Compatibility**
   - Keep `improve` as an alias for `cook` to avoid breaking existing scripts
   - Show a gentle deprecation notice when `improve` is used
   - Document the transition clearly

3. **Comprehensive Updates**
   - Update all documentation to use `cook` instead of `improve`
   - Change internal module names from `improve` to `cook`
   - Update error messages, help text, and logging
   - Rename improve directory to cook directory

4. **Flag Compatibility**
   - All existing flags work identically with the new command
   - Examples: `mmm cook --target 8.0`, `mmm cook -wn 3`, `mmm cook --map "*.rs"`

### Non-Functional Requirements

1. **User Experience**
   - Clear migration path for existing users
   - Intuitive for new users
   - Consistent terminology throughout

2. **Code Quality**
   - Clean refactoring without breaking changes
   - Maintain test coverage
   - Update all examples and scripts

## Acceptance Criteria

- [ ] Main command changed from `improve` to `cook` in CLI
- [ ] `improve` works as an alias with deprecation notice
- [ ] src/improve/ directory renamed to src/cook/
- [ ] All internal references updated (module names, functions, types)
- [ ] Documentation updated (README, help text, examples)
- [ ] Tests updated to use new command name
- [ ] Config files and workflows reference updated
- [ ] ARCHITECTURE.md updated with new module structure
- [ ] Example commands in all docs use `cook`
- [ ] Deprecation notice is helpful and not annoying

## Technical Details

### Implementation Approach

1. **CLI Changes**
   ```rust
   #[derive(Subcommand)]
   enum Commands {
       /// Cook your code to perfection (make it better)
       #[command(name = "cook", alias = "improve")]
       Cook(CookCommand),
       
       /// Manage git worktrees for parallel cooking sessions
       #[command(name = "worktree")]
       Worktree(WorktreeCommand),
   }
   ```

2. **Module Rename**
   - Rename `src/improve/` to `src/cook/`
   - Update all imports from `crate::improve` to `crate::cook`
   - Rename `ImproveCommand` to `CookCommand`
   - Update function names like `improve::run` to `cook::run`

3. **Deprecation Notice**
   When user runs `mmm improve`:
   ```
   Note: 'improve' has been renamed to 'cook'. Please use 'mmm cook' in the future.
   The 'improve' alias will be removed in a future version.
   ```

4. **Documentation Updates**
   - README.md: Change all examples to use `cook`
   - Help text: Update command descriptions
   - Error messages: Reference `cook` instead of `improve`
   - Comments: Update internal documentation

### Architecture Changes

Update ARCHITECTURE.md:
```
### 2. Cook Command (`src/cook/`)
- **mod.rs**: Core cooking loop with Claude CLI integration and mapping support
- **command.rs**: CLI with target, verbose, focus, config, map, args, and fail-fast flags
- **session.rs**: Minimal session data structures
- **workflow.rs**: Configurable workflow execution with variable substitution
- **git_ops.rs**: Thread-safe git operations
```

### Data Structures

```rust
// Renamed from ImproveCommand
pub struct CookCommand {
    // All fields remain the same
    pub target: Option<f32>,
    pub verbose: bool,
    pub focus: Option<String>,
    pub config: Option<PathBuf>,
    pub map: Vec<String>,
    pub args: Vec<String>,
    pub worktree: bool,
    pub max_iterations: Option<usize>,
    pub fail_fast: bool,
}

// Update environment variable names
const MMM_FOCUS: &str = "MMM_FOCUS";  // Keep same for compatibility
const MMM_TARGET: &str = "MMM_TARGET"; // Keep same for compatibility
```

## Dependencies

- **Prerequisites**: Spec 35 should be completed first to avoid conflicts
- **Affected Components**: All modules that reference improve command
- **External Dependencies**: User scripts and workflows using `mmm improve`

## Testing Strategy

- **Unit Tests**: Update all test names and assertions
- **Integration Tests**: Test both `cook` and `improve` commands work
- **Deprecation Tests**: Verify alias works and shows notice
- **Documentation Tests**: Ensure all examples use new syntax

## Documentation Requirements

- **Migration Guide**: 
  - Clear instructions for updating scripts
  - Timeline for deprecation
  - Benefits of the new naming
- **User Documentation**: 
  - Update all examples to use `mmm cook`
  - Add note about the alias
- **Code Documentation**: 
  - Update all inline comments
  - Update module documentation

## Implementation Notes

### Usage Examples After Change

```bash
# Basic usage (was: mmm improve)
mmm cook

# With options (was: mmm improve --target 8.0 --verbose)
mmm cook --target 8.0 --verbose

# Short form (was: mmm improve -wn 3)
mmm cook -wn 3

# With mapping (was: mmm improve --map "*.rs")
mmm cook --map "*.rs"

# With focus (was: mmm improve --focus performance)
mmm cook --focus performance
```

### Memorable Aspects

The "cook" metaphor works well:
- You're "cooking" your code to make it better
- Different "recipes" (workflows) for different needs  
- "Heat" (iterations) transforms raw code into refined code
- Works with the "mmm" sound (like "mmm, that's good cooking!")

## Migration and Compatibility

### Phase 1: Introduction (This PR)
- Add `cook` as primary command
- Keep `improve` as fully functional alias
- Show deprecation notice

### Phase 2: Documentation Push (Next Release)
- Update all official docs to use `cook`
- Blog post about the change
- Update any tutorials or guides

### Phase 3: Deprecation (Future Release)
- Make deprecation notice more prominent
- Consider removing alias after sufficient time

### Backward Compatibility
- Environment variables remain unchanged (MMM_*)
- Config file formats unchanged
- Git commit messages can remain the same
- API and library usage unaffected