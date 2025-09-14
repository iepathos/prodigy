---
number: 68
title: Step-Level On-Failure Handlers
category: foundation
priority: high
status: draft
dependencies: [66]
created: 2025-01-14
---

# Specification 68: Step-Level On-Failure Handlers

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [66 - Variable Capture]

## Context

The whitepaper shows inline failure handlers within workflow steps, particularly in MapReduce operations:
```yaml
map:
  agent_template:
    commands:
      - claude: "/add-type-hints ${item}"
      - shell: "mypy --strict ${item}"
      - on_failure:
          claude: "/fix-type-errors ${item}"
```

This pattern enables immediate recovery from failures without stopping the workflow, allowing for self-healing operations where the system can attempt to fix problems automatically.

## Objective

Implement step-level on-failure handlers that enable immediate recovery actions when commands fail, supporting both simple recovery commands and complex multi-step recovery workflows.

## Requirements

### Functional Requirements
- Support `on_failure:` blocks at step level
- Execute handler only when step fails
- Support single command or multiple commands in handler
- Enable nested on_failure handlers
- Access to failure context in handler (error message, exit code)
- Handler success allows workflow to continue
- Handler failure propagates unless configured otherwise
- Support different handler types (retry, fallback, cleanup)
- Integration with validation failures

### Non-Functional Requirements
- Clear logging of handler execution
- Minimal overhead when no failure occurs
- Consistent error propagation
- Handler timeout configuration

## Acceptance Criteria

- [ ] `on_failure: "command"` executes on step failure
- [ ] `on_failure: ["cmd1", "cmd2"]` executes multiple commands
- [ ] `${error.message}` available in handler commands
- [ ] `${error.exit_code}` available in handler commands
- [ ] Handler success marks step as recovered
- [ ] Handler failure marks step as failed
- [ ] Nested handlers work correctly
- [ ] MapReduce on_failure works per item
- [ ] Clear logs show handler execution
- [ ] Integration with retry logic

## Technical Details

### Implementation Approach

