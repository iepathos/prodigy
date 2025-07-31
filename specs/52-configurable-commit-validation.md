# Specification 52: Configurable Commit Validation for Workflow Commands

**Category**: compatibility
**Priority**: high
**Status**: draft
**Dependencies**: [51 (Validate Git Commits)]

## Context

Specification 51 introduced git commit validation to ensure workflow commands actually make changes before continuing. However, this validation is too rigid for all use cases. Some commands legitimately may not create commits:

- Linting commands that find no issues to fix
- Analysis commands that only generate reports
- Review commands that only create specification files
- Commands that succeed when no work is needed

Currently, when a command like `/mmm-lint` finds no linting issues to fix, the workflow stops with an error even though this is a valid successful outcome. Users need the ability to configure which commands require commit validation and which can proceed without commits.

## Objective

Extend the workflow configuration format to allow per-command specification of commit validation behavior. This enables workflows to distinguish between commands that must produce commits and those that may legitimately complete without changes.

## Requirements

### Functional Requirements
- Workflow YAML files can specify commit validation behavior per command
- Commands can be marked as `commit_required: false` to skip validation
- Default behavior remains commit validation enabled (backward compatible)
- Validation setting is passed through to the workflow executor
- Clear documentation of the new configuration option
- Support both explicit true/false values and absence of the field

### Non-Functional Requirements
- No performance impact beyond existing validation
- Maintains backward compatibility with existing workflows
- Clear error messages when validation fails
- Simple and intuitive configuration syntax

## Acceptance Criteria

- [ ] Workflow YAML schema supports `commit_required` field
- [ ] Commands with `commit_required: false` skip git validation
- [ ] Commands without the field default to validation enabled
- [ ] Workflow executor respects the per-command setting
- [ ] Example workflows demonstrate the configuration
- [ ] Documentation clearly explains when to use this setting
- [ ] Existing workflows continue to function unchanged
- [ ] Error messages reference the configuration option when appropriate

## Technical Details

### Implementation Approach

1. Extend the `WorkflowCommand` struct to include validation setting:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCommand {
    pub name: String,
    pub id: Option<String>,
    #[serde(default = "default_true")]
    pub commit_required: bool,
    pub inputs: Option<HashMap<String, WorkflowInput>>,
    pub outputs: Option<HashMap<String, WorkflowOutput>>,
}

fn default_true() -> bool {
    true
}
```

2. Update workflow executor to check the setting:
```rust
// In execute_structured_command
if command.commit_required && !changes_made {
    return Err(anyhow!("No commits created by command {}", command.name));
}
```

3. Example workflow configuration:
```yaml
# Test coverage improvement workflow
commands:
    - name: mmm-coverage
      id: coverage
      outputs:
        spec:
          file_pattern: "specs/temp/*-coverage-improvements.md"
    
    - name: mmm-implement-spec
      inputs:
        spec:
          from: "${coverage.spec}"
    
    - name: mmm-lint
      commit_required: false  # OK if no linting issues found
```

### Configuration Examples

#### Commands that should skip validation:
```yaml
# Analysis command - only generates reports
- name: mmm-analyze
  commit_required: false

# Linting - OK if code is already clean  
- name: mmm-lint
  commit_required: false

# Code review - creates specs, not commits
- name: mmm-code-review  
  commit_required: false
```

#### Commands that require validation (default):
```yaml
# Implementation commands must create commits
- name: mmm-implement-spec
  # commit_required: true (default)

# Bug fixes must create commits
- name: mmm-fix-bugs
  # commit_required: true (default)

# Cleanup commands must make changes
- name: mmm-cleanup-tech-debt
  # commit_required: true (default)
```

### Error Message Updates

When validation fails and could be configured:
```
❌ Workflow stopped: No changes were committed by /mmm-lint

The command executed successfully but did not create any git commits.
This may be expected if there were no linting issues to fix.

To allow this command to proceed without commits, add to your workflow:
  - name: mmm-lint
    commit_required: false

Alternatively, run with MMM_NO_COMMIT_VALIDATION=true to skip all validation.
```

## Dependencies

- **Prerequisites**: 
  - Spec 51: Validate Git Commits (provides base validation functionality)
- **Affected Components**: 
  - `workflow.rs` - Parse and respect new configuration
  - `structured_commands.rs` - Update command struct
  - Workflow YAML parser - Handle new field
- **External Dependencies**: None beyond existing

## Testing Strategy

- **Unit Tests**: 
  - Parse workflows with and without `commit_required`
  - Verify default value behavior
  - Test validation skip when set to false
- **Integration Tests**: 
  - Run workflows with mixed validation settings
  - Verify commands behave according to configuration
  - Test backward compatibility with old workflows
- **User Acceptance**: 
  - Test with real-world workflows like coverage.yml
  - Verify linting workflows don't fail unnecessarily
  - Ensure clear error messages guide users

## Documentation Requirements

- **Code Documentation**: 
  - Document the new field in WorkflowCommand
  - Explain default behavior in comments
  - Add examples in code
- **User Documentation**: 
  - Update workflow configuration guide
  - Add section on commit validation settings
  - Provide decision guide for when to disable validation
- **Architecture Updates**: 
  - Update workflow schema documentation
  - Add examples to cookbook

## Implementation Notes

- Consider allowing validation configuration at workflow level as well as command level
- Future enhancement: distinguish between "no commits" and "no changes at all"
- Could extend to support validation modes: "commits", "changes", "none"
- Consider warning when disabling validation on typically change-producing commands

## Migration and Compatibility

- Existing workflows continue to work with validation enabled
- No migration needed - opt-in configuration
- Can gradually update workflows to specify validation needs
- Global override (MMM_NO_COMMIT_VALIDATION) remains available