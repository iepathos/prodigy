---
number: 123
title: Configurable Claude Permissions
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-10-08
---

# Specification 123: Configurable Claude Permissions

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently always passes `--dangerously-skip-permissions` when executing Claude commands in workflows. This flag bypasses Claude's permission system entirely, giving Claude unrestricted access to file operations and system commands.

While this is convenient for automated workflows where human oversight is limited, some users want more control over what permissions Claude has during workflow execution. This is particularly important for:

1. **Security-conscious workflows** - Users working with sensitive codebases who want to restrict Claude's file access
2. **Organizational policies** - Teams with governance requirements around AI tool permissions
3. **Debugging and testing** - Users who want to see exactly what permissions Claude is requesting
4. **Gradual permission grants** - Users who prefer to grant permissions incrementally as needed

Currently, the `--dangerously-skip-permissions` flag is hardcoded in multiple locations:
- `src/cook/execution/claude.rs:170` - Print mode execution
- `src/cook/execution/claude.rs:242` - Streaming mode execution
- `src/abstractions/claude.rs:544` - Code review command
- `src/abstractions/claude.rs:572` - Implement spec command
- `src/abstractions/claude.rs:606` - Lint command

## Objective

Add a configurable option to control whether Prodigy passes `--dangerously-skip-permissions` to Claude commands, giving users the choice between convenience (skip permissions) and control (interactive permissions).

## Requirements

### Functional Requirements

1. **CLI Flag**: Add a `--skip-permissions` flag to `prodigy run` command
   - When present: Pass `--dangerously-skip-permissions` to Claude (current behavior)
   - When absent: Do NOT pass `--dangerously-skip-permissions`, allowing Claude's interactive permission system to work

2. **Workflow Configuration**: Add an optional `skip_permissions` field to workflow YAML files
   ```yaml
   name: my-workflow
   mode: mapreduce
   skip_permissions: false  # Optional: defaults to true for backward compatibility

   map:
     agent_template:
       - claude: "/process-item"
   ```

3. **Precedence**: CLI flag should override workflow configuration
   - If CLI flag is present, use its value
   - Otherwise, use workflow configuration value
   - Default to `true` (skip permissions) for backward compatibility

4. **Environment Variable**: Support `PRODIGY_SKIP_PERMISSIONS` environment variable
   - Precedence: CLI flag > workflow config > environment variable > default (true)

### Non-Functional Requirements

1. **Backward Compatibility**: Existing workflows must continue to work without modification
2. **Consistency**: Behavior must be consistent across all Claude execution modes (print, streaming)
3. **Abstraction Layer**: Permission configuration must work through the ClaudeExecutor abstraction
4. **Clear Documentation**: Users must understand the security implications of each choice

## Acceptance Criteria

- [ ] CLI flag `--skip-permissions` is added to `prodigy run` command
- [ ] Workflow YAML supports optional `skip_permissions: true/false` field
- [ ] Environment variable `PRODIGY_SKIP_PERMISSIONS=true/false` is supported
- [ ] Precedence order is correctly implemented: CLI > workflow > env var > default
- [ ] Default behavior (skip permissions) is preserved for backward compatibility
- [ ] Both print mode and streaming mode respect the configuration
- [ ] `ClaudeExecutor` trait methods accept permission configuration parameter
- [ ] All existing tests pass without modification
- [ ] New integration tests verify permission flag behavior
- [ ] Documentation updated to explain security implications

## Technical Details

### Implementation Approach

1. **Add Permission Configuration to Context**
   ```rust
   pub struct ExecutionContext {
       // ... existing fields ...
       pub skip_permissions: bool, // Default: true
   }
   ```

2. **Update ClaudeExecutor Trait**
   ```rust
   #[async_trait]
   pub trait ClaudeExecutor: Send + Sync {
       async fn execute_claude_command(
           &self,
           command: &str,
           project_path: &Path,
           env_vars: HashMap<String, String>,
           skip_permissions: bool, // New parameter
       ) -> Result<ExecutionResult>;
   }
   ```

3. **Modify Command Execution**
   ```rust
   // In execute_with_print and execute_with_streaming:
   let mut args = vec!["--print".to_string(), command.to_string()];
   if skip_permissions {
       args.insert(1, "--dangerously-skip-permissions".to_string());
   }
   ```

4. **Add Workflow YAML Parsing**
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkflowDefinition {
       // ... existing fields ...
       #[serde(default = "default_skip_permissions")]
       pub skip_permissions: bool,
   }

   fn default_skip_permissions() -> bool {
       true // Backward compatibility
   }
   ```

5. **Update CLI Arguments**
   ```rust
   #[derive(Parser)]
   pub struct RunCommand {
       // ... existing fields ...
       #[arg(long, default_value_t = true)]
       pub skip_permissions: bool,
   }
   ```

### Architecture Changes

**Before**:
```
Workflow → ClaudeExecutor → Always passes --dangerously-skip-permissions
```

**After**:
```
CLI Flag / Workflow Config / Env Var
    ↓
ExecutionContext.skip_permissions
    ↓