1. **Enhanced Step with On-Failure Handler**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkflowStep {
       #[serde(flatten)]
       pub command: CommandType,

       /// Handler to execute on failure
       #[serde(skip_serializing_if = "Option::is_none")]
       pub on_failure: Option<OnFailureHandler>,

       /// Handler to execute on success (for symmetry)
       #[serde(skip_serializing_if = "Option::is_none")]
       pub on_success: Option<OnSuccessHandler>,

       /// Whether handler failure should be fatal
       #[serde(default)]
       pub handler_failure_fatal: bool,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(untagged)]
   pub enum OnFailureHandler {
       Single(String),
       Multiple(Vec<String>),
       Detailed(FailureHandlerConfig),
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct FailureHandlerConfig {
       /// Commands to execute on failure
       pub commands: Vec<HandlerCommand>,

       /// Handler execution strategy
       #[serde(default)]
       pub strategy: HandlerStrategy,

       /// Maximum handler execution time
       #[serde(with = "duration_serde")]
       pub timeout: Option<Duration>,

       /// Variables to capture from handler
       #[serde(default)]
       pub capture: HashMap<String, String>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum HandlerStrategy {
       /// Try to fix the problem
       Recovery,
       /// Use alternative approach
       Fallback,
       /// Clean up resources
       Cleanup,
       /// Custom handler logic
       Custom,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct HandlerCommand {
       #[serde(flatten)]
       pub command: CommandType,

       /// Continue to next handler command even if this fails
       #[serde(default)]
       pub continue_on_error: bool,
   }
   ```

2. **Failure Handler Executor**:
   ```rust
   pub struct FailureHandlerExecutor {
       command_executor: Arc<CommandExecutor>,
       variable_store: Arc<VariableStore>,
   }

   impl FailureHandlerExecutor {
       pub async fn handle_failure(
           &self,
           handler: &OnFailureHandler,
           failure_context: &FailureContext,
           execution_context: &mut ExecutionContext,
       ) -> Result<HandlerResult> {
           info!("Executing on_failure handler for: {}", failure_context.step_name);

           // Inject failure context as variables
           self.inject_failure_context(failure_context, execution_context).await?;

           // Parse and execute handler
           let commands = self.parse_handler(handler)?;
           let mut results = Vec::new();
           let mut overall_success = true;

           for (idx, cmd) in commands.iter().enumerate() {
               info!("Handler command {}/{}", idx + 1, commands.len());

               let result = self.execute_handler_command(
                   cmd,
                   execution_context
               ).await;

               match result {
                   Ok(res) => {
                       results.push(res);
                   }
                   Err(e) => {
                       error!("Handler command failed: {}", e);
                       overall_success = false;

                       if !cmd.continue_on_error {
                           break;
                       }
                   }
               }
           }

           Ok(HandlerResult {
               success: overall_success,
               strategy: self.determine_strategy(handler),
               command_results: results,
               recovered: overall_success && matches!(
                   self.determine_strategy(handler),
                   HandlerStrategy::Recovery
               ),
           })
       }

       async fn inject_failure_context(
           &self,
           context: &FailureContext,
           exec_context: &mut ExecutionContext,
       ) -> Result<()> {
           // Make error details available as variables
           exec_context.variables.insert(
               "error.message".to_string(),
               context.error_message.clone(),
           );
           exec_context.variables.insert(
               "error.exit_code".to_string(),
               context.exit_code.to_string(),
           );
           exec_context.variables.insert(
               "error.step".to_string(),
               context.step_name.clone(),
           );
           exec_context.variables.insert(
               "error.timestamp".to_string(),
               context.timestamp.to_rfc3339(),
           );

           Ok(())
       }

       async fn execute_handler_command(
           &self,
           cmd: &HandlerCommand,
           context: &mut ExecutionContext,
       ) -> Result<CommandResult> {
           match &cmd.command {
               CommandType::Shell(shell_cmd) => {
                   self.command_executor.execute_shell(shell_cmd, context).await
               }
               CommandType::Claude(claude_cmd) => {
                   self.command_executor.execute_claude(claude_cmd, context).await
               }
               _ => Err(anyhow!("Unsupported handler command type")),
           }
       }
   }
   ```

3. **Integration with Step Execution**:
   ```rust
   impl StepExecutor {
       pub async fn execute_with_handlers(
           &self,
           step: &WorkflowStep,
           context: &mut ExecutionContext,
       ) -> Result<StepResult> {
           // Execute main command
           let command_result = self.execute_command(step, context).await;

           // Handle failure if occurred
           let final_result = match command_result {
               Ok(res) if !res.success => {
                   if let Some(handler) = &step.on_failure {
                       let failure_context = FailureContext {
                           step_name: step.name(),
                           error_message: res.error.unwrap_or_default(),
                           exit_code: res.exit_code,
                           timestamp: Utc::now(),
                       };

                       let handler_result = self.handler_executor
                           .handle_failure(handler, &failure_context, context)
                           .await?;

                       if handler_result.recovered {
                           info!("Step recovered through on_failure handler");
                           StepResult {
                               success: true,
                               recovered: true,
                               handler_executed: true,
                               ..res
                           }
                       } else {
                           res
                       }
                   } else {
                       res
                   }
               }
               Ok(res) => {
                   // Handle success if handler specified
                   if res.success && step.on_success.is_some() {
                       self.handle_success(&step.on_success, context).await?;
                   }
                   res
               }
               Err(e) => return Err(e),
           };

           Ok(final_result)
       }
   }
   ```

### Architecture Changes
- Add `FailureHandlerExecutor` component
- Enhance `StepResult` with recovery information
- Integrate handlers with retry logic
- Add handler metrics collection
- Update execution flow for handler branches

### Data Structures
```yaml
# Example with on_failure handlers
tasks:
  - name: "Build project"
    shell: "npm run build"
    on_failure:
      - shell: "npm cache clean --force"
      - shell: "npm install"
      - shell: "npm run build"

  - name: "Type checking"
    shell: "tsc --noEmit"
    on_failure:
      strategy: recovery
      commands:
        - claude: "/fix-typescript-errors"
        - shell: "tsc --noEmit"

  - name: "Deploy"
    shell: "./deploy.sh production"
    on_failure:
      strategy: fallback
      commands:
        - shell: "./deploy.sh staging"
        - shell: "notify-team 'Production deploy failed, staged to staging'"

# MapReduce with item-level handlers
map:
  agent_template:
    commands:
      - claude: "/optimize ${item}"
      - validate: "test-performance ${item}"
      - on_failure:
          claude: "/rollback-optimization ${item}"
```

## Dependencies

- **Prerequisites**: [66 - Variable Capture] for error context
- **Affected Components**:
  - `src/cook/workflow/on_failure.rs` - Handler logic
  - `src/cook/execution/` - Integration with executors
  - `src/config/workflow.rs` - Handler configuration
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Handler parsing and execution
  - Failure context injection
  - Recovery detection
  - Handler strategy selection
- **Integration Tests**:
  - End-to-end failure handling
  - Multiple handler commands
  - Nested handler execution
  - MapReduce item handlers
- **Scenario Tests**:
  - Build recovery workflows
  - Deployment fallback patterns
  - Self-healing operations
  - Cleanup on failure

## Documentation Requirements

- **Code Documentation**: Document handler execution flow
- **User Documentation**:
  - On-failure handler guide
  - Common recovery patterns
  - Handler strategy selection
  - Best practices for self-healing
- **Architecture Updates**: Add handler flow to execution diagrams

## Implementation Notes

- Handlers should have access to full failure context
- Consider handler composition for complex scenarios
- Support async handlers for long-running recovery
- Enable handler testing in isolation
- Future: ML-based handler selection

## Migration and Compatibility

- Workflows without handlers work unchanged
- No breaking changes to existing workflows
- Gradual adoption of handlers possible
- Clear examples for common failure scenarios