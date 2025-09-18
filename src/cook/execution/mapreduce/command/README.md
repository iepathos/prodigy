# MapReduce Command Execution Module

This module provides the command execution framework for MapReduce workflows in Prodigy. It supports multiple command types (Claude, Shell, etc.) through a pluggable executor architecture.

## Architecture Overview

The module follows a strategy pattern for command execution:

```
WorkflowStep → CommandHandler → CommandExecutor → Result
```

- **WorkflowStep**: The workflow configuration containing command details
- **CommandHandler**: Orchestrates execution, selects appropriate executor
- **CommandExecutor**: Type-specific execution logic (Claude, Shell, etc.)
- **Result**: Standardized command result with output, status, and variables

## Core Components

### CommandExecutor Trait

The `CommandExecutor` trait defines the interface for all command executors:

```rust
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// Execute a command step with the given context
    async fn execute(
        &self,
        step: &WorkflowStep,
        context: &ExecutionContext,
    ) -> Result<CommandResult, CommandError>;

    /// Check if this executor supports the given command type
    fn supports(&self, command_type: &CommandType) -> bool;
}
```

### ExecutionContext

Provides runtime context for command execution:

```rust
pub struct ExecutionContext {
    pub worktree_name: String,      // Git worktree name
    pub worktree_path: PathBuf,     // Worktree filesystem path
    pub item_id: String,             // Current work item ID
    pub work_item: Option<String>,  // Work item data (JSON)
    pub environment: HashMap<String, String>, // Environment variables
}
```

### CommandResult

Standardized result from command execution:

```rust
pub struct CommandResult {
    pub output: Option<String>,     // Command output (stdout)
    pub stderr: Option<String>,     // Error output
    pub exit_code: i32,             // Process exit code
    pub variables: HashMap<String, String>, // Captured variables
    pub duration: Duration,         // Execution time
    pub success: bool,              // Success indicator
}
```

## Existing Command Types

### Claude Commands

Executes Claude AI commands through the Claude CLI:
- Supports all Claude command types
- Adds `PRODIGY_AUTOMATION=true` environment variable
- Handles legacy command format

### Shell Commands

Executes shell commands in the worktree context:
- Runs commands in bash shell
- Supports variable capture from output
- Handles exit codes and stderr

## Adding New Command Types

To add a new command type, follow these steps:

### 1. Define the Command Type

Add your command type to the `CommandType` enum in `types.rs`:

```rust
pub enum CommandType {
    Claude(String),
    Shell(String),
    YourNewType(String),  // Add your type here
}
```

### 2. Update WorkflowStep

Add a field for your command in `WorkflowStep` if needed:

```rust
pub struct WorkflowStep {
    pub claude: Option<String>,
    pub shell: Option<String>,
    pub your_command: Option<String>,  // Add your field
    // ... other fields
}
```

### 3. Implement the Executor

Create a new file for your executor (e.g., `your_executor.rs`):

```rust
use super::executor::{CommandExecutor, CommandResult, CommandError, ExecutionContext};
use async_trait::async_trait;

pub struct YourCommandExecutor {
    // Add any dependencies needed
}

impl YourCommandExecutor {
    pub fn new() -> Self {
        Self {}
    }

    // Add helper methods to keep execute() under 20 lines
    fn validate_step(step: &WorkflowStep) -> Result<String, CommandError> {
        // Extract and validate command from step
    }

    async fn run_command(&self, cmd: String) -> Result<Output, CommandError> {
        // Execute the actual command
    }

    fn build_result(output: Output, start: Instant) -> CommandResult {
        // Convert output to CommandResult
    }
}

#[async_trait]
impl CommandExecutor for YourCommandExecutor {
    async fn execute(
        &self,
        step: &WorkflowStep,
        context: &ExecutionContext,
    ) -> Result<CommandResult, CommandError> {
        let start = Instant::now();
        let command = Self::validate_step(step)?;
        let output = self.run_command(command).await?;
        Ok(Self::build_result(output, start))
    }

    fn supports(&self, command_type: &CommandType) -> bool {
        matches!(command_type, CommandType::YourNewType(_))
    }
}
```

### 4. Register the Executor

Add your executor to the `CommandHandler` in `handler.rs`:

