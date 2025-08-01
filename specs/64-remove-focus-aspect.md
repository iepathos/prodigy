# Specification 64: Remove Focus Aspect

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The current MMM implementation includes a "focus" parameter that can be passed to the cook command (e.g., `--focus "performance"`). This focus directive is then:

1. Passed as a CLI argument to the cook command
2. Stored in the ExecutionEnvironment
3. Set as an environment variable `MMM_FOCUS` only for the first step of a workflow
4. Used when creating worktree sessions
5. Displayed in the UI when starting a workflow
6. Used by Claude commands to prioritize certain aspects of analysis

However, this approach has several drawbacks:
- It limits flexibility by only applying to the first command in a workflow
- It creates hidden state that's not obvious to users
- It complicates the command execution model
- The same functionality can be achieved more explicitly through command arguments

## Objective

Remove the focus aspect entirely from MMM and provide guidance on using command arguments (`--args`) as a more flexible and explicit alternative for passing directives to individual commands.

## Requirements

### Functional Requirements
- Remove focus field from CookCommand struct
- Remove focus from ExecutionEnvironment
- Remove MMM_FOCUS environment variable handling
- Update worktree creation to not use focus
- Update all Claude commands to use explicit arguments instead
- Maintain backward compatibility by ignoring focus if provided

### Non-Functional Requirements
- No breaking changes to existing workflows
- Clear migration path for users currently using focus
- Improved clarity in how directives are passed to commands

## Acceptance Criteria

- [ ] Focus parameter removed from CLI interface
- [ ] Focus removed from all internal data structures
- [ ] MMM_FOCUS environment variable no longer set
- [ ] Worktree sessions created without focus parameter
- [ ] All tests updated and passing
- [ ] Documentation updated to show --args usage patterns
- [ ] Migration guide created for existing focus users

## Technical Details

### Implementation Approach

1. **Remove from CookCommand**
   ```rust
   // Before
   pub struct CookCommand {
       pub playbook: PathBuf,
       pub path: Option<PathBuf>,
       pub focus: Option<String>,  // Remove this
       pub max_iterations: u32,
       // ... other fields
   }
   
   // After
   pub struct CookCommand {
       pub playbook: PathBuf,
       pub path: Option<PathBuf>,
       pub max_iterations: u32,
       // ... other fields
   }
   ```

2. **Remove from ExecutionEnvironment**
   ```rust
   // Before
   pub struct ExecutionEnvironment {
       pub working_dir: PathBuf,
       pub project_dir: PathBuf,
       pub worktree_name: Option<String>,
       pub session_id: String,
       pub focus: Option<String>,  // Remove this
   }
   
   // After
   pub struct ExecutionEnvironment {
       pub working_dir: PathBuf,
       pub project_dir: PathBuf,
       pub worktree_name: Option<String>,
       pub session_id: String,
   }
   ```

3. **Update Workflow Executor**
   - Remove focus parameter from execute_step method
   - Remove MMM_FOCUS environment variable logic
   - Remove focus tracking test code

4. **Update Worktree Creation**
   ```rust
   // Before
   worktree_manager.create_session(config.command.focus.as_deref()).await?
   
   // After
   worktree_manager.create_session(None).await?
   ```

5. **Update Claude Commands**
   - Remove references to MMM_FOCUS from all command documentation
   - Update examples to show --args usage instead

### Migration Strategy

Users currently using focus can achieve the same results with args:

```yaml
# Before (with focus)
mmm cook workflow.yml --focus "performance"

# After (with args)
commands:
  - name: mmm-code-review
    args: ["--focus", "performance"]
```

Or with playbook variables:
```yaml
# workflow.yml
variables:
  focus: "${ARG}"

commands:
  - name: mmm-code-review
    args: ["--focus", "${focus}"]
```

Then run:
```bash
mmm cook workflow.yml --args "performance"
```

### Code Changes Required

1. **src/main.rs**
   - Remove focus field from CLI argument parsing
   - Remove focus from CookCommand construction

2. **src/cook/command.rs**
   - Remove focus field from CookCommand struct

3. **src/cook/mod.rs**
   - Remove focus from CookConfig construction

4. **src/cook/orchestrator.rs**
   - Remove focus from ExecutionEnvironment
   - Update setup_environment to not pass focus
   - Remove focus display from workflow execution

5. **src/cook/workflow/executor.rs**
   - Remove is_first_step parameter from execute_step
   - Remove MMM_FOCUS environment variable logic
   - Remove focus tracking test code

6. **src/worktree/manager.rs**
   - Update create_session to not accept focus parameter
   - Remove focus from WorktreeSession

7. **All test files**
   - Update tests to not use focus parameter
   - Remove focus-specific test cases

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - CLI interface
  - Cook module
  - Worktree management
  - Claude command documentation
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Verify CookCommand construction without focus
  - Test workflow execution without focus
  - Ensure environment variables don't include MMM_FOCUS
- **Integration Tests**: 
  - Test existing workflows continue to work
  - Test new args-based approach
  - Verify worktree creation without focus
- **Backward Compatibility Tests**: 
  - Ensure old command lines with --focus are handled gracefully

## Documentation Requirements

- **Migration Guide**: 
  - Explain why focus was removed
  - Show before/after examples
  - Provide playbook patterns for common use cases
- **Command Documentation**: 
  - Update all Claude command docs to remove MMM_FOCUS references
  - Add examples using --args approach
- **README Updates**: 
  - Remove focus from CLI examples
  - Add section on passing directives to commands

## Implementation Notes

1. **Graceful Deprecation**
   - Consider keeping --focus flag but making it a no-op with deprecation warning
   - This avoids breaking existing scripts immediately

2. **Alternative Patterns**
   - Document how to use playbook variables effectively
   - Show examples of conditional command execution based on args

3. **Simplification Benefits**
   - Clearer command execution model
   - More flexibility for users
   - Easier to understand and debug
   - Removes hidden state

## Migration and Compatibility

1. **Phase 1: Deprecation**
   - Keep --focus flag but issue deprecation warning
   - Update documentation to prefer --args approach
   - Update all examples

2. **Phase 2: Removal**
   - Remove focus implementation entirely
   - Keep deprecation warning for unrecognized flag

3. **User Communication**
   - Add deprecation notice in next release notes
   - Provide clear migration examples
   - Update all documentation