# Specification 41: Auto-Accept Flag for Non-Interactive Operation

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: 37

## Context

Currently, when running `mmm cook` with the `--worktree` flag, the system prompts the user interactively after completion to ask if they want to merge the worktree. While this is great for interactive use, it creates friction in automated environments or when users want to run the command and walk away.

Additionally, after a worktree is merged, there's another prompt asking if the user wants to delete the worktree. This creates a two-step interactive process that prevents fully automated operation.

Users need a way to pre-approve these prompts so the entire cook process can run unattended from start to finish, automatically:
1. Creating the worktree
2. Running improvements
3. Merging the worktree when complete
4. Cleaning up the worktree after merge

## Objective

Add a `-y` or `--yes` flag to the cook command that automatically accepts all interactive prompts, enabling fully unattended operation of the improvement workflow.

## Requirements

### Functional Requirements
- Add `-y` and `--yes` flags to the cook command
- When flag is set, automatically answer "yes" to all prompts:
  - Worktree merge prompt after successful completion
  - Worktree deletion prompt after successful merge
- Maintain existing interactive behavior when flag is not set
- Flag should work in combination with `--worktree` flag
- No effect when not using worktrees (gracefully ignored)

### Non-Functional Requirements
- Clear documentation about what the flag auto-accepts
- Consistent with standard Unix CLI conventions (like `apt-get -y`)
- No performance impact on normal operation
- Safe defaults - only auto-accept on success conditions

## Acceptance Criteria

- [ ] `mmm cook -y --worktree` runs completely unattended
- [ ] `mmm cook --yes --worktree` works as long form
- [ ] Flag appears in `mmm cook --help` with clear description
- [ ] Auto-accepts worktree merge prompt on success
- [ ] Auto-accepts worktree deletion prompt after merge
- [ ] Does NOT auto-accept if improvement fails
- [ ] Works correctly in both TTY and non-TTY environments
- [ ] Existing interactive behavior preserved without flag
- [ ] Tests cover auto-accept scenarios

## Technical Details

### Implementation Approach

1. **Update CookCommand struct**
   ```rust
   #[derive(Debug, Args, Clone)]
   pub struct CookCommand {
       // ... existing fields ...
       
       /// Automatically answer yes to all prompts
       #[arg(short = 'y', long = "yes")]
       pub auto_accept: bool,
   }
   ```

2. **Pass flag through to improve logic**
   ```rust
   // In cook/mod.rs
   pub async fn run(cmd: CookCommand) -> Result<()> {
       // Convert to improve command
       let improve_cmd = ImproveCommand {
           // ... existing fields ...
           auto_accept: cmd.auto_accept,
       };
       // ... rest of implementation
   }
   ```

3. **Update merge prompt logic**
   ```rust
   // In improve/mod.rs
   if let Some(ref session) = worktree_session {
       let should_merge = if cmd.auto_accept {
           println!("Auto-accepting worktree merge (--yes flag set)");
           true
       } else if is_tty() {
           match prompt_for_merge(&session.name) {
               MergeChoice::Yes => true,
               MergeChoice::No => false,
           }
       } else {
           false
       };
       
       if should_merge {
           println!("Merging worktree {}...", session.name);
           merge_worktree(&session.name, cmd.auto_accept)?;
       } else {
           println!("\nTo merge changes later, run:");
           println!("  mmm worktree merge {}", session.name);
       }
   }
   ```

4. **Update worktree deletion prompt**
   ```rust
   // In worktree/manager.rs merge_session method
   pub fn merge_session(&self, name: &str, auto_accept: bool) -> Result<()> {
       // ... existing merge logic ...
       
       // After successful merge
       let should_delete = if auto_accept {
           println!("Auto-accepting worktree deletion (--yes flag set)");
           true
       } else {
           // Existing prompt logic
           prompt_for_deletion(name)
       };
       
       if should_delete {
           self.remove_worktree(name)?;
       }
   }
   ```

### Architecture Changes

- **cook/command.rs**: Add auto_accept flag
- **cook/mod.rs**: Pass flag to improve command
- **improve/command.rs**: Add auto_accept flag  
- **improve/mod.rs**: Use flag for merge prompt
- **worktree/manager.rs**: Use flag for deletion prompt

### Example Usage

```bash
# Fully automated workflow - runs to completion without prompts
$ mmm cook -y --worktree --focus "security"
Creating worktree session-abc123...
✓ Running improvements...
✓ Improvement session completed successfully!
Auto-accepting worktree merge (--yes flag set)
Merging worktree session-abc123...
✓ Successfully merged 5 commits into main branch
Auto-accepting worktree deletion (--yes flag set)
✓ Worktree removed

# Can also use long form
$ mmm cook --yes --worktree --focus "performance"

# Without flag, maintains interactive behavior
$ mmm cook --worktree --focus "testing"
✓ Improvement session completed successfully!
Would you like to merge the completed worktree now? (y/N): _

# In automated script or CI/CD
#!/bin/bash
mmm cook -y -w --focus "security" --max-iterations 5
mmm cook -y -w --focus "performance" --max-iterations 5
mmm cook -y -w --focus "testing" --max-iterations 5
```

### Safety Considerations

1. **Only on Success**: Auto-accept only triggers for successful completions
2. **Clear Logging**: Always log when auto-accepting for audit trail
3. **No Data Loss**: Merge is safe operation, deletion only after successful merge
4. **Flag Scope**: Only affects prompts, not the improvement process itself

## Dependencies

- **Prerequisites**: 
  - Spec 37 (Interactive worktree merge prompt) - Prompts to auto-accept
- **Affected Components**: 
  - `src/cook/command.rs` - Add flag
  - `src/cook/mod.rs` - Pass flag through
  - `src/improve/command.rs` - Add flag
  - `src/improve/mod.rs` - Use flag for prompts
  - `src/worktree/manager.rs` - Use flag for deletion
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Flag parsing correctly
  - Auto-accept logic branches
  - Safety checks (only on success)
- **Integration Tests**: 
  - Full automated workflow with flag
  - Flag has no effect without worktree
  - Verify prompts skipped in order
- **Manual Testing**: 
  - Script/automation scenarios
  - CI/CD pipeline usage
  - Combination with other flags

## Documentation Requirements

- **Code Documentation**: 
  - Document auto-accept behavior
  - Note safety considerations
- **User Documentation**: 
  - Update README with automation examples
  - Add CI/CD usage section
  - Include in help text
- **Architecture Updates**: 
  - Note non-interactive operation mode

## Implementation Notes

1. **Flag Naming**: Using `-y/--yes` to match common Unix tools (apt, yum, etc.)
2. **Fail-Safe**: Never auto-accept on failure conditions
3. **Audit Trail**: Always log when auto-accepting for debugging
4. **Future Prompts**: Design to handle future interactive prompts
5. **Partial Success**: Consider behavior for partial success scenarios

## Migration and Compatibility

- No breaking changes - adds functionality only
- Existing scripts continue to work
- Interactive users unaffected without flag
- Can combine with all existing flags

## Success Metrics

- Increased usage in automation scripts
- Reduced support issues about interactive prompts
- Successful CI/CD integrations
- No accidental data loss from auto-acceptance

## Future Enhancements

- `--no` flag to auto-decline all prompts
- Granular control (e.g., `--yes-merge` but not delete)
- Configuration file support for default behavior
- Integration with other automation tools