```rust
impl CommandHandler {
    pub fn new() -> Self {
        let mut executors: Vec<Box<dyn CommandExecutor>> = vec![
            Box::new(ClaudeCommandExecutor::new(claude_executor)),
            Box::new(ShellCommandExecutor::new()),
            Box::new(YourCommandExecutor::new()),  // Add here
        ];
        // ...
    }
}
```

### 5. Add Tests

Create tests for your executor in `tests.rs`:

```rust
#[cfg(test)]
mod your_executor_tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_success() {
        // Test successful execution
    }

    #[tokio::test]
    async fn test_execute_failure() {
        // Test error handling
    }

    #[tokio::test]
    async fn test_supports() {
        // Test command type support
    }
}
```

## Best Practices

### 1. Keep Methods Small

Follow the 20-line method limit by extracting logic into helper methods:
- Input validation
- Command preparation
- Result transformation
- Error handling

### 2. Error Handling

Use descriptive error messages with context:

```rust
CommandError::InvalidConfiguration(format!(
    "Missing required field 'command' for {}",
    command_type
))
```

### 3. Async Safety

Ensure your executor is:
- `Send + Sync` for async runtime compatibility
- Thread-safe if storing state
- Properly handling cancellation

### 4. Testing

Write comprehensive tests covering:
- Success cases
- Error scenarios
- Edge cases (empty input, invalid config)
- Context handling

### 5. Documentation

Document your executor with:
- Purpose and use cases
- Configuration requirements
- Example workflow steps
- Error conditions

## Example: Adding a Python Script Executor

Here's a complete example of adding a Python script executor:

```rust
// python_executor.rs
use super::executor::{CommandExecutor, CommandResult, CommandError, ExecutionContext};
use async_trait::async_trait;
use std::process::Command;
use std::time::Instant;

pub struct PythonExecutor {
    python_path: String,
}

impl PythonExecutor {
    pub fn new() -> Self {
        Self {
            python_path: "python3".to_string(),
        }
    }

    fn extract_script(step: &WorkflowStep) -> Result<&str, CommandError> {
        step.python.as_ref().map(|s| s.as_str()).ok_or_else(|| {
            CommandError::InvalidConfiguration("No Python script in step".to_string())
        })
    }

    async fn run_python(&self, script: &str, context: &ExecutionContext)
        -> Result<std::process::Output, CommandError> {
        Command::new(&self.python_path)
            .arg("-c")
            .arg(script)
            .current_dir(&context.worktree_path)
            .output()
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))
    }

    fn output_to_result(output: std::process::Output, start: Instant) -> CommandResult {
        CommandResult {
            output: Some(String::from_utf8_lossy(&output.stdout).to_string()),
            stderr: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            exit_code: output.status.code().unwrap_or(-1),
            variables: HashMap::new(),
            duration: start.elapsed(),
            success: output.status.success(),
        }
    }
}

#[async_trait]
impl CommandExecutor for PythonExecutor {
    async fn execute(
        &self,
        step: &WorkflowStep,
        context: &ExecutionContext,
    ) -> Result<CommandResult, CommandError> {
        let start = Instant::now();
        let script = Self::extract_script(step)?;
        let output = self.run_python(script, context).await?;
        Ok(Self::output_to_result(output, start))
    }

    fn supports(&self, command_type: &CommandType) -> bool {
        matches!(command_type, CommandType::Python(_))
    }
}
```

## Troubleshooting

### Common Issues

1. **Executor not being called**: Ensure it's registered in `CommandHandler` and `supports()` returns true
2. **Async runtime errors**: Verify executor is `Send + Sync`
3. **Method too long**: Extract logic into helper methods
4. **Missing context**: Check `ExecutionContext` is properly populated

### Debug Tips

- Enable trace logging: `RUST_LOG=trace`
- Add debug prints in `supports()` method
- Check executor registration order in `CommandHandler`
- Verify command type parsing in workflow

## Future Enhancements

Planned improvements for the command execution module:

- [ ] Command pipelining (output chaining)
- [ ] Conditional execution based on previous results
- [ ] Command timeout configuration
- [ ] Resource limits (CPU, memory)
- [ ] Command result caching
- [ ] Custom error recovery strategies