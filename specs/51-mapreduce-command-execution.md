---
number: 51
title: MapReduce Command Execution Integration
category: parallel
priority: critical
status: draft
dependencies: [49, 50]
created: 2025-08-18
---

# Specification 51: MapReduce Command Execution Integration

**Category**: parallel
**Priority**: critical
**Status**: draft
**Dependencies**: [49 - MapReduce Parallel Execution, 50 - Variable Interpolation Engine]

## Context

The MapReduce executor currently has placeholder code in the `execute_agent_commands` function that doesn't actually execute workflow commands. Lines 481-485 in `mapreduce.rs` show a loop that just collects command information rather than executing real Claude or shell commands. This prevents the MapReduce system from performing actual work.

The existing WorkflowExecutor in the codebase knows how to execute individual workflow steps, but it needs to be integrated with the MapReduce executor in a way that maintains agent isolation, handles parallel execution safely, and properly manages the execution context for each agent.

## Objective

Complete the integration between MapReduceExecutor and the existing command execution infrastructure to enable actual command execution for each parallel agent, including:
1. Executing Claude commands via ClaudeExecutor
2. Running shell commands in the agent's worktree
3. Handling on_failure recovery flows
4. Capturing and passing command output between steps
5. Managing commits and git operations per agent

## Requirements

### Functional Requirements

1. **Command Execution Integration**
   - Execute WorkflowStep commands using existing handlers
   - Route Claude commands through ClaudeExecutor
   - Execute shell commands in agent's worktree directory
   - Support all existing command types (claude, shell, file, git, cargo)

2. **Context Management**
   - Maintain per-agent execution context
   - Pass command output to subsequent steps
   - Update context with shell output for `${shell.output}` interpolation
   - Preserve git state per worktree

3. **Error Handling & Recovery**
   - Implement on_failure handlers from workflow YAML
   - Support max_attempts for retry logic
   - Handle fail_workflow flag to continue/stop on errors
   - Capture error details for reporting

4. **Output Management**
   - Capture stdout/stderr from shell commands
   - Store Claude command responses
   - Aggregate outputs for reduce phase access
   - Support capture_output flag for selective storage

5. **Commit Tracking**
   - Track git commits per agent worktree
   - Support commit_required validation
   - Aggregate commit SHAs for reduce phase
   - Handle merge preparation for reduce

### Non-Functional Requirements

1. **Isolation**
   - Commands in one agent must not affect others
   - File system changes isolated to worktree
   - Environment variables scoped per agent

2. **Performance**
   - Minimal overhead for command routing
   - Efficient output buffering
   - Parallel execution without bottlenecks

3. **Reliability**
   - Graceful handling of command timeouts
   - Clean recovery from partial failures
   - Atomic operations where possible

## Acceptance Criteria

- [ ] Claude commands execute through ClaudeExecutor with proper context
- [ ] Shell commands run in the correct worktree directory
- [ ] Command output is captured and available as `${shell.output}`
- [ ] on_failure handlers trigger when commands fail
- [ ] Retry logic works according to max_attempts setting
- [ ] fail_workflow=false allows continuation after failures
- [ ] Git commits are tracked per agent worktree
- [ ] All command types (claude, shell, file, git, cargo) work in parallel agents
- [ ] Integration tests verify end-to-end command execution
- [ ] Error messages clearly indicate which agent/command failed
- [ ] Performance tests show <5% overhead vs direct execution

## Technical Details

### Implementation Approach

1. Refactor `execute_agent_commands` to use WorkflowExecutor
2. Create agent-specific execution contexts
3. Implement command result handling and output capture
4. Add on_failure handler execution logic
5. Integrate with existing retry mechanism

### Architecture Changes

