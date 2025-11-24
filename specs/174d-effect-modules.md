---
number: 174d
title: Effect Modules
category: foundation
priority: high
status: draft
dependencies: [172, 173, 174a, 174b, 174c]
parent: 174
created: 2025-11-24
---

# Specification 174d: Effect Modules

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Specs 172, 173, 174a, 174b, 174c
**Parent**: Spec 174 (Pure Core Extraction)

## Context

This is the fourth phase of Spec 174. Now that we have pure functions (174a, 174b, 174c), we create Effect wrappers that compose pure logic with I/O operations using Stillwater's Effect pattern.

**Scope**: Create effect modules that wrap pure functions. This enables testability with mock environments.

## Objective

Create Effect-based I/O wrappers for:
- Workflow command execution (Claude, shell, handlers)
- Session management (load, update, save)
- Compose pure transformations with I/O

## Requirements

### Functional Requirements

#### FR1: Workflow Command Effects
- **MUST** create `execute_claude_command_effect` using pure command_builder
- **MUST** create `execute_shell_command_effect` using pure command_builder
- **MUST** create `execute_handler_effect` for handler execution
- **MUST** use pure output_parser for all command results

#### FR2: Session Update Effects
- **MUST** create `update_session_effect` wrapping load→apply_session_update→save
- **MUST** create `batch_update_session_effect` for multiple updates
- **MUST** use pure session update functions from 174c

#### FR3: Effect Composition
- **MUST** use `Effect::from_async_fn` for I/O boundaries
- **MUST** use `Effect::map` for pure transformations
- **MUST** use `Effect::and_then` for sequential composition
- **MUST** preserve error context through chains

#### FR4: Environment Types
- **MUST** define `WorkflowEnv` with ClaudeRunner, ShellRunner, OutputPatterns
- **MUST** define `SessionEnv` with Storage
- **MUST** enable mock implementations for testing

### Non-Functional Requirements

#### NFR1: Composition
- **MUST** compose cleanly with other effects
- **MUST** maintain error context
- **MUST** be reusable across executors

#### NFR2: Testability
- **MUST** enable testing with mock environments
- **MUST** make integration tests straightforward
- **MUST** separate pure logic from I/O

## Acceptance Criteria

- [ ] `src/cook/workflow/effects/` module created
- [ ] `claude.rs` with `execute_claude_command_effect`
- [ ] `shell.rs` with `execute_shell_command_effect`
- [ ] `handler.rs` with `execute_handler_effect`
- [ ] `src/unified_session/effects.rs` created
- [ ] Session effects: `update_session_effect`, `batch_update_session_effect`
- [ ] Environment types defined
- [ ] Integration tests with mock environments
- [ ] All tests pass
- [ ] `cargo fmt` and `cargo clippy` pass

## Technical Details

### Workflow Effects

```rust
// src/cook/workflow/effects/claude.rs

use stillwater::Effect;
use crate::cook::workflow::pure::{command_builder, output_parser};
use std::collections::HashMap;

pub struct WorkflowEnv {
    pub claude_runner: Box<dyn ClaudeRunner>,
    pub shell_runner: Box<dyn ShellRunner>,
    pub output_patterns: Vec<OutputPattern>,
}

/// Effect: Execute Claude command
pub fn execute_claude_command_effect(
    template: &str,
    variables: &HashMap<String, String>,
) -> Effect<CommandOutput, CommandError, WorkflowEnv> {
    let template = template.to_string();
    let variables = variables.clone();

    Effect::from_async_fn(move |env| async move {
        // Pure: Build command
        let command = command_builder::build_command(&template, &variables);

        // I/O: Execute
        let output = env.claude_runner.run(&command).await?;

        // Pure: Parse output
        let new_vars = output_parser::parse_output_variables(
            &output.stdout,
            &env.output_patterns,
        );

        Ok(CommandOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.exit_code,
            variables: new_vars,
        })
    })
}
```

