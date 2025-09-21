# Prodigy Architecture

## System Overview

Prodigy is a workflow orchestration tool designed around functional programming principles with a clear separation between pure business logic and I/O operations. The architecture follows a modular, trait-based design for testability and extensibility.

## Core Architecture Patterns

### Functional Core, Imperative Shell
- **Pure Core**: Business logic implemented as pure functions in domain modules
- **Imperative Shell**: I/O operations isolated at module boundaries
- **Data Flow**: Immutable data transformations with explicit state management

### Dependency Injection
- Trait-based abstractions for all external dependencies
- Constructor injection for testability
- Mock implementations for unit testing

### Event-Driven Processing
- Async event processing with tokio channels
- Cross-worktree event aggregation
- Persistent event logging for debugging

## Module Structure

```
prodigy/
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── lib.rs                  # Public API
│   ├── error/                  # Unified error handling system
│   │   ├── mod.rs              # Core error types and traits
│   │   ├── codes.rs            # Error code registry (E0001-E9999)
│   │   ├── helpers.rs          # Migration helpers and extensions
│   │   └── migration_example.rs # Migration patterns and examples
│   ├── storage/                # Storage abstraction layer
│   │   ├── mod.rs              # Storage module exports
│   │   ├── traits.rs           # Core storage traits
│   │   ├── types.rs            # Storage data types
│   │   ├── error.rs            # Storage error types
│   │   ├── config.rs           # Storage configuration
│   │   ├── factory.rs          # Storage factory
│   │   ├── lock.rs             # Distributed locking
│   │   └── backends/           # Storage backend implementations
│   │       ├── file.rs         # File-based storage
│   │       └── memory.rs       # In-memory storage (testing)
│   ├── subprocess/             # Subprocess management
│   │   ├── streaming/          # Real-time streaming infrastructure
│   │   │   ├── processor.rs    # Stream processor trait and implementations
│   │   │   ├── runner.rs       # Streaming command runner
│   │   │   ├── backpressure.rs # Backpressure management
│   │   │   └── types.rs        # Core streaming types
│   ├── analytics/              # Claude session analytics
│   │   ├── mod.rs              # Module exports
│   │   ├── models.rs           # Data models for sessions
│   │   ├── session_watcher.rs  # JSONL file monitoring
│   │   ├── engine.rs           # Analytics computation
│   │   └── replay.rs           # Session replay functionality
│   ├── config/                 # Configuration management
│   │   ├── command.rs          # Command parsing and validation
│   │   ├── loader.rs           # Configuration loading
│   │   └── mapreduce.rs        # MapReduce configuration
│   ├── commands/               # Command execution framework
│   │   ├── handlers/           # Command-specific handlers
│   │   ├── context.rs          # Execution context
│   │   └── registry.rs         # Command registration
│   ├── cook/                   # Core workflow orchestration
│   │   ├── environment/        # Environment management
│   │   │   ├── mod.rs          # Environment utilities
│   │   │   ├── config.rs       # Environment configuration types
│   │   │   ├── manager.rs      # Environment manager
│   │   │   ├── path_resolver.rs # Cross-platform path handling
│   │   │   └── secret_store.rs # Secret management
│   │   ├── execution/          # Command execution engine
│   │   │   ├── mod.rs          # CommandExecutor trait
│   │   │   ├── mapreduce/      # MapReduce execution
│   │   │   │   ├── mod.rs      # Main MapReduce executor
│   │   │   │   ├── agent.rs    # Agent lifecycle management
│   │   │   │   ├── utils.rs    # Pure utility functions
│   │   │   │   ├── phases/     # Phase execution orchestration
│   │   │   │   │   ├── mod.rs  # Phase executor traits and types
│   │   │   │   │   ├── coordinator.rs # Phase transition orchestration
│   │   │   │   │   ├── setup.rs # Setup phase executor
│   │   │   │   │   ├── map.rs  # Map phase orchestrator
│   │   │   │   │   └── reduce.rs # Reduce phase executor
│   │   │   │   └── command/    # Command execution abstraction
│   │   │   │       ├── mod.rs  # Module exports
│   │   │   │       ├── executor.rs # CommandExecutor trait and router
│   │   │   │       ├── claude.rs   # Claude command executor
│   │   │   │       ├── shell.rs    # Shell command executor
│   │   │   │       ├── handler.rs  # Handler command executor
│   │   │   │       └── interpolation.rs # Variable interpolation
│   │   │   ├── mapreduce_resume.rs # Enhanced resume functionality
│   │   │   ├── foreach.rs      # Simple parallel iteration
│   │   │   └── claude.rs       # Claude integration
│   │   ├── goal_seek/          # Goal-seeking primitives
│   │   │   ├── engine.rs       # Refinement engine
│   │   │   ├── validator.rs    # Validation framework
│   │   │   ├── validators.rs   # Built-in validators
│   │   │   └── shell_executor.rs # Shell command execution
│   │   ├── retry_v2.rs         # Enhanced retry strategies
│   │   │   ├── RetryConfig     # Backoff configuration
│   │   │   ├── RetryExecutor   # Retry execution engine
│   │   │   ├── CircuitBreaker  # Failure protection
│   │   │   └── RetryMetrics    # Observability
│   │   ├── workflow/           # Workflow management
│   │   │   ├── executor.rs     # Step execution
│   │   │   ├── normalized.rs   # Workflow normalization
│   │   │   ├── on_failure.rs   # Enhanced error handling with strategies
│   │   │   ├── checkpoint.rs   # Checkpoint management
│   │   │   ├── resume.rs       # Resume execution
│   │   │   └── composition/    # Workflow composition
│   │   │       ├── mod.rs      # Composition structures
│   │   │       ├── composer.rs # Workflow composer
│   │   │       ├── registry.rs # Template registry
│   │   │       └── sub_workflow.rs # Sub-workflow execution
│   │   ├── coordinators/       # High-level coordination
│   │   ├── session/            # Session management
│   │   └── orchestrator.rs     # Main orchestration
│   ├── session/                # Session state management
│   ├── testing/                # Test utilities
│   │   └── mocks/              # Mock implementations
│   └── abstractions/           # Common abstractions
├── tests/                      # Test infrastructure
│   ├── cli_integration/        # CLI integration tests ✅
│   │   ├── test_utils.rs       # Test harness and utilities
│   │   ├── run_command_tests.rs
│   │   ├── exec_command_tests.rs
│   │   ├── batch_command_tests.rs
│   │   ├── worktree_command_tests.rs
│   │   ├── events_command_tests.rs
│   │   ├── dlq_command_tests.rs
│   │   ├── argument_parsing_tests.rs
│   │   ├── configuration_tests.rs
│   │   ├── signal_handling_tests.rs
│   │   └── verbose_output_tests.rs
│   └── *.rs                    # Other integration tests
├── workflows/                  # Example workflows
├── docs/                       # Documentation
└── .prodigy/                   # Project context files
```