ClaudeExecutor → Conditionally passes --dangerously-skip-permissions
```

### Data Structures

No new data structures required. Extensions to existing structures:
- `ExecutionContext` - Add `skip_permissions: bool` field
- `WorkflowDefinition` - Add `skip_permissions: Option<bool>` field
- `RunCommand` - Add `--skip-permissions` CLI flag

### Migration Path

1. Phase 1: Add `skip_permissions` field with default value `true`
2. Phase 2: Update documentation to recommend explicit configuration
3. Phase 3: Consider changing default to `false` in a major version bump

## Dependencies

**Prerequisites**: None - this is a self-contained feature

**Affected Components**:
- `src/cook/execution/claude.rs` - Command execution implementation
- `src/abstractions/claude.rs` - Claude abstraction layer
- `src/workflow/definition.rs` - Workflow YAML parsing
- `src/cli/run.rs` - CLI argument parsing

**External Dependencies**: None

## Testing Strategy

### Unit Tests

1. **ExecutionContext Tests**
   - Verify default value is `true`
   - Test serialization/deserialization

2. **ClaudeExecutor Tests**
   - Verify flag is passed when `skip_permissions = true`
   - Verify flag is NOT passed when `skip_permissions = false`
   - Test both print and streaming modes

3. **Workflow Parsing Tests**
   - Parse workflow with `skip_permissions: true`
   - Parse workflow with `skip_permissions: false`
   - Parse workflow without `skip_permissions` field (should default to `true`)

### Integration Tests

1. **CLI Flag Tests**
   - Run workflow with `--skip-permissions=true`
   - Run workflow with `--skip-permissions=false`
   - Verify CLI flag overrides workflow config

2. **Environment Variable Tests**
   - Set `PRODIGY_SKIP_PERMISSIONS=false`
   - Verify environment variable is respected
   - Verify CLI flag overrides environment variable

3. **Backward Compatibility Tests**
   - Run existing workflows without modification
   - Verify they still skip permissions by default

### User Acceptance

1. Users can run workflows with interactive permissions
2. Users can see what permissions Claude requests
3. Existing workflows continue to work without changes
4. Documentation clearly explains security tradeoffs

## Documentation Requirements

### Code Documentation

1. Document `skip_permissions` field in `ExecutionContext`
2. Update `ClaudeExecutor` trait documentation
3. Add inline comments explaining permission behavior

### User Documentation

1. **CLI Help Text**
   ```
   --skip-permissions <BOOL>
       Skip Claude permission prompts (default: true)

       When true, passes --dangerously-skip-permissions to Claude,
       bypassing interactive permission requests. This is convenient
       for automated workflows but gives Claude unrestricted access.

       When false, Claude will request permissions interactively,
       giving you more control but requiring user interaction.
   ```

2. **Workflow Configuration Guide**
   Add section to CLAUDE.md explaining:
   - How to configure `skip_permissions` in workflow YAML
   - Security implications of each choice
   - Recommended settings for different use cases
   - Examples of both modes

3. **Security Best Practices**
   Document when to use each mode:
   - CI/CD workflows: `skip_permissions: true`
   - Sensitive codebases: `skip_permissions: false`
   - Development/testing: `skip_permissions: false`

### Architecture Updates

Update ARCHITECTURE.md to document:
- Permission configuration flow
- Precedence order (CLI > workflow > env > default)
- Security considerations

## Implementation Notes

### Error Handling

1. **Interactive Mode in Non-Interactive Context**
   - If `skip_permissions = false` but running in CI/CD (no TTY), Claude will fail
   - Detect this condition and provide clear error message:
     ```
     Error: skip_permissions=false requires interactive terminal
     Hint: Set skip_permissions=true for CI/CD environments
     ```

2. **Permission Denial**
   - If user denies a permission Claude requests, the workflow will fail
   - This is expected behavior - document it clearly

### Performance Considerations

No performance impact - this is a simple boolean configuration change.

### Security Considerations

1. **Default Behavior**
   - Default to `skip_permissions: true` for backward compatibility
   - Document that this gives Claude unrestricted access
   - Recommend users explicitly configure based on their security needs

2. **Audit Trail**
   - Log whether permissions were skipped or interactive
   - Include in workflow execution events

3. **Sensitive Operations**
   - Consider warning users when skipping permissions on sensitive operations
   - Example: Warn if workflow has `shell:` commands and `skip_permissions: true`

## Migration and Compatibility

### Backward Compatibility

✅ **Fully backward compatible**
- Default value matches current behavior
- Existing workflows work without modification
- No breaking changes to APIs or file formats

### Migration Steps

For users who want to enable permission controls:

1. **Test in Development**
   ```bash
   prodigy run workflow.yml --skip-permissions=false
   ```

2. **Update Workflow**
   ```yaml
   name: my-workflow
   skip_permissions: false
   ```

3. **Verify CI/CD**
   - Ensure CI/CD workflows use `skip_permissions: true`
   - Or set `PRODIGY_SKIP_PERMISSIONS=true` in CI environment

### Rollback

If issues arise, users can:
1. Remove `skip_permissions` field from workflow → defaults to `true`
2. Pass `--skip-permissions=true` on CLI → overrides workflow
3. Set `PRODIGY_SKIP_PERMISSIONS=true` → system-wide default

## Future Enhancements

Potential follow-up work (not in this spec):

1. **Permission Profiles**
   - Predefined permission sets (read-only, standard, full)
   - Example: `permissions: read-only` → Allow reads, deny writes

2. **Per-Command Permissions**
   ```yaml
   map:
     agent_template:
       - claude: "/analyze-code"
         skip_permissions: false  # Interactive for this command
       - shell: "cargo build"
         skip_permissions: true   # Skip for shell commands
   ```

3. **Permission Audit Logs**
   - Log all permission requests and grants
   - Export to audit trail

4. **Permission Allowlists**
   - Explicitly list allowed file paths
   - Auto-grant for allowed paths, prompt for others

These enhancements can be addressed in future specifications as needed.