```rust
// src/cook/workflow/effects/shell.rs

pub fn execute_shell_command_effect(
    template: &str,
    variables: &HashMap<String, String>,
) -> Effect<CommandOutput, CommandError, WorkflowEnv> {
    let template = template.to_string();
    let variables = variables.clone();

    Effect::from_async_fn(move |env| async move {
        // Pure: Build command
        let command = command_builder::build_command(&template, &variables);

        // I/O: Execute
        let output = env.shell_runner.run(&command).await?;

        // Pure: Parse output
        let new_vars = output_parser::parse_output_variables(
            &output.stdout,
            &env.output_patterns,
        );

        Ok(CommandOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.exit_code,
            variables: new_vars,
        })
    })
}
```

### Session Effects

```rust
// src/unified_session/effects.rs

use stillwater::Effect;
use crate::core::session::updates::{apply_session_update, SessionUpdate};
use crate::unified_session::{UnifiedSession, SessionError, SessionId};

pub struct SessionEnv {
    pub storage: Box<dyn SessionStorage>,
}

/// Effect: Update session with I/O
pub fn update_session_effect(
    id: SessionId,
    update: SessionUpdate,
) -> Effect<UnifiedSession, SessionError, SessionEnv> {
    Effect::from_async_fn(move |env| async move {
        // I/O: Load session
        let session = env.storage.load_session(&id).await?;

        // Pure: Apply update
        let updated = apply_session_update(session, update)?;

        // I/O: Save session
        env.storage.save_session(&updated).await?;

        Ok(updated)
    })
}

/// Effect: Batch update session (multiple updates)
pub fn batch_update_session_effect(
    id: SessionId,
    updates: Vec<SessionUpdate>,
) -> Effect<UnifiedSession, SessionError, SessionEnv> {
    Effect::from_async_fn(move |env| async move {
        // I/O: Load once
        let mut session = env.storage.load_session(&id).await?;

        // Pure: Apply all updates
        for update in updates {
            session = apply_session_update(session, update)?;
        }

        // I/O: Save once
        env.storage.save_session(&session).await?;

        Ok(session)
    })
}
```

### Testing with Mocks

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockClaudeRunner {
        responses: HashMap<String, String>,
    }

    impl ClaudeRunner for MockClaudeRunner {
        async fn run(&self, cmd: &str) -> Result<Output, CommandError> {
            let stdout = self.responses.get(cmd).cloned()
                .unwrap_or_else(|| "default".into());
            Ok(Output { stdout, stderr: String::new(), exit_code: 0 })
        }
    }

    #[tokio::test]
    async fn test_claude_effect_with_mock() {
        let mut responses = HashMap::new();
        responses.insert("/test".into(), "success".into());

        let env = WorkflowEnv {
            claude_runner: Box::new(MockClaudeRunner { responses }),
            shell_runner: Box::new(MockShellRunner::default()),
            output_patterns: vec![],
        };

        let effect = execute_claude_command_effect("/test", &HashMap::new());
        let result = effect.run_async(&env).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().stdout, "success");
    }
}
```

## Testing Strategy

- Integration tests with mock environments
- Verify pure function composition
- Test error propagation
- Test effect chaining

## Implementation Notes

### Critical Success Factors
1. **Clean separation** - Pure logic isolated from I/O
2. **Composability** - Effects chain cleanly
3. **Testability** - Mock environments work well
4. **Error context** - Preserve debugging info

### Migration Path
1. Create effect module structure
2. Define environment types
3. Implement Claude command effect
4. Implement shell command effect
5. Implement handler effect
6. Implement session effects
7. Create mock implementations
8. Write integration tests
9. Commit and close spec

## Dependencies

### Prerequisites
- **174a** - Execution planning (for patterns)
- **174b** - Pure transformations (used by workflow effects)
- **174c** - Pure session updates (used by session effects)

### Blocks
- **174e** - Orchestrator refactor (uses these effects)
- **174f** - Workflow executor refactor (uses these effects)

## Success Metrics

- [ ] All 10 acceptance criteria met
- [ ] Integration tests with mocks pass
- [ ] Effects compose cleanly
- [ ] Zero clippy warnings
