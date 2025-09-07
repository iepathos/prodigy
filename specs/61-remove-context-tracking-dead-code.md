---
number: 61
title: Remove Context Tracking Dead Code
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-09-07
---

# Specification 61: Remove Context Tracking Dead Code

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently contains dead code related to a context tracking feature that was never fully implemented. This includes:

1. Environment variables (`PRODIGY_CONTEXT_AVAILABLE`, `PRODIGY_CONTEXT_DIR`, `PRODIGY_AUTOMATION`) that are set but serve no purpose
2. References in documentation to non-existent context generation features
3. Claude commands that check for context files that are never created
4. Code paths that set these variables without any corresponding functionality

This dead code creates confusion, maintenance burden, and misleading documentation. The context tracking feature is tangential to Prodigy's core purpose of workflow orchestration and adds unnecessary complexity.

## Objective

Remove all dead code related to the unimplemented context tracking feature, simplifying the codebase and focusing Prodigy on its core workflow orchestration capabilities.

## Requirements

### Functional Requirements
- Remove all references to `PRODIGY_CONTEXT_AVAILABLE` environment variable
- Remove all references to `PRODIGY_CONTEXT_DIR` environment variable  
- Maintain `PRODIGY_AUTOMATION` only if it serves an actual purpose, otherwise remove
- Update all Claude commands to remove context checking logic
- Clean up any context-related test code
- Remove misleading documentation about context features

### Non-Functional Requirements
- Ensure no breaking changes to existing workflow execution
- Maintain backward compatibility for workflows that don't rely on context
- Reduce code complexity and maintenance burden
- Improve code clarity and documentation accuracy

## Acceptance Criteria

- [ ] All references to `PRODIGY_CONTEXT_AVAILABLE` removed from source code
- [ ] All references to `PRODIGY_CONTEXT_DIR` removed from source code
- [ ] Environment variable setting code cleaned up in all execution paths
- [ ] Claude commands updated to remove context checking logic
- [ ] Test code updated to remove context-related assertions
- [ ] Documentation updated to remove false claims about context generation
- [ ] All tests pass after removal of dead code
- [ ] Workflows execute successfully without context variables
- [ ] Code coverage maintained or improved

## Technical Details

### Implementation Approach

1. **Source Code Cleanup**
   - Remove environment variable insertion from:
     - `src/cook/orchestrator.rs` (3 locations)
     - `src/cook/workflow/executor.rs`
     - `src/cook/execution/mapreduce.rs`
     - `src/cook/execution/claude.rs`
   - Clean up test code in:
     - `src/cook/workflow/executor_tests.rs`
     - `src/abstractions/claude.rs` (example code)

2. **Claude Command Updates**
   - Update `.claude/commands/prodigy-code-review.md`
   - Update `.claude/commands/prodigy-cleanup-tech-debt.md`
   - Remove any context loading logic from all other commands
   - Simplify command execution flow

3. **Documentation Cleanup**
   - Already completed in previous commit, but verify:
     - README.md accurately reflects actual functionality
     - CLAUDE.md contains no false context claims
   - Update any remaining documentation references

### Architecture Changes

No significant architectural changes required. This is purely a cleanup operation that simplifies existing code paths.

### Data Structures

No data structure changes required. The environment variable HashMap will simply have fewer entries.

### APIs and Interfaces

The public API remains unchanged. Claude commands will receive fewer environment variables but this doesn't affect their interface.

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - Cook orchestrator
  - Workflow executor
  - MapReduce executor
  - Claude executor abstraction
  - Claude command definitions
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Update existing tests to remove context-related assertions
- **Integration Tests**: Verify workflows execute correctly without context variables
- **Regression Tests**: Ensure all existing workflows continue to function
- **Command Tests**: Verify Claude commands work without context checks

## Documentation Requirements

- **Code Documentation**: Update inline comments to remove context references
- **User Documentation**: Already updated in previous commit
- **Architecture Updates**: Update ARCHITECTURE.md if it contains context references
- **Command Documentation**: Update all Claude command markdown files

## Implementation Notes

### Order of Operations
1. First update tests to remove context assertions
2. Then remove context variable setting from source code
3. Update Claude commands to remove context checks
4. Clean up any remaining references
5. Run full test suite to verify nothing broke

### Potential Gotchas
- Some Claude commands may have conditional logic based on context availability
- Need to verify `PRODIGY_AUTOMATION` is actually used somewhere before removing
- Ensure no external tools or scripts depend on these environment variables

### Future Considerations
If context tracking is needed in the future, it should be:
- Designed from scratch with clear requirements
- Implemented completely before being documented
- Focused on specific, actionable context rather than generic "analysis"
- Optional and not central to core workflow orchestration

## Migration and Compatibility

### Breaking Changes
- Claude commands will no longer receive `PRODIGY_CONTEXT_AVAILABLE` and `PRODIGY_CONTEXT_DIR` environment variables
- Any custom commands that check for these variables will need updating

### Migration Path
1. Update any custom Claude commands to not rely on context variables
2. Remove any workflow steps that attempt to use context data
3. Update any external integrations that expect these variables

### Compatibility Notes
- Existing workflows will continue to function
- The change is backward compatible for standard usage
- Only affects users who attempted to use the non-existent context feature