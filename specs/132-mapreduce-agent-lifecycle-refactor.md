---
number: 132
title: MapReduce Executor - Agent Lifecycle Refactoring
category: foundation
priority: medium
status: draft
dependencies: [129, 130, 131]
created: 2025-10-11
---

# Specification 132: MapReduce Executor - Agent Lifecycle Refactoring

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: 129 (StepResult fix), 130 (Pure functions), 131 (Phase modules)

## Context

After completing the phase module split (Spec 131), the MapReduce executor still contains complex agent lifecycle management mixed with execution logic. The agent execution spans ~270 lines in `execute_agent_for_item` method with deep nesting (4+ levels) and mixed concerns.

### Current Problems

1. **Mixed responsibilities**: Agent method handles lifecycle, execution, error handling, and state transitions
2. **Deep nesting**: 4+ levels of conditionals make logic hard to follow
3. **Long function**: `execute_agent_for_item` is 267 lines (should be < 20)
4. **State machine hidden**: Agent states (Created → Running → Completed/Failed) not explicit
5. **Not reusable**: Agent logic tightly coupled to MapReduce coordinator

## Objective

Refactor agent lifecycle into a clean state machine with explicit transitions, separating pure state logic from I/O operations. Reduce agent-related code in executor by ~40%.

## Requirements

### Functional Requirements

**Module 1: `agent/lifecycle.rs`** (Pure State Machine)
- Define explicit agent states (Created, Running, Completed, Failed)
- Define state transitions as pure functions
- Validate state transitions
- No I/O, purely functional

**Module 2: `agent/execution.rs`** (I/O Operations)
- Execute agent commands in worktree
- Handle command failures
- Collect agent outputs
- Manage worktree lifecycle

**Module 3: `agent/types.rs`** (Data Structures)
- AgentState enum
- AgentTransition enum
- AgentConfig struct
- AgentResult struct

### Non-Functional Requirements

- Agent lifecycle code under 300 lines total
- State transitions must be pure functions
- Clear separation: state logic vs. I/O
- All existing tests pass unchanged
- No performance regression

## Acceptance Criteria

- [ ] Created `agent/lifecycle.rs` with pure state machine (~100 lines)
- [ ] Created `agent/execution.rs` with I/O operations (~150 lines)
- [ ] Created `agent/types.rs` with data structures (~50 lines)
- [ ] `executor.rs` agent code reduced from ~270 to ~100 lines
- [ ] State transitions are explicit and tested
- [ ] All existing tests pass unchanged
- [ ] State machine visualization documented
- [ ] Performance within 2% of baseline

## Technical Details

### Agent State Machine

**States**:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
    /// Agent created but not started
    Created {
        agent_id: String,
        work_item: Value,
    },
    /// Agent is currently executing
    Running {
        agent_id: String,
        started_at: Instant,
        worktree_path: PathBuf,
    },
    /// Agent completed successfully
    Completed {
        agent_id: String,
        output: Option<String>,
        commits: Vec<String>,
        duration: Duration,
    },
    /// Agent failed with error
    Failed {
        agent_id: String,
        error: String,
        duration: Duration,
        json_log_location: Option<String>,
    },
}
```

**Transitions**:

```rust
#[derive(Debug, Clone)]
pub enum AgentTransition {
    Start { worktree_path: PathBuf },
    Complete { output: Option<String>, commits: Vec<String> },
    Fail { error: String, json_log_location: Option<String> },
}
```

### Module 1: lifecycle.rs (Pure Logic)

```rust
//! Agent lifecycle state machine (pure functions only)

use super::types::{AgentState, AgentTransition, AgentResult};
use std::time::{Duration, Instant};

