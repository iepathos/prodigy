---
number: 174f
title: Refactor Workflow Executor
category: foundation
priority: high
status: draft
dependencies: [174b, 174d]
parent: 174
created: 2025-11-24
---

# Specification 174f: Refactor Workflow Executor

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Specs 174b (Pure Workflow Transformations), 174d (Effect Modules)
**Parent**: Spec 174 (Pure Core Extraction)

## Context

This is the sixth phase of Spec 174. The workflow executor (`src/cook/workflow/executor/commands.rs`, 2,243 LOC) mixes command building, variable expansion, and execution. Now that we have pure transformations (174b) and effects (174d), we refactor the executor.

**Goal**: Reduce workflow executor from 2,243 LOC to ~300 LOC by delegating to pure transformations and effects.

## Objective

Refactor workflow executor to:
- Use pure command building from 174b
- Use pure output parsing from 174b
- Delegate to effect modules for execution (174d)
- Focus on workflow orchestration only

## Requirements

### Functional Requirements

#### FR1: Use Pure Transformations
- **MUST** use `build_command()` for all command construction
- **MUST** use `expand_variables()` for variable substitution
- **MUST** use `parse_output_variables()` for output extraction
- **MUST** eliminate inlined transformation logic

#### FR2: Delegate to Effects
- **MUST** use `execute_claude_command_effect` for Claude commands
- **MUST** use `execute_shell_command_effect` for shell commands
- **MUST** use `execute_handler_effect` for handlers
- **MUST** eliminate direct execution code

#### FR3: LOC Reduction
- **MUST** reduce executor from 2,243 LOC to ~300 LOC
- **MUST** move all transformation logic to pure modules
- **MUST** move all execution logic to effect modules

#### FR4: Functionality Preservation
- **MUST** maintain all existing workflow features
- **MUST** pass all existing workflow tests
- **MUST** preserve variable handling behavior

### Non-Functional Requirements

#### NFR1: Code Quality
- **MUST** reduce from 2,243 LOC to ~300 LOC
- **MUST** improve readability through separation
- **MUST** pass clippy with no warnings

#### NFR2: Performance
- **MUST** show no performance regression
- **MUST** maintain existing benchmarks

## Acceptance Criteria

- [ ] Workflow executor reduced to ~300 LOC
- [ ] Uses pure functions from 174b for transformations
- [ ] Uses effects from 174d for execution
- [ ] All transformation logic moved to pure modules
- [ ] All execution logic moved to effect modules
- [ ] All existing tests pass
- [ ] No performance regression
- [ ] `cargo fmt` and `cargo clippy` pass

## Technical Details

### Refactored Executor

```rust
// src/cook/workflow/executor/commands.rs (~300 LOC)

use crate::cook::workflow::pure::{command_builder, output_parser};
use crate::cook::workflow::effects::{
    execute_claude_command_effect,
    execute_shell_command_effect,
    execute_handler_effect
};
use stillwater::Effect;

pub struct WorkflowExecutor {
    env: WorkflowEnv,
}

impl WorkflowExecutor {
    /// Execute single command
    pub async fn execute_command(
        &self,
        command: &Command,
        variables: &HashMap<String, String>,
    ) -> Result<CommandOutput> {
        let effect = match &command.command_type {
            CommandType::Claude(template) => {
                execute_claude_command_effect(template, variables)
            }
            CommandType::Shell(template) => {
                execute_shell_command_effect(template, variables)
            }
            CommandType::Handler(name) => {
                execute_handler_effect(name, variables)
            }
        };

        effect.run_async(&self.env).await
            .context("Executing command")
    }

    /// Execute workflow (sequence of commands)
    pub async fn execute_workflow(
        &self,
        workflow: &Workflow,
    ) -> Result<WorkflowResult> {
        let mut variables = workflow.initial_variables.clone();
        let mut outputs = vec![];

        for command in &workflow.commands {
            let output = self.execute_command(command, &variables).await?;

            // Merge new variables
            variables.extend(output.variables);

            outputs.push(output);
        }

        Ok(WorkflowResult { outputs, final_variables: variables })
    }
}
```

## Testing Strategy

- Run full workflow executor test suite
- Test all command types (Claude, shell, handler)
- Test variable expansion and output parsing
- Test error handling
- Benchmark performance

## Implementation Notes

### Migration Path
1. Add calls to pure transformation functions
2. Replace execution with effect calls
3. Gradually remove inlined logic
4. Move logic to appropriate modules
5. Verify LOC reduction
6. Run full test suite
7. Benchmark performance
8. Commit

### Critical Success Factors
1. **LOC reduction** - Must achieve ~300 LOC
2. **Test preservation** - All tests must pass
3. **No regression** - Performance maintained
4. **Clean delegation** - Clear separation of concerns

## Dependencies

### Prerequisites
- **174b** - Pure workflow transformations (required)
- **174d** - Effect modules (required)

### Blocks
- None - can proceed after 174b and 174d

## Success Metrics

- [ ] All 8 acceptance criteria met
- [ ] LOC reduced from 2,243 to ~300
- [ ] All tests pass
- [ ] Zero performance regression
