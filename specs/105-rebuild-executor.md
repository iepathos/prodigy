---
number: 102B
title: Rebuild Clean Executor - Phase 2
category: foundation
priority: critical
status: draft
dependencies: [101, 102A]
created: 2025-09-23
---

# Specification 102B: Rebuild Clean Executor - Phase 2

## Context

After Phase 1 (102A) aggressively extracts functionality into modules, the executor will be a broken mess of spaghetti references. Phase 2 rebuilds it with a clean architecture from scratch, using the extracted modules.

## Objective

Create a new, clean executor architecture that orchestrates the extracted modules with clear separation of concerns and simple, testable code.

## Requirements

### Functional Requirements
- Create new `WorkflowOrchestrator` as main coordinator
- Create `ExecutionContext` to pass state between modules
- Implement clean command pattern for different command types
- Support sequential and MapReduce execution modes
- Handle iteration logic cleanly
- All tests must pass (after rewriting)

### Non-Functional Requirements
- Main orchestrator under 500 lines
- No method over 30 lines
- Clear separation of concerns
- Easily extensible for new command types
- No circular dependencies

## Acceptance Criteria

- [ ] New clean architecture implemented
- [ ] Old executor.rs deleted or reduced to <500 lines
- [ ] All workflows execute correctly
- [ ] Tests rewritten and passing
- [ ] Clear module boundaries
- [ ] Documentation for new architecture

## Technical Details

### New Architecture

```rust
// workflow/orchestrator.rs (NEW - Primary Coordinator)
pub struct WorkflowOrchestrator {
    context: ExecutionContext,
    step_executor: StepExecutor,
    iteration_controller: IterationController,
}

impl WorkflowOrchestrator {
    pub async fn execute(&mut self, workflow: &Workflow) -> Result<ExecutionResult> {
        // Simple, clean orchestration
        validation::validate_workflow(workflow)?;

        match workflow.mode {
            Mode::Sequential => self.execute_sequential(workflow).await,
            Mode::MapReduce => self.execute_mapreduce(workflow).await,
        }
    }

    async fn execute_sequential(&mut self, workflow: &Workflow) -> Result<ExecutionResult> {
        if workflow.iterate {
            self.execute_iterations(workflow).await
        } else {
            self.execute_once(workflow).await
        }
    }

    async fn execute_once(&mut self, workflow: &Workflow) -> Result<ExecutionResult> {
        let mut results = Vec::new();

        for step in &workflow.steps {
            let result = self.step_executor.execute(step, &mut self.context).await?;
            results.push(result);
        }

        Ok(ExecutionResult::from(results))
    }

    async fn execute_iterations(&mut self, workflow: &Workflow) -> Result<ExecutionResult> {
        let mut all_results = Vec::new();

        for iteration in 1..=workflow.max_iterations {
            self.context.set_iteration(iteration);

            let results = self.execute_once(workflow).await?;
            let should_continue = self.iteration_controller.should_continue(&results, &self.context)?;

            all_results.push(results);

            if !should_continue {
                break;
            }
        }

        Ok(ExecutionResult::iterations(all_results))
    }
}
```

```rust
// workflow/execution_context.rs (NEW - Shared State)
pub struct ExecutionContext {
    // All mutable state in one place
    pub environment: HashMap<String, String>,
    pub captured_outputs: HashMap<String, String>,
    pub completed_steps: Vec<CompletedStep>,
    pub current_iteration: u32,
    pub git_tracker: GitChangeTracker,
    pub timing: TimingTracker,
    pub progress: ProgressTracker,
}

impl ExecutionContext {
    pub fn interpolate(&self, template: &str) -> String {
        let interpolator = Interpolator::new(self);
        interpolator.interpolate(template)
    }

    pub fn capture_output(&mut self, name: String, value: String) {
        self.captured_outputs.insert(name, value);
    }

    pub fn should_skip_step(&self, index: usize) -> bool {
        self.completed_steps.iter().any(|s| s.index == index)
    }
}
```

```rust
// workflow/step_executor.rs (NEW - Step Execution)
pub struct StepExecutor {
    command_executors: HashMap<CommandType, Box<dyn CommandExecutor>>,
}

impl StepExecutor {
    pub fn new() -> Self {
        let mut executors = HashMap::new();
        executors.insert(CommandType::Claude, Box::new(ClaudeExecutor::new()));
        executors.insert(CommandType::Shell, Box::new(ShellExecutor::new()));
        executors.insert(CommandType::Test, Box::new(TestExecutor::new()));
        // ... other executors

        Self { command_executors: executors }
    }

    pub async fn execute(&self, step: &Step, context: &mut ExecutionContext) -> Result<StepResult> {
        // Check preconditions
        if !validation::should_execute_step(step, context)? {
            return Ok(StepResult::skipped());
        }

        // Get appropriate executor
        let executor = self.command_executors
            .get(&step.command_type)
            .ok_or_else(|| anyhow!("Unknown command type"))?;

        // Execute with timing
        context.timing.start_step(&step.name);
        let result = executor.execute(step, context).await;
        let duration = context.timing.end_step(&step.name);

        // Handle result
        match result {
            Ok(output) => {
                let step_result = StepResult::success(output, duration);
                context.completed_steps.push(CompletedStep::from(step, &step_result));
                Ok(step_result)
            }
            Err(e) if step.allow_failure => {
                Ok(StepResult::failed_allowed(e.to_string(), duration))
            }
            Err(e) => {
                recovery::handle_failure(step, e, context).await
            }
        }
    }
}
```