/// Apply transition to current state
pub fn apply_transition(
    state: AgentState,
    transition: AgentTransition,
) -> Result<AgentState, StateError> {
    match (state, transition) {
        (AgentState::Created { agent_id, .. }, AgentTransition::Start { worktree_path }) => {
            Ok(AgentState::Running {
                agent_id,
                started_at: Instant::now(),
                worktree_path,
            })
        }

        (AgentState::Running { agent_id, started_at, .. }, AgentTransition::Complete { output, commits }) => {
            Ok(AgentState::Completed {
                agent_id,
                output,
                commits,
                duration: started_at.elapsed(),
            })
        }

        (AgentState::Running { agent_id, started_at, .. }, AgentTransition::Fail { error, json_log_location }) => {
            Ok(AgentState::Failed {
                agent_id,
                error,
                duration: started_at.elapsed(),
                json_log_location,
            })
        }

        (state, transition) => {
            Err(StateError::InvalidTransition {
                from: format!("{:?}", state),
                transition: format!("{:?}", transition),
            })
        }
    }
}

/// Convert final agent state to result
pub fn state_to_result(state: &AgentState) -> Option<AgentResult> {
    match state {
        AgentState::Completed { agent_id, output, commits, duration } => {
            Some(AgentResult {
                agent_id: agent_id.clone(),
                success: true,
                output: output.clone(),
                commits: commits.clone(),
                duration: *duration,
                error: None,
                json_log_location: None,
            })
        }

        AgentState::Failed { agent_id, error, duration, json_log_location } => {
            Some(AgentResult {
                agent_id: agent_id.clone(),
                success: false,
                output: None,
                commits: Vec::new(),
                duration: *duration,
                error: Some(error.clone()),
                json_log_location: json_log_location.clone(),
            })
        }

        _ => None,
    }
}

/// Validate state transition is legal
pub fn is_valid_transition(
    state: &AgentState,
    transition: &AgentTransition,
) -> bool {
    matches!(
        (state, transition),
        (AgentState::Created { .. }, AgentTransition::Start { .. })
            | (AgentState::Running { .. }, AgentTransition::Complete { .. })
            | (AgentState::Running { .. }, AgentTransition::Fail { .. })
    )
}

