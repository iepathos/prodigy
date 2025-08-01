# Specification 57: Subprocess Abstraction Layer

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [56-cook-orchestrator-refactor]

## Context

Many of MMM's core functions depend on subprocess execution (git commands, claude CLI, cargo commands). These direct subprocess calls are scattered throughout the codebase and are difficult to test, leading to low test coverage. Currently:

- Direct `tokio::process::Command` calls are embedded in business logic
- No way to mock subprocess execution for testing
- Error handling is inconsistent across different subprocess calls
- No centralized logging or debugging for subprocess operations
- Difficult to add features like timeout, retry, or output filtering

This specification defines a comprehensive subprocess abstraction layer that enables testing and provides consistent subprocess management.

## Objective

Create a unified subprocess abstraction layer that provides mockable interfaces for all external command execution, enabling comprehensive unit testing and consistent error handling.

## Requirements

### Functional Requirements
- Support all existing subprocess operations (git, claude, cargo, etc.)
- Enable complete mocking for unit tests
- Provide consistent error handling and reporting
- Support timeout and retry mechanisms
- Capture and structure subprocess output
- Support both streaming and buffered output modes

### Non-Functional Requirements
- Zero overhead for production use cases
- Type-safe command building
- Async/await compatible
- Thread-safe for concurrent execution
- Minimal API surface for ease of use

## Acceptance Criteria

- [ ] All direct `Command` usage replaced with abstraction
- [ ] Mock implementation supports all testing scenarios
- [ ] 100% unit test coverage for subprocess abstraction
- [ ] All existing subprocess operations still work
- [ ] Consistent error messages across all subprocess failures
- [ ] Timeout support for all subprocess operations
- [ ] Structured output capture with stdout/stderr separation

## Technical Details

### Implementation Approach

1. **Core Abstraction**
   ```rust
   // Main trait for subprocess execution
   #[async_trait]
   pub trait ProcessRunner: Send + Sync {
       async fn run(&self, command: ProcessCommand) -> Result<ProcessOutput>;
       async fn run_streaming(&self, command: ProcessCommand) -> Result<ProcessStream>;
   }

   // Command representation
   pub struct ProcessCommand {
       pub program: String,
       pub args: Vec<String>,
       pub env: HashMap<String, String>,
       pub working_dir: Option<PathBuf>,
       pub timeout: Option<Duration>,
       pub stdin: Option<String>,
   }

   // Output representation
   pub struct ProcessOutput {
       pub status: ExitStatus,
       pub stdout: String,
       pub stderr: String,
       pub duration: Duration,
   }
   ```

2. **Specialized Runners**
   ```rust
   // Git-specific runner with common operations
   #[async_trait]
   pub trait GitRunner: ProcessRunner {
       async fn status(&self, path: &Path) -> Result<GitStatus>;
       async fn commit(&self, path: &Path, message: &str) -> Result<String>;
       async fn create_worktree(&self, path: &Path, name: &str) -> Result<()>;
       // ... other git operations
   }

   // Claude CLI runner
   #[async_trait]
   pub trait ClaudeRunner: ProcessRunner {
       async fn check_availability(&self) -> Result<bool>;
       async fn run_command(&self, cmd: &str, args: &[String]) -> Result<String>;
   }
   ```

3. **Builder Pattern for Commands**
   ```rust
   pub struct ProcessCommandBuilder {
       command: ProcessCommand,
   }

   impl ProcessCommandBuilder {
       pub fn new(program: &str) -> Self { /* ... */ }
       pub fn arg(mut self, arg: &str) -> Self { /* ... */ }
       pub fn args(mut self, args: &[String]) -> Self { /* ... */ }
       pub fn env(mut self, key: &str, value: &str) -> Self { /* ... */ }
       pub fn current_dir(mut self, dir: &Path) -> Self { /* ... */ }
       pub fn timeout(mut self, timeout: Duration) -> Self { /* ... */ }
       pub fn stdin(mut self, input: String) -> Self { /* ... */ }
       pub fn build(self) -> ProcessCommand { /* ... */ }
   }
   ```

### Architecture Changes