## Key Traits and Interfaces

### UnifiedStorage
```rust
#[async_trait]
pub trait UnifiedStorage: Send + Sync {
    fn session_storage(&self) -> &dyn SessionStorage;
    fn event_storage(&self) -> &dyn EventStorage;
    fn checkpoint_storage(&self) -> &dyn CheckpointStorage;
    fn dlq_storage(&self) -> &dyn DLQStorage;
    fn workflow_storage(&self) -> &dyn WorkflowStorage;
    async fn acquire_lock(&self, key: &str, ttl: Duration) -> StorageResult<Box<dyn StorageLockGuard>>;
    async fn health_check(&self) -> StorageResult<HealthStatus>;
}
```
**Implementations**:
- `FileBackend`: File-based storage (default)
- `MemoryBackend`: In-memory storage (testing)
- `PostgresBackend`: PostgreSQL storage (planned)
- `RedisBackend`: Redis storage (planned)

### CommandExecutor
```rust
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn execute(
        &self,
        command: &str,
        args: &[String],
        context: ExecutionContext,
    ) -> Result<ExecutionResult>;
}
```
**Implementations**:
- `ClaudeExecutor`: Claude Code CLI integration
- `ShellCommandExecutor`: Shell command execution
- `CommandExecutorMock`: Testing mock

### Validator (Goal-Seeking)
```rust
pub trait Validator: Send + Sync {
    fn validate(&self, output: &str) -> Result<ValidationResult>;
    fn name(&self) -> &str;
}
```
**Implementations**:
- `SpecCoverageValidator`: Specification coverage analysis
- `TestPassValidator`: Test execution validation
- `OutputQualityValidator`: Code quality metrics

### PhaseExecutor (MapReduce Phases)
```rust
#[async_trait]
pub trait PhaseExecutor: Send + Sync {
    async fn execute(&self, context: &mut PhaseContext) -> Result<PhaseResult, PhaseError>;
    fn phase_type(&self) -> PhaseType;
    fn can_skip(&self, context: &PhaseContext) -> bool;
    fn validate_context(&self, context: &PhaseContext) -> Result<(), PhaseError>;
}
```
**Implementations**:
- `SetupPhaseExecutor`: Setup command execution and work item generation
- `MapPhaseExecutor`: Parallel work item distribution and agent orchestration
- `ReducePhaseExecutor`: Result aggregation and reduce command execution

### SessionManager
```rust
#[async_trait]
pub trait SessionManager: Send + Sync {
    async fn start_session(&mut self) -> Result<String>;
    async fn complete_session(&mut self, success: bool) -> Result<()>;
    async fn track_iteration(&mut self) -> Result<()>;
}
```

### EnvironmentManager
Manages environment variables and working directories for workflow execution:
- Setup step-specific environments
- Handle secret management
- Resolve dynamic and conditional values
- Support environment profiles
- Cross-platform path resolution

## Data Flow Architecture

### Workflow Processing
```
YAML Config → NormalizedWorkflow → WorkflowSteps → ExecutionResults
     ↓              ↓                   ↓              ↓
Configuration   Validation         Execution      Result
   Parsing      & Planning         Engine        Aggregation
                                       ↓
                                 Checkpoint
                                  Creation
```

### Checkpoint & Resume Flow
```
Workflow → ExecuteSteps → SaveCheckpoint → Interruption
    ↓          ↓              ↓               ↓
  Start    Progress      Periodic        LoadCheckpoint
           Tracking      Saves               ↓
                                         ResumeExecution
                                              ↓
                                         SkipCompleted →
                                         ContinueFrom
```

### Goal-Seeking Flow
```
GoalSeekConfig → GoalSeekEngine → AttemptRecord → GoalSeekResult
      ↓              ↓               ↓              ↓
   Parameters    Iterative       History        Termination
                 Execution      Tracking        Condition
```

### MapReduce Processing
```
WorkItems → AgentPool → ParallelExecution → ResultAggregation
    ↓          ↓             ↓                   ↓
  Data        Agent        Distributed         Reduced
 Source      Creation       Processing          Result
```

## Storage Architecture

### Storage Abstraction Layer
The storage abstraction layer provides a unified interface for all storage operations, enabling seamless switching between different backends:

- **Trait-Based Design**: All storage operations defined through traits
- **Multiple Backends**: File, PostgreSQL, Redis, S3, Memory
- **Distributed Locking**: Coordination for concurrent operations
- **Streaming Support**: Efficient handling of large datasets
- **Transaction Support**: Atomic operations where supported

### Backend Implementations

#### File Backend (Default)
- **Configurable Storage**: Base directory configurable via `PRODIGY_STORAGE_DIR` (default: `~/.prodigy/`)
- **File-Based Locking**: Exclusive file creation for coordination
- **JSON Serialization**: Human-readable data format
- **Directory Structure**:
  - `sessions/`: Session state files
  - `events/`: Event log files (JSONL)
  - `checkpoints/`: Workflow checkpoint files
  - `dlq/`: Dead letter queue items
  - `workflows/`: Workflow definitions
  - `locks/`: Lock files for coordination

#### Database Backends (Feature-Gated)
Optional backends available through Cargo feature flags:
- **PostgreSQL** (`--features postgres`): Full ACID compliance, complex queries
- **Redis** (`--features redis`): High-performance caching, pub/sub support
- **S3** (`--features s3`): Object storage for large-scale deployments
- **Distributed** (`--features distributed`): Enables all backend types

### Storage Configuration
- **Default Location**: `~/.prodigy/` (configurable via `PRODIGY_STORAGE_DIR`)
- **Events**: Cross-worktree event aggregation by repository
- **State**: MapReduce job checkpoints and session data
- **DLQ**: Failed work items for retry analysis
- **Worktrees**: Isolated git worktrees for parallel sessions
- **Migration**: Automatic migration from legacy local storage (`.prodigy/`) on first run

### Session State
```rust
pub struct SessionState {
    pub session_id: String,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub iterations_completed: u32,
    pub files_changed: u32,
    pub iteration_timings: Vec<(u32, Duration)>,
    pub command_timings: Vec<(String, Duration)>,
}
```

## Error Handling Strategy

### Error Types
- **Configuration Errors**: Invalid YAML, missing commands
- **Execution Errors**: Command failures, timeouts
- **Validation Errors**: Spec validation, test failures
- **System Errors**: File I/O, git operations

### Error Propagation
- Use `anyhow::Result` for application errors
- Include context with `.context()` method
- Fail fast with descriptive error messages
- Support error recovery in workflow `on_failure` handlers

### Goal-Seeking Error Handling
- Execution failures become attempt records
- Validation failures inform next attempts
- Convergence detection prevents infinite loops
- Timeout protection for long-running operations

## Async Architecture

### Runtime
- Single `tokio` runtime for all async operations
- Async command execution with timeout support
- Channel-based communication between components

### Concurrency Model
- **MapReduce**: Multiple agents in separate worktrees
- **Goal-Seeking**: Sequential attempts with async validation
- **Session Management**: Concurrent session tracking

## Testing Strategy

### Unit Tests
- Pure function testing with property-based tests
- Mock implementations for all external dependencies
- Isolated testing of business logic

### Integration Tests
- End-to-end workflow execution
- CLI command testing with comprehensive coverage ✅
- File system integration testing
- Signal handling and graceful shutdown tests ✅
- Configuration loading and precedence tests ✅
- Argument parsing and validation tests ✅

### Test Utilities
- `CommandExecutorMock`: Predictable command execution
- `MockSubprocessManager`: Shell command mocking
- Test fixtures for workflow configuration
- `CliTest`: Comprehensive CLI test harness ✅
- `CliOutput`: Structured test output validation ✅

## Performance Considerations

### Optimization Points
- Event log rotation and archival
- Session state compression
- Parallel validation in goal-seeking
- Command result caching

### Resource Management
- Git worktree cleanup after sessions
- Temporary file management
- Memory usage monitoring for large workflows

## Extension Points

### Custom Validators
Implement the `Validator` trait for domain-specific validation:
```rust
pub struct CustomValidator;

impl Validator for CustomValidator {
    fn validate(&self, output: &str) -> Result<ValidationResult> {
        // Custom validation logic
    }
    
    fn name(&self) -> &str {
        "custom_validator"
    }
}
```

### Custom Command Handlers
Add new command types through the handler system:
```rust
pub struct CustomHandler;

impl CommandHandler for CustomHandler {
    async fn execute(&self, request: CommandRequest) -> Result<CommandResult> {
        // Custom command logic
    }
}
```

This architecture supports the functional programming paradigm while providing the flexibility needed for a complex workflow orchestration system.