/// Get agent ID from any state
pub fn get_agent_id(state: &AgentState) -> &str {
    match state {
        AgentState::Created { agent_id, .. }
        | AgentState::Running { agent_id, .. }
        | AgentState::Completed { agent_id, .. }
        | AgentState::Failed { agent_id, .. } => agent_id,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("Invalid transition from {from} with {transition}")]
    InvalidTransition { from: String, transition: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_created_to_running() {
        let state = AgentState::Created {
            agent_id: "test-1".to_string(),
            work_item: serde_json::json!({"id": 1}),
        };

        let transition = AgentTransition::Start {
            worktree_path: PathBuf::from("/tmp/worktree"),
        };

        let new_state = apply_transition(state, transition).unwrap();
        assert!(matches!(new_state, AgentState::Running { .. }));
    }

    #[test]
    fn test_running_to_completed() {
        let state = AgentState::Running {
            agent_id: "test-1".to_string(),
            started_at: Instant::now(),
            worktree_path: PathBuf::from("/tmp/worktree"),
        };

        let transition = AgentTransition::Complete {
            output: Some("success".to_string()),
            commits: vec!["abc123".to_string()],
        };

        let new_state = apply_transition(state, transition).unwrap();
        assert!(matches!(new_state, AgentState::Completed { .. }));
    }

    #[test]
    fn test_invalid_transition() {
        let state = AgentState::Completed {
            agent_id: "test-1".to_string(),
            output: Some("done".to_string()),
            commits: vec![],
            duration: Duration::from_secs(10),
        };

        let transition = AgentTransition::Start {
            worktree_path: PathBuf::from("/tmp/worktree"),
        };

        assert!(apply_transition(state, transition).is_err());
    }

    #[test]
    fn test_state_to_result_completed() {
        let state = AgentState::Completed {
            agent_id: "test-1".to_string(),
            output: Some("output".to_string()),
            commits: vec!["abc123".to_string()],
            duration: Duration::from_secs(30),
        };

        let result = state_to_result(&state).unwrap();
        assert!(result.success);
        assert_eq!(result.agent_id, "test-1");
        assert_eq!(result.output, Some("output".to_string()));
    }

    #[test]
    fn test_state_to_result_failed() {
        let state = AgentState::Failed {
            agent_id: "test-1".to_string(),
            error: "command failed".to_string(),
            duration: Duration::from_secs(5),
            json_log_location: Some("/tmp/log.json".to_string()),
        };

        let result = state_to_result(&state).unwrap();
        assert!(!result.success);
        assert_eq!(result.error, Some("command failed".to_string()));
    }
}
```

### Module 2: execution.rs (I/O Operations)

```rust
//! Agent execution with I/O operations

use super::lifecycle::{apply_transition, state_to_result, AgentState, AgentTransition};
use super::types::{AgentConfig, AgentResult};
use crate::cook::execution::mapreduce::resources::WorktreeManager;

/// Execute agent with full lifecycle
pub async fn execute_agent<W, C>(
    config: AgentConfig,
    worktree_manager: &W,
    command_executor: &C,
) -> Result<AgentResult>
where
    W: WorktreeManager,
    C: CommandExecutor,
{
    // Start with Created state
    let mut state = AgentState::Created {
        agent_id: config.agent_id.clone(),
        work_item: config.work_item.clone(),
    };

    // Create worktree
    let worktree_path = worktree_manager
        .create_worktree(&config.agent_id)
        .await?;

    // Transition to Running
    state = apply_transition(
        state,
        AgentTransition::Start {
            worktree_path: worktree_path.clone(),
        },
    )?;

    // Execute commands
    let execution_result = execute_commands(
        &config.commands,
        &worktree_path,
        &config.work_item,
        command_executor,
    )
    .await;

    // Transition based on result
    state = match execution_result {
        Ok((output, commits)) => {
            apply_transition(
                state,
                AgentTransition::Complete { output, commits },
            )?
        }
        Err(e) => {
            apply_transition(
                state,
                AgentTransition::Fail {
                    error: e.to_string(),
                    json_log_location: extract_log_location(&e),
                },
            )?
        }
    };

    // Clean up worktree
    if let Err(e) = worktree_manager.cleanup_worktree(&worktree_path).await {
        warn!("Failed to clean up worktree {}: {}", worktree_path.display(), e);
    }

    // Convert to result
    state_to_result(&state).ok_or_else(|| anyhow!("Invalid final state"))
}

/// Execute agent commands in sequence
async fn execute_commands<C>(
    commands: &[WorkflowStep],
    worktree_path: &Path,
    work_item: &Value,
    command_executor: &C,
) -> Result<(Option<String>, Vec<String>)>
where
    C: CommandExecutor,
{
    let mut output = None;
    let commits_before = get_commits(worktree_path).await?;

    // Build interpolation context
    let context = build_agent_context(work_item);

    // Execute each command
    for step in commands {
        let interpolated = interpolate_step(step, &context)?;
        let result = command_executor.execute_step(&interpolated, worktree_path).await?;

        if !result.success {
            return Err(anyhow!("Command failed: {}", result.stderr));
        }

        output = Some(result.stdout);
    }

    // Get new commits
    let commits_after = get_commits(worktree_path).await?;
    let new_commits = diff_commits(&commits_before, &commits_after);

    Ok((output, new_commits))
}

async fn get_commits(path: &Path) -> Result<Vec<String>> {
    use tokio::process::Command;

    let output = Command::new("git")
        .args(["log", "--format=%H", "-n", "100"])
        .current_dir(path)
        .output()
        .await?;

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect())
}

fn diff_commits(before: &[String], after: &[String]) -> Vec<String> {
    after
        .iter()
        .filter(|c| !before.contains(c))
        .cloned()
        .collect()
}

fn build_agent_context(work_item: &Value) -> HashMap<String, String> {
    // Use pure function from interpolation module
    crate::cook::execution::mapreduce::pure::interpolation::build_item_variables(
        work_item,
        "agent",
    )
}

fn extract_log_location(error: &anyhow::Error) -> Option<String> {
    // Extract log location from error if present
    // This pattern matches "📝 Claude log: /path/to/log.json"
    error
        .to_string()
        .lines()
        .find(|line| line.contains("📝 Claude log:"))
        .and_then(|line| line.split(": ").nth(1))
        .map(|s| s.trim().to_string())
}
```

### Updated Coordinator

```rust
impl MapReduceCoordinator {
    /// Execute single agent (now just coordination)
    async fn execute_agent(&self, config: AgentConfig) -> Result<AgentResult> {
        // Delegate to agent execution module
        agent::execution::execute_agent(
            config,
            &self.worktree_manager,
            &self.command_executor,
        )
        .await
    }

    /// Execute multiple agents in parallel
    async fn execute_agents_parallel(
        &self,
        configs: Vec<AgentConfig>,
        parallelism: usize,
    ) -> Vec<AgentResult> {
        let semaphore = Arc::new(Semaphore::new(parallelism));

        stream::iter(configs)
            .map(|config| {
                let sem = semaphore.clone();
                async move {
                    let _permit = sem.acquire().await.unwrap();
                    self.execute_agent(config).await
                }
            })
            .buffer_unordered(parallelism)
            .collect()
            .await
    }
}
```

## Implementation Steps

1. **Create agent module structure**
2. **Implement types.rs with data structures**
3. **Implement lifecycle.rs with state machine**
   - Add comprehensive unit tests
   - Test all state transitions
   - Test invalid transitions
4. **Implement execution.rs with I/O operations**
   - Add integration tests
   - Test worktree management
   - Test command execution
5. **Update coordinator**
   - Remove agent execution code
   - Delegate to agent modules
   - Verify all tests pass
6. **Benchmark performance**

## Testing Strategy

### Unit Tests (lifecycle.rs)

- Test all valid state transitions
- Test invalid transitions are rejected
- Test state_to_result conversions
- Test get_agent_id for all states
- Property-based tests for state machine invariants

### Integration Tests (execution.rs)

- Test complete agent execution
- Test agent failure handling
- Test worktree cleanup
- Test commit tracking
- Test output collection

### End-to-End Tests

All existing MapReduce tests must pass:
- Full workflow execution
- Multiple agents in parallel
- Agent failure and DLQ
- Checkpoint recovery

## Dependencies

**Prerequisites**:
- Spec 129 (StepResult fix)
- Spec 130 (Pure functions extracted)
- Spec 131 (Phase modules split)

**Affected Components**:
- MapReduce coordinator
- Map phase execution
- Worktree manager
- Event logging

## Implementation Notes

### Why State Machine?

Explicit state machines provide:
1. **Clarity**: States and transitions are obvious
2. **Safety**: Invalid transitions are compile-time errors
3. **Testability**: Each transition can be tested in isolation
4. **Debugging**: Current state is always clear
5. **Extensibility**: New states/transitions easy to add

### Benefits of Separation

Separating lifecycle (pure) from execution (I/O):
1. **Testability**: Lifecycle logic tested without I/O
2. **Reusability**: State machine can be used in different contexts
3. **Reasoning**: Pure state logic is easier to understand
4. **Composition**: State machine can be composed with different executors

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking agent execution | Comprehensive integration testing |
| State machine complexity | Keep states minimal, transitions explicit |
| Performance overhead | Benchmark state transitions (should be negligible) |
| Lost context in errors | Preserve error context through transitions |

## Success Metrics

- Agent-related code in executor reduced from ~270 to ~100 lines (63% reduction)
- State machine module under 100 lines
- Execution module under 150 lines
- Zero test failures
- Performance within 2% of baseline
- State transitions are explicit and tested
- Agent execution logic is reusable

## Documentation Requirements

### Code Documentation

- State machine diagram showing states and transitions
- Each state and transition documented
- Example usage of agent execution
- Error handling patterns

### User Documentation

No user-facing changes - internal refactoring only.

### Architecture Updates

Update ARCHITECTURE.md to document:
- Agent lifecycle state machine
- State transitions
- Agent execution flow
- How to extend agent behavior

## Migration and Compatibility

### Breaking Changes

None - All changes are internal implementation details.

### Rollback Plan

Agent refactoring is a single unit of work. If issues arise, the entire refactoring can be reverted as one commit.

### Deployment

Can be deployed immediately with no user impact. Changes are entirely internal to the MapReduce system.