```rust
// Enhanced MapReduceExecutor
impl MapReduceExecutor {
    async fn execute_agent_commands(
        &self,
        item_id: &str,
        item: &Value,
        template_steps: &[WorkflowStep],
        env: &ExecutionEnvironment,
    ) -> Result<AgentResult> {
        // Create agent-specific context
        let mut context = self.create_agent_context(item_id, item, env)?;
        let mut outputs = Vec::new();
        let mut commits = Vec::new();
        
        // Execute each step with proper handler
        for step in template_steps {
            let result = self.execute_single_step(step, &mut context).await?;
            
            // Handle failures with on_failure logic
            if let Err(e) = result {
                if let Some(on_failure) = &step.on_failure {
                    self.handle_failure(on_failure, &mut context, e).await?;
                }
            }
            
            outputs.push(result.output);
            context.update_with_output(result.output);
        }
        
        // Collect commits from worktree
        commits = self.collect_worktree_commits(&context.worktree_path).await?;
        
        Ok(AgentResult { item_id, outputs, commits, ... })
    }
    
    async fn execute_single_step(
        &self,
        step: &WorkflowStep,
        context: &mut AgentContext,
    ) -> Result<StepResult> {
        // Interpolate variables
        let interpolated = self.interpolate_step(step, context)?;
        
        // Route to appropriate handler
        match &interpolated.command_type {
            CommandType::Claude => self.execute_claude_command(interpolated, context).await,
            CommandType::Shell => self.execute_shell_command(interpolated, context).await,
            CommandType::File => self.execute_file_command(interpolated, context).await,
            // ... other command types
        }
    }
}

// Agent-specific context
pub struct AgentContext {
    pub item_id: String,
    pub worktree_path: PathBuf,
    pub variables: HashMap<String, String>,
    pub shell_output: Option<String>,
    pub environment: ExecutionEnvironment,
    pub retry_count: u32,
}

// Step execution result
pub struct StepResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration: Duration,
}
```

### Data Flow

```
1. Create AgentContext with worktree info
2. For each WorkflowStep:
   a. Interpolate variables using context
   b. Execute command via appropriate handler
   c. Capture output and update context
   d. Handle failures with on_failure flow
   e. Update retry counter if needed
3. Collect all commits from worktree
4. Return AgentResult with outputs and commits
```

### APIs and Interfaces

```rust
// Command execution traits
#[async_trait]
trait AgentCommandExecutor {
    async fn execute_claude(&self, cmd: &str, context: &AgentContext) -> Result<String>;
    async fn execute_shell(&self, cmd: &str, context: &AgentContext) -> Result<String>;
    async fn execute_file(&self, op: FileOp, context: &AgentContext) -> Result<()>;
}

// Failure handling
struct OnFailureHandler {
    pub command: WorkflowStep,
    pub max_attempts: u32,
    pub fail_workflow: bool,
}

impl OnFailureHandler {
    async fn handle(&self, error: Error, context: &mut AgentContext) -> Result<()>;
}
```

## Dependencies

- **Prerequisites**: 
  - Spec 49 (MapReduce base)
  - Spec 50 (Variable interpolation)
- **Affected Components**:
  - `src/cook/execution/mapreduce.rs` - Main integration point
  - `src/cook/workflow/executor.rs` - Reuse execution logic
  - `src/commands/handlers/*` - All command handlers
  - `src/cook/execution/claude.rs` - Claude execution
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Mock command execution for each type
  - on_failure handler triggering
  - Output capture and context updates
  - Retry logic with max_attempts

- **Integration Tests**:
  - Full MapReduce workflow with real commands
  - Parallel Claude API calls
  - Shell commands in multiple worktrees
  - Git operations and commit tracking
  - Failure recovery scenarios

- **Performance Tests**:
  - Overhead of command routing
  - Parallel execution scaling
  - Output buffering efficiency

## Documentation Requirements

- **Code Documentation**:
  - Command execution flow
  - Context management rules
  - Failure handling logic

- **User Documentation**:
  - Supported command types in MapReduce
  - on_failure handler configuration
  - Debugging parallel execution

## Implementation Notes

### Phase 1: Basic Integration (Day 1-2)
- Wire WorkflowExecutor to MapReduceExecutor
- Implement basic command routing
- Add output capture

### Phase 2: Failure Handling (Day 3)
- Implement on_failure handlers
- Add retry logic with max_attempts
- Handle fail_workflow flag

### Phase 3: Context Management (Day 4)
- Complete context updates between steps
- Add shell output interpolation
- Implement commit tracking

### Key Considerations

1. **Thread Safety**: Ensure ClaudeExecutor is safe for parallel use
2. **Resource Limits**: Prevent resource exhaustion from parallel commands
3. **Timeout Handling**: Respect per-agent timeouts during execution
4. **Logging**: Prefix logs with agent ID for debugging
5. **Cleanup**: Ensure proper cleanup even on failure

## Migration and Compatibility

- **Breaking Changes**: None - completes existing interface
- **Migration Path**: Existing MapReduce workflows will start working
- **Compatibility**: Uses existing command handler infrastructure
- **Rollback**: Can revert to placeholder implementation if needed