---
number: 101
title: Remove Deprecated Functionality
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-21
---

# Specification 101: Remove Deprecated Functionality

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The Prodigy codebase contains several deprecated commands, aliases, and parameters that are still functional but marked for removal. These create confusion for users, increase maintenance burden, and complicate the codebase unnecessarily. The evaluation identified multiple deprecated elements that should be removed to streamline the tool.

## Objective

Remove all deprecated commands, aliases, and parameters from the codebase while maintaining backward compatibility through clear migration messages where absolutely necessary.

## Requirements

### Functional Requirements
- Remove the deprecated `cook` command entirely
- Remove the `improve` alias and associated checking code
- Remove the `dlq reprocess` subcommand (replaced by `dlq retry`)
- Remove deprecated MapReduce parameters from YAML parsing
- Update all documentation to reflect removed functionality
- Provide clear error messages when deprecated commands are attempted

### Non-Functional Requirements
- Ensure no breaking changes for active, non-deprecated functionality
- Maintain clear upgrade path for users of deprecated features
- Keep git history clean with atomic commits for each removal

## Acceptance Criteria

- [ ] `prodigy cook` command returns "Unknown command" error with migration hint
- [ ] `prodigy improve` alias no longer recognized
- [ ] `prodigy dlq reprocess` returns error directing to `dlq retry`
- [ ] YAML files with deprecated MapReduce parameters fail validation with clear messages
- [ ] All references to deprecated functionality removed from documentation
- [ ] No test failures after removing deprecated code
- [ ] Migration guide created for users upgrading from older versions

## Technical Details

### Implementation Approach

1. **Phase 1: Command Removal**
   - Remove `Cook` variant from `Commands` enum in `src/main.rs`
   - Remove `check_deprecated_alias()` function and all calls
   - Remove warning messages for deprecated commands
   - Update command matching logic

2. **Phase 2: Subcommand Cleanup**
   - Remove `Reprocess` variant from `DlqCommands` enum
   - Update DLQ command handling to remove reprocess logic
   - Clean up associated test files

3. **Phase 3: YAML Parameter Cleanup**
   - Remove deprecated parameter checking from `src/cli/yaml_validator.rs`
   - Remove migration logic from `src/cli/yaml_migrator.rs`
   - Update MapReduce configuration structs to reject deprecated fields
   - Add explicit rejection with helpful error messages

4. **Phase 4: Test Updates**
   - Remove tests for deprecated functionality
   - Update integration tests to not use deprecated commands
   - Ensure all example workflows use current syntax

### Architecture Changes

No architectural changes required - this is purely removal of unused code paths.

### Data Structures

Remove deprecated fields from:
- `MapReduceWorkflowConfig` struct
- `MapPhaseYaml` struct
- `WorkflowStep` on_failure handling

### APIs and Interfaces

CLI interface simplified by removing:
- `cook` command
- `improve` alias
- `dlq reprocess` subcommand

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - CLI command parser
  - YAML validator and migrator
  - DLQ command handlers
  - Integration tests
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Remove deprecated command tests
- **Integration Tests**: Verify deprecated commands return appropriate errors
- **Regression Tests**: Ensure non-deprecated functionality unchanged
- **User Acceptance**: Test migration messages are clear and helpful

## Documentation Requirements

- **Code Documentation**: Remove all references to deprecated features
- **User Documentation**: Update CLI help text and man pages
- **Migration Guide**: Create MIGRATION.md with upgrade instructions

## Implementation Notes

- Each removal should be a separate commit for clean history
- Keep error messages helpful for users attempting deprecated commands
- Consider adding telemetry to track deprecated command usage before removal
- Ensure all example workflows in documentation are updated

## Migration and Compatibility

Users attempting deprecated commands will receive:
```
Error: Command 'cook' has been removed in v0.2.0
Please use 'prodigy run' instead.
For more information, see MIGRATION.md
```

YAML files with deprecated parameters will fail with:
```
Error: Deprecated parameter 'timeout_per_agent' is no longer supported.
Please remove this parameter from your workflow file.
See MIGRATION.md for updated syntax.
```