```rust
// workflow/command_executors.rs (NEW - Command Pattern)
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn execute(&self, step: &Step, context: &mut ExecutionContext) -> Result<CommandOutput>;
}

pub struct ClaudeExecutor {
    client: ClaudeClient,
}

#[async_trait]
impl CommandExecutor for ClaudeExecutor {
    async fn execute(&self, step: &Step, context: &mut ExecutionContext) -> Result<CommandOutput> {
        let command = context.interpolate(&step.command);
        let result = self.client.execute(&command).await?;

        if let Some(capture_as) = &step.capture_output {
            context.capture_output(capture_as.clone(), result.output.clone());
        }

        Ok(CommandOutput {
            stdout: result.output,
            stderr: result.error.unwrap_or_default(),
            exit_code: if result.success { 0 } else { 1 },
        })
    }
}

pub struct ShellExecutor {
    shell: Shell,
}

#[async_trait]
impl CommandExecutor for ShellExecutor {
    async fn execute(&self, step: &Step, context: &mut ExecutionContext) -> Result<CommandOutput> {
        let command = context.interpolate(&step.command);
        let output = self.shell.execute(&command).await?;

        if let Some(capture_as) = &step.capture_output {
            context.capture_output(capture_as.clone(), output.stdout.clone());
        }

        Ok(output)
    }
}
```

```rust
// workflow/iteration_controller.rs (NEW - Iteration Logic)
pub struct IterationController {
    max_iterations: u32,
    convergence_threshold: usize,
}

impl IterationController {
    pub fn should_continue(
        &self,
        last_result: &ExecutionResult,
        context: &ExecutionContext,
    ) -> Result<bool> {
        // No changes made
        if !last_result.has_changes() {
            return Ok(false);
        }

        // Reached max iterations
        if context.current_iteration >= self.max_iterations {
            return Ok(false);
        }

        // Check for convergence (last N iterations had no changes)
        if self.has_converged(context) {
            return Ok(false);
        }

        Ok(true)
    }

    fn has_converged(&self, context: &ExecutionContext) -> bool {
        // Check recent history for convergence
        false // TODO: Implement
    }
}
```

### Simplified Executor (or delete entirely)

```rust
// workflow/executor.rs (OPTIONAL - Keep as thin wrapper or delete)
pub struct WorkflowExecutor {
    orchestrator: WorkflowOrchestrator,
}

impl WorkflowExecutor {
    pub async fn execute(&mut self, workflow: &ExtendedWorkflowConfig, env: &ExecutionEnvironment) -> Result<()> {
        let normalized = normalize_workflow(workflow);
        self.orchestrator.execute(&normalized).await.map(|_| ())
    }
}

// OR just use WorkflowOrchestrator directly everywhere
```

## Implementation Steps

1. **Create ExecutionContext** - Central state container
2. **Create CommandExecutor trait and implementations** - Clean command pattern
3. **Create StepExecutor** - Orchestrates command execution
4. **Create IterationController** - Manages iteration logic
5. **Create WorkflowOrchestrator** - Main coordinator
6. **Update all call sites** - Use new orchestrator
7. **Delete old executor code** - Remove all legacy code
8. **Rewrite tests** - Test new architecture

## Test Migration

```rust
// BEFORE:
#[test]
async fn test_workflow_execution() {
    let mut executor = WorkflowExecutor::new(...);
    executor.execute(&workflow, &env).await.unwrap();
    assert!(executor.completed_steps.len() > 0);
}

// AFTER:
#[test]
async fn test_workflow_execution() {
    let mut orchestrator = WorkflowOrchestrator::new();
    let result = orchestrator.execute(&workflow).await.unwrap();
    assert!(result.steps_completed() > 0);
}
```

## Benefits

- **Clean Architecture** - Clear separation of concerns
- **Testable** - Each component easily tested in isolation
- **Extensible** - Easy to add new command types
- **Maintainable** - Small, focused modules
- **Performant** - No compatibility overhead

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Missing functionality | Use compiler and test failures to find gaps |
| Different behavior | Comprehensive testing, careful review |
| Integration issues | Test end-to-end workflows thoroughly |

## Success Metrics

- No file over 500 lines
- No function over 30 lines
- All tests passing
- Clean module boundaries
- Easy to understand and modify

## Documentation Requirements

- Architecture diagram of new structure
- Migration guide from old to new
- Examples of extending with new command types