# Specification 37: Interactive Worktree Merge Prompt

**Category**: parallel
**Priority**: medium
**Status**: draft
**Dependencies**: 24, 25, 26, 29

## Context

Currently, when an MMM worktree-based improvement task completes, the system simply prints a message telling the user to run `mmm worktree merge <worktree-name>`. This requires the user to:
1. Copy or remember the worktree name
2. Type a new command
3. Manually execute the merge

This adds friction to the workflow and makes the automated improvement process feel incomplete. For a truly seamless experience, we should prompt the user immediately upon completion and handle the merge for them if they agree.

## Objective

Enhance the worktree-based improvement workflow by prompting the user to merge completed worktrees immediately upon successful completion, executing the merge automatically if they agree.

## Requirements

### Functional Requirements
- When an `mmm improve --worktree` session completes successfully, prompt the user: "Would you like to merge the completed worktree now? (y/N)"
- If user responds with 'y' or 'yes' (case-insensitive), execute the worktree merge automatically
- If user responds with 'n', 'no', or just presses Enter, display the manual merge command as before
- Update worktree state to track whether a merge prompt was shown and the user's response
- Ensure prompt works correctly in both TTY and non-TTY environments

### Non-Functional Requirements
- Minimal delay between completion and prompt
- Clear, concise prompt messaging
- Graceful handling of non-interactive environments (CI/CD)
- Preserve existing behavior when running without --worktree flag

## Acceptance Criteria

- [ ] Successful worktree sessions show merge prompt
- [ ] Typing 'y' or 'yes' triggers automatic merge
- [ ] Typing 'n', 'no', or Enter skips merge and shows manual command
- [ ] Non-TTY environments skip prompt and show manual command
- [ ] Failed sessions do not show merge prompt
- [ ] WorktreeState tracks prompt interaction
- [ ] Tests cover interactive and non-interactive scenarios

## Technical Details

### Implementation Approach

1. **Update improve/mod.rs**
   ```rust
   // After successful worktree session completion
   if let Some(ref session) = worktree_session {
       if is_tty() {
           match prompt_for_merge(&session.name) {
               MergeChoice::Yes => {
                   println!("Merging worktree {}...", session.name);
                   merge_worktree(&session.name)?;
               },
               MergeChoice::No => {
                   println!("\nTo merge changes later, run:");
                   println!("  mmm worktree merge {}", session.name);
               }
           }
       } else {
           // Non-interactive environment
           println!("\nWorktree completed. To merge changes, run:");
           println!("  mmm worktree merge {}", session.name);
       }
   }
   ```

2. **Add Prompt Utilities**
   ```rust
   use std::io::{self, Write};
   
   enum MergeChoice {
       Yes,
       No,
   }
   
   fn prompt_for_merge(worktree_name: &str) -> MergeChoice {
       print!("\nWould you like to merge the completed worktree now? (y/N): ");
       io::stdout().flush().unwrap();
       
       let mut input = String::new();
       io::stdin().read_line(&mut input).unwrap_or_default();
       
       match input.trim().to_lowercase().as_str() {
           "y" | "yes" => MergeChoice::Yes,
           _ => MergeChoice::No,
       }
   }
   
   fn is_tty() -> bool {
       atty::is(atty::Stream::Stdin) && atty::is(atty::Stream::Stdout)
   }
   ```

3. **Execute Merge**
   ```rust
   fn merge_worktree(worktree_name: &str) -> Result<()> {
       // Create WorktreeManager instance
       let repo_path = std::env::current_dir()?;
       let worktree_manager = WorktreeManager::new(repo_path)?;
       
       // Execute merge using existing logic
       worktree_manager.merge_session(worktree_name)?;
       
       Ok(())
   }
   ```

4. **Update WorktreeState**
   ```rust
   // Add to WorktreeState struct
   pub struct WorktreeState {
       // ... existing fields ...
       pub merge_prompt_shown: bool,
       pub merge_prompt_response: Option<String>, // "yes", "no", "skipped"
   }
   ```

### Architecture Changes

- **improve/mod.rs**: Add merge prompt logic after successful completion
- **worktree/state.rs**: Track prompt interaction in state
- **Cargo.toml**: Add `atty` crate for TTY detection

### Example Workflow

```bash
# Start improvement with worktree
$ mmm improve --worktree --focus "error handling"

# ... improvement process runs ...

✓ Improvement session completed successfully!

Would you like to merge the completed worktree now? (y/N): y
Merging worktree session-abc123...
✓ Successfully merged 5 commits into main branch

# Alternative flow (user chooses no)
Would you like to merge the completed worktree now? (y/N): n

To merge changes later, run:
  mmm worktree merge session-abc123
```

## Dependencies

- **Prerequisites**: 
  - Spec 24 (Git worktree isolation) - Base worktree functionality
  - Spec 25 (Claude-assisted merge) - Merge functionality to invoke
  - Spec 26 (Worktree CLI flag) - --worktree flag support
  - Spec 29 (Centralized state) - State tracking infrastructure
- **Affected Components**: 
  - `src/improve/mod.rs` - Add prompt logic
  - `src/worktree/state.rs` - Track prompt interaction
  - `Cargo.toml` - Add atty dependency
- **External Dependencies**: 
  - `atty` crate for TTY detection

## Testing Strategy

- **Unit Tests**: 
  - Prompt parsing (y, yes, n, no, empty input)
  - TTY detection mocking
  - State update verification
- **Integration Tests**: 
  - Full workflow with automatic merge
  - Full workflow with manual merge choice
  - Non-TTY environment behavior
- **Manual Testing**: 
  - Interactive prompt in real terminal
  - Pipe/redirect scenarios
  - CI environment behavior

## Documentation Requirements

- **Code Documentation**: 
  - Document prompt behavior and choices
  - Explain TTY detection logic
- **User Documentation**: 
  - Update README with new interactive behavior
  - Add note about non-interactive environments
  - Update workflow examples
- **Architecture Updates**: 
  - Note interactive components in improve flow

## Implementation Notes

1. **Default to No**: Following Unix convention, default to the safer option (not merging) if user just presses Enter
2. **TTY Detection**: Only show prompt in interactive terminals, skip in CI/CD or piped output
3. **Error Handling**: If merge fails after user says yes, show clear error and manual merge command
4. **State Tracking**: Record prompt interaction for analytics and debugging
5. **Timeout Consideration**: For now, no timeout on prompt (user can take their time)

## Migration and Compatibility

- No breaking changes - adds functionality only
- Existing scripts using --worktree continue to work
- Non-interactive environments maintain current behavior
- Can be disabled with future flag if needed (e.g., --no-prompt)

## Success Metrics

- Increased percentage of worktrees merged immediately after completion
- Reduced time between completion and merge
- Positive user feedback on workflow smoothness
- No issues in CI/CD environments

## Future Enhancements

- Configurable default choice (always yes/no)
- Timeout with auto-decline after N seconds
- Show diff preview before merge prompt
- Option to squash commits during merge