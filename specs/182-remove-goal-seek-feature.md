---
number: 182
title: Remove Goal-Seek Feature
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-11-25
---

# Specification 182: Remove Goal-Seek Feature

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: none

## Context

The goal-seek feature was designed for iterative refinement operations - attempting a command repeatedly until a validation score meets a threshold. However, this feature:

1. **Is not actively used** - No workflows in production use goal-seek
2. **Doesn't align with current project direction** - Prodigy has evolved toward MapReduce workflows, checkpoint/resume, and DLQ patterns rather than iterative refinement
3. **Adds maintenance burden** - The feature touches CLI, executor, configuration parsing, and workflow execution
4. **Increases complexity** - Extra command type to handle in all execution paths

The goal-seek module includes:
- `src/cook/goal_seek/engine.rs` - Core iteration engine
- `src/cook/goal_seek/validator.rs` - Score extraction and validation
- `src/cook/goal_seek/validators.rs` - Specific validator implementations
- `src/cook/goal_seek/shell_executor.rs` - Shell command execution for validation
- `src/cook/goal_seek/mod.rs` - Module exports and GoalSeekConfig

## Objective

Remove the goal-seek feature entirely from Prodigy to reduce codebase complexity and maintenance burden. This is a breaking change but acceptable since the feature is unused.

## Requirements

### Functional Requirements

1. **Remove CLI subcommand**
   - Remove `goal-seek` (and `seek` alias) from CLI arguments
   - Remove `run_goal_seek` function and `GoalSeekParams` struct
   - Update CLI router to remove the goal-seek path

2. **Remove core module**
   - Delete entire `src/cook/goal_seek/` directory
   - Remove `pub mod goal_seek;` from `src/cook/mod.rs`

3. **Remove from workflow executor**
   - Remove `GoalSeek` variant from `CommandType` enum
   - Remove `goal_seek` field from `WorkflowStep` struct
   - Remove `StepCommand::GoalSeek` variant from normalized commands
   - Remove `execute_goal_seek_command` function
   - Update validation to no longer accept `goal_seek` as valid command type

4. **Remove from configuration**
   - Remove `goal_seek` field from command configuration structs
   - Update any serialization/deserialization handling

5. **Update documentation**
   - Remove goal-seek references from CLAUDE.md
   - Remove `docs/advanced/goal-seeking-operations.md`
   - Remove `book/src/advanced/goal-seeking-operations.md`
   - Update command type documentation in workflow docs
   - Update any examples that reference goal-seek

### Non-Functional Requirements

- All existing tests must pass after removal (except goal-seek specific tests)
- No runtime errors from missing goal-seek handling
- Clean git history with single logical commit

## Acceptance Criteria

- [ ] `src/cook/goal_seek/` directory is deleted
- [ ] CLI no longer accepts `goal-seek` or `seek` subcommand
- [ ] `CommandType` enum has no `GoalSeek` variant
- [ ] `WorkflowStep` has no `goal_seek` field
- [ ] `cargo build` succeeds with no errors
- [ ] `cargo test` passes (excluding removed goal-seek tests)
- [ ] `cargo clippy` shows no new warnings
- [ ] Documentation no longer references goal-seek feature
- [ ] No orphaned imports or dead code related to goal-seek

## Technical Details

### Files to Delete

```
src/cook/goal_seek/
├── engine.rs
├── mod.rs
├── shell_executor.rs
├── validator.rs
└── validators.rs

docs/advanced/goal-seeking-operations.md
book/src/advanced/goal-seeking-operations.md
```

### Files to Modify

**CLI layer:**
- `src/cli/args.rs` - Remove GoalSeek variant from Commands enum
- `src/cli/router.rs` - Remove goal-seek routing
- `src/cli/commands/mod.rs` - Remove goal_seek exports
- `src/cli/commands/goal_seek.rs` - Delete entirely

**Core workflow:**
- `src/cook/mod.rs` - Remove goal_seek module
- `src/cook/workflow/executor.rs` - Remove GoalSeek handling
- `src/cook/workflow/executor/types.rs` - Remove GoalSeek from CommandType
- `src/cook/workflow/executor/commands.rs` - Remove goal_seek execution
- `src/cook/workflow/executor/specialized_commands.rs` - Remove execute_goal_seek_command
- `src/cook/workflow/executor/data_structures.rs` - Remove goal_seek field
- `src/cook/workflow/executor/step_executor.rs` - Remove goal_seek handling
- `src/cook/workflow/executor/builder.rs` - Remove goal_seek field initialization
- `src/cook/workflow/executor/failure_handler.rs` - Remove goal_seek field
- `src/cook/workflow/executor/pure.rs` - Remove goal_seek validation
- `src/cook/workflow/normalized.rs` - Remove GoalSeek variant

**Configuration:**
- `src/config/command.rs` - Remove goal_seek field and imports

**Core types:**
- `src/core/workflow/mod.rs` - Remove GoalSeek from CommandType enum

**Tests:**
- `src/cook/workflow/executor_tests.rs` - Remove goal-seek specific tests

**Documentation:**
- `CLAUDE.md` - Remove goal_seek from command types documentation
- Various drift analysis JSON files in `.prodigy/` may reference goal-seek

### Implementation Approach

1. Start from the leaves (CLI, docs) and work toward core
2. Remove CLI subcommand first (easy to verify)
3. Delete the core module
4. Fix all compilation errors systematically
5. Remove from tests
6. Update documentation
7. Clean up any remaining references

## Dependencies

- **Prerequisites**: None
- **Affected Components**: CLI, workflow executor, configuration parser, documentation
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Remove goal-seek specific tests, ensure remaining tests pass
- **Integration Tests**: Verify workflows without goal-seek still execute correctly
- **Build Verification**: `cargo build`, `cargo test`, `cargo clippy` all pass
- **Documentation**: Manual review that all goal-seek references are removed

## Documentation Requirements

- **Code Documentation**: Remove all goal-seek doc comments
- **User Documentation**: Remove goal-seek sections from docs/ and book/
- **CLAUDE.md**: Update command types list

## Migration and Compatibility

This is a **breaking change**. Any workflows using `goal_seek:` commands will fail to parse after this change. Since the feature is unused in production, no migration path is provided - users would need to rewrite affected workflows using alternative patterns (e.g., shell scripts with loops, or MapReduce with retry logic).

## Implementation Notes

The removal should be straightforward since goal-seek is fairly isolated. The main complexity is ensuring all references are found and removed to avoid dead code or broken imports.

Search for these patterns to ensure complete removal:
- `goal_seek` (snake_case in code and config)
- `GoalSeek` (PascalCase in types)
- `goal-seek` (kebab-case in CLI and docs)
- `seek` (CLI alias)