1. **Production Implementation**
   ```rust
   pub struct TokioProcessRunner;

   #[async_trait]
   impl ProcessRunner for TokioProcessRunner {
       async fn run(&self, command: ProcessCommand) -> Result<ProcessOutput> {
           let mut cmd = tokio::process::Command::new(&command.program);
           // Configure and execute
           // Handle timeout with tokio::time::timeout
           // Capture output and timings
       }
   }
   ```

2. **Mock Implementation**
   ```rust
   pub struct MockProcessRunner {
       responses: Arc<Mutex<HashMap<String, ProcessOutput>>>,
       call_history: Arc<Mutex<Vec<ProcessCommand>>>,
   }

   impl MockProcessRunner {
       pub fn new() -> Self { /* ... */ }
       pub fn expect_command(&mut self, program: &str) -> &mut MockCommandConfig { /* ... */ }
       pub fn verify_called(&self, program: &str, times: usize) -> bool { /* ... */ }
   }
   ```

3. **Integration with Existing Code**
   ```rust
   // Before:
   let output = Command::new("git").args(&["status"]).output().await?;

   // After:
   let output = runner.run(
       ProcessCommandBuilder::new("git")
           .arg("status")
           .build()
   ).await?;
   ```

### Data Structures

1. **Exit Status Representation**
   ```rust
   #[derive(Debug, Clone)]
   pub enum ExitStatus {
       Success,
       Error(i32),
       Timeout,
       Signal(i32),
   }
   ```

2. **Streaming Output**
   ```rust
   pub struct ProcessStream {
       stdout: Box<dyn Stream<Item = Result<String>> + Send>,
       stderr: Box<dyn Stream<Item = Result<String>> + Send>,
       status: Box<dyn Future<Output = Result<ExitStatus>> + Send>,
   }
   ```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - All modules using subprocess execution
  - Git operations abstraction
  - Claude client abstraction
  - Metrics collection (cargo commands)
- **External Dependencies**: 
  - tokio::process (for production implementation)
  - async-trait

## Testing Strategy

- **Unit Tests**: 
  - Test mock implementation thoroughly
  - Test command builder with all options
  - Test timeout and error scenarios
  - Test concurrent execution
- **Integration Tests**: 
  - Test real subprocess execution
  - Test git operations with real repos
  - Test error conditions (missing executables)
- **Performance Tests**: 
  - Measure overhead vs direct Command usage
  - Test concurrent subprocess execution
- **User Acceptance**: 
  - All existing operations work unchanged
  - Error messages remain helpful

## Documentation Requirements

- **Code Documentation**: 
  - Document trait contracts clearly
  - Provide examples for common use cases
  - Document mock usage patterns
- **Testing Guide**: 
  - How to mock subprocess calls
  - Common test scenarios
  - Best practices for expectations
- **Migration Guide**: 
  - How to replace existing Command usage
  - Common patterns and idioms

## Implementation Notes

1. **Structured Errors**
   ```rust
   #[derive(Debug, thiserror::Error)]
   pub enum ProcessError {
       #[error("Command not found: {0}")]
       CommandNotFound(String),
       #[error("Process timed out after {0:?}")]
       Timeout(Duration),
       #[error("Process exited with code {0}")]
       ExitCode(i32),
       #[error("Process terminated by signal {0}")]
       Signal(i32),
       #[error("IO error: {0}")]
       Io(#[from] std::io::Error),
   }
   ```

2. **Logging Integration**
   - Log all subprocess executions at debug level
   - Log failures at error level
   - Include timing information
   - Optionally log full output

3. **Security Considerations**
   - Validate command arguments
   - Prevent shell injection
   - Sanitize environment variables
   - Document security best practices

## Migration and Compatibility

1. **Phased Migration**
   - Phase 1: Implement abstraction layer
   - Phase 2: Migrate git operations
   - Phase 3: Migrate claude operations
   - Phase 4: Migrate remaining subprocesses

2. **Compatibility Layer**
   - Provide convenience functions for common operations
   - Support both old and new patterns temporarily
   - Deprecate direct Command usage

3. **Testing During Migration**
   - Maintain all existing tests
   - Add new tests using mocks
   - Ensure no behavior changes