---
number: 88
title: MapReduce Command Execution Module Extraction
category: optimization
priority: high
status: draft
dependencies: [87]
created: 2025-09-17
---

# Specification 88: MapReduce Command Execution Module Extraction

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [87 - Agent Module Extraction]

## Context

The MapReduce executor contains complex command execution logic spread across multiple large methods. The `execute_single_step` method alone is 136 lines and handles multiple command types (Claude, shell, handler) with embedded interpolation logic. This mixed responsibility makes it difficult to test individual command types, add new command types, or modify execution strategies.

## Objective

Extract command execution functionality into a dedicated module that provides a clean abstraction for executing different command types, handling interpolation, and managing execution context. This will enable easier testing, better extensibility for new command types, and clearer separation of concerns.

## Requirements

### Functional Requirements
- Extract command type determination and routing logic
- Separate command interpolation from execution
- Isolate Claude, shell, and handler command execution
- Support command chaining and pipeline execution
- Preserve all current command execution behaviors
- Maintain support for variable capture and substitution

### Non-Functional Requirements
- Each command executor should be under 20 lines
- Command interpolation should be pure functions
- Support async execution without blocking
- Enable easy addition of new command types
- Maintain current performance characteristics

## Acceptance Criteria

- [ ] Command execution module created at `src/cook/execution/mapreduce/command/`
- [ ] Command executor trait defined for extensibility
- [ ] Separate executors for Claude, shell, and handler commands
- [ ] Interpolation logic extracted to pure functions
- [ ] `execute_single_step` method removed from main module
- [ ] Main module reduced by approximately 600 lines
- [ ] All command execution tests pass
- [ ] New unit tests for each command executor
- [ ] Command execution performance unchanged or improved
- [ ] Support for adding new command types documented

## Technical Details

### Implementation Approach

1. **Module Structure**:
   ```
   src/cook/execution/mapreduce/command/
   ├── mod.rs           # Module exports and command router
   ├── executor.rs      # CommandExecutor trait and router
   ├── claude.rs        # Claude command execution
   ├── shell.rs         # Shell command execution
   ├── handler.rs       # Handler command execution
   └── interpolation.rs # Variable interpolation functions
   ```

2. **Key Extractions**:
   - `execute_single_step` (136 lines) → Split across executor modules
   - `determine_command_type` → `executor.rs`
   - `execute_claude_command` → `claude.rs`
   - `execute_shell_command` → `shell.rs`
   - `execute_handler_command` → `handler.rs`
   - `interpolate_workflow_step*` → `interpolation.rs`

### Architecture Changes

- Introduce command executor registry pattern
- Use strategy pattern for command execution
- Implement command pipeline for chaining
- Create immutable command context for execution

### Data Structures

```rust
pub trait CommandExecutor: Send + Sync {
    async fn execute(
        &self,
        step: &WorkflowStep,
        context: &ExecutionContext,
    ) -> Result<CommandResult, CommandError>;

    fn supports(&self, command_type: &CommandType) -> bool;
}

pub struct CommandResult {
    pub output: Option<String>,
    pub exit_code: i32,
    pub variables: HashMap<String, String>,
    pub duration: Duration,
}

pub struct CommandRouter {
    executors: HashMap<CommandType, Box<dyn CommandExecutor>>,
}

pub struct InterpolationEngine {
    // Pure functions for variable substitution
}
```

### APIs and Interfaces

```rust
pub trait CommandInterpolator {
    fn interpolate(&self, template: &str, context: &InterpolationContext) -> String;
    fn extract_variables(&self, template: &str) -> Vec<String>;
}

impl CommandRouter {
    pub fn new() -> Self;
    pub fn register(&mut self, executor: Box<dyn CommandExecutor>);
    pub async fn execute(&self, step: &WorkflowStep, context: &ExecutionContext)
        -> Result<CommandResult, CommandError>;
}
```

## Dependencies

- **Prerequisites**:
  - Phase 1: Utils module extraction (completed)
  - Phase 2: Agent module extraction (spec 87)
- **Affected Components**: MapReduceExecutor, agent execution, workflow steps
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Test each command executor independently
- **Integration Tests**: Verify command routing and chaining
- **Interpolation Tests**: Validate variable substitution
- **Error Tests**: Ensure proper error handling and propagation
- **Performance Tests**: Benchmark command execution overhead

## Documentation Requirements

- **Code Documentation**: Document command executor interface
- **Extension Guide**: How to add new command types
- **Architecture Updates**: Update command execution flow diagrams
- **Migration Guide**: Converting custom command handlers

## Implementation Notes

- Start with CommandExecutor trait definition
- Implement one executor at a time, starting with shell
- Ensure interpolation remains pure and testable
- Use async/await properly without unnecessary boxing
- Consider command caching for repeated executions
- Maintain detailed execution logs for debugging

## Migration and Compatibility

- No changes to workflow file format
- Existing command configurations continue to work
- Internal refactoring maintains API compatibility
- Consider feature flags for gradual rollout
- Provide debug mode for command execution tracing