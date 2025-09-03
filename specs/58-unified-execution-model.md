---
number: 58
title: Unified Execution Model
category: architecture
priority: high
status: draft
dependencies: [57]
created: 2025-09-03
---

# Specification 58: Unified Execution Model

**Category**: architecture
**Priority**: high
**Status**: draft
**Dependencies**: [57 - Claude Agent Observability]

## Context

The current workflow execution system has evolved organically, resulting in multiple divergent execution paths that duplicate logic and create feature inconsistency. As documented in the Architecture Assessment, the current implementation has three distinct execution paths:

1. **Standard Workflow Path** (`execute_workflow`) - Full feature support with WorkflowExecutor
2. **Args/Map Path** (`execute_workflow_with_args`) - Arguments and file pattern processing
3. **MapReduce Path** (`execute_mapreduce_workflow`) - Parallel execution mode

This divergence has created several critical issues:
- **Feature Inconsistency**: New features like validation only work in certain paths
- **Code Duplication**: Command execution, variable substitution, and git verification logic repeated
- **Testing Complexity**: Same features must be tested across multiple code paths
- **Maintenance Burden**: Bug fixes and new features require changes in multiple places

The validation bug that was recently fixed exemplifies this problem - validation configuration was lost when converting between WorkflowCommand and Command formats in the args/map execution path.

## Objective

Create a unified workflow execution model that consolidates all execution modes into a single, consistent pipeline. This model should handle sequential, parallel, and MapReduce execution through a common interface while maintaining full feature compatibility across all modes.

## Requirements

### Functional Requirements

#### Unified Execution Interface
- Single execution entry point for all workflow types
- Support for sequential, parallel, and MapReduce execution modes
- Consistent feature availability across all execution modes
- Backward compatibility with existing workflow configurations
- Proper handling of validation, handlers, timeouts, and all workflow features

#### Execution Mode Support
- **Sequential Mode**: Traditional step-by-step execution
- **Parallel Mode**: Concurrent execution with configurable worker limits
- **MapReduce Mode**: Map phase followed by optional reduce phase
- **Hybrid Modes**: Combinations of sequential and parallel execution within workflows

#### Feature Consistency
- Validation configuration preserved across all execution paths
- Handler execution (on_failure, on_success, on_exit_code) in all modes
- Timeout and working directory support universally available
- Environment variable handling consistent across modes
- Git commit requirements and verification in all paths

#### Configuration Abstraction
- WorkflowStep as the canonical representation for all commands
- Consistent variable substitution across execution modes
- Unified error handling and recovery mechanisms
- Common progress reporting and observability integration

### Non-Functional Requirements

#### Performance
- Zero performance regression for existing workflows
- Efficient resource utilization in parallel modes
- Optimal scheduling for MapReduce workloads
- Minimal memory overhead for sequential execution

#### Maintainability
- Single source of truth for execution logic
- Clear separation between orchestration and execution
- Testable components with minimal coupling
- Comprehensive error handling and logging

#### Compatibility
- Full backward compatibility with existing workflows
- Graceful migration path from current architecture
- No breaking changes to public APIs
- Support for existing configuration formats

## Acceptance Criteria

- [ ] Single UnifiedWorkflowExecutor handles all execution modes
- [ ] All workflow features work consistently across modes (validation, handlers, timeouts)
- [ ] Sequential workflows execute with zero performance regression
- [ ] Parallel workflows support configurable concurrency limits
- [ ] MapReduce workflows maintain current functionality and performance
- [ ] Variable substitution works consistently across all modes
- [ ] Git commit verification applies uniformly to all execution paths
- [ ] Progress reporting integrates with observability system
- [ ] Error handling provides consistent behavior across modes
- [ ] Existing workflow configurations continue to work unchanged
- [ ] Comprehensive test coverage for all execution modes
- [ ] Migration guide documents changes for advanced users

## Technical Details

### Implementation Approach

#### Phase 1: Unified Executor Interface

```rust
// src/cook/workflow/unified_executor.rs
pub struct UnifiedWorkflowExecutor<C: CommandExecutor> {
    command_executor: Arc<C>,
    mode: ExecutionMode,
    config: ExecutionConfig,
    progress_reporter: Arc<dyn ProgressReporter>,
    observability: Arc<dyn ObservabilityCollector>,
}

#[derive(Debug, Clone)]
pub enum ExecutionMode {
    Sequential,
    Parallel { 
        max_workers: usize,
        batch_size: Option<usize>,
    },
    MapReduce { 
        map_config: MapConfig,
        reduce_config: Option<ReduceConfig>,
        intermediate_storage: StorageConfig,
    },
    Hybrid {
        phases: Vec<ExecutionPhase>,
    },
}

#[derive(Debug, Clone)]
pub struct ExecutionPhase {
    pub name: String,
    pub mode: ExecutionMode,
    pub steps: Range<usize>,
    pub dependencies: Vec<String>,
}

impl<C: CommandExecutor> UnifiedWorkflowExecutor<C> {
    pub async fn execute(
        &mut self,
        workflow: &NormalizedWorkflow,
        env: &ExecutionEnvironment,
    ) -> Result<ExecutionResult> {
        match &self.mode {
            ExecutionMode::Sequential => {
                self.execute_sequential(workflow, env).await
            }
            ExecutionMode::Parallel { max_workers, batch_size } => {
                self.execute_parallel(workflow, env, *max_workers, *batch_size).await
            }
            ExecutionMode::MapReduce { map_config, reduce_config, intermediate_storage } => {
                self.execute_mapreduce(workflow, env, map_config, reduce_config, intermediate_storage).await
            }
            ExecutionMode::Hybrid { phases } => {
                self.execute_hybrid(workflow, env, phases).await
            }
        }
    }
    
    async fn execute_sequential(
        &mut self,
        workflow: &NormalizedWorkflow,
        env: &ExecutionEnvironment,
    ) -> Result<ExecutionResult> {
        let mut context = ExecutionContext::new(env);
        let mut results = Vec::new();
        
        for (index, step) in workflow.steps.iter().enumerate() {
            self.observability.record_step_start(index, step).await;
            
            let step_result = self.execute_single_step(step, &mut context).await?;
            
            self.observability.record_step_complete(index, &step_result).await;
            self.progress_reporter.report_step_completion(index, workflow.steps.len()).await;
            
            results.push(step_result);
            
            // Check for early termination conditions
            if self.should_terminate(&step_result, &context)? {
                break;
            }
        }
        
        Ok(ExecutionResult::new(results))
    }
    
    async fn execute_parallel(
        &mut self,
        workflow: &NormalizedWorkflow,
        env: &ExecutionEnvironment,
        max_workers: usize,
        batch_size: Option<usize>,
    ) -> Result<ExecutionResult> {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_workers));
        let inputs = workflow.generate_parallel_inputs()?;
        let batch_size = batch_size.unwrap_or(inputs.len().min(100));
        
        let mut all_results = Vec::new();
        
        // Process inputs in batches to manage memory
        for batch in inputs.chunks(batch_size) {
            let batch_tasks: Vec<_> = batch
                .iter()
                .enumerate()
                .map(|(index, input)| {
                    let permit = semaphore.clone();
                    let executor = self.command_executor.clone();
                    let workflow = workflow.clone();
                    let env = env.clone();
                    let input = input.clone();
                    let observability = self.observability.clone();
                    
                    tokio::spawn(async move {
                        let _permit = permit.acquire().await?;
                        
                        observability.record_parallel_task_start(index, &input).await;
                        let result = Self::execute_with_input(
                            &executor, &workflow, &env, &input
                        ).await;
                        observability.record_parallel_task_complete(index, &result).await;
                        
                        result
                    })
                })
                .collect();
            
            // Wait for batch completion
            let batch_results: Result<Vec<_>> = futures::future::join_all(batch_tasks)
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .collect();
                
            all_results.extend(batch_results?);
            
            self.progress_reporter.report_batch_completion(
                all_results.len(), inputs.len()
            ).await;
        }
        
        Ok(ExecutionResult::from_parallel_results(all_results))
    }
    
    async fn execute_mapreduce(
        &mut self,
        workflow: &NormalizedWorkflow,
        env: &ExecutionEnvironment,
        map_config: &MapConfig,
        reduce_config: &Option<ReduceConfig>,
        storage_config: &StorageConfig,
    ) -> Result<ExecutionResult> {
        // Map phase
        let map_results = self.execute_map_phase(
            workflow, env, map_config, storage_config
        ).await?;
        
        // Optional reduce phase
        let final_results = if let Some(reduce_config) = reduce_config {
            self.execute_reduce_phase(
                &map_results, env, reduce_config, storage_config
            ).await?
        } else {
            map_results
        };
        
        Ok(ExecutionResult::from_mapreduce_results(final_results))
    }
}
```

#### Phase 2: Workflow Normalization

```rust
// src/cook/workflow/normalization.rs
#[derive(Debug, Clone)]
pub struct NormalizedWorkflow {
    pub name: String,
    pub steps: Vec<NormalizedStep>,
    pub global_config: GlobalStepConfig,
    pub execution_inputs: Vec<ExecutionInput>,
}

#[derive(Debug, Clone)]
pub struct NormalizedStep {
    pub id: String,
    pub command: CommandSpec,
    pub validation: Option<ValidationConfig>,
    pub handlers: StepHandlers,
    pub timeout: Option<Duration>,
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub commit_required: bool,
    pub parallel_safe: bool,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StepHandlers {
    pub on_failure: Vec<HandlerStep>,
    pub on_success: Vec<HandlerStep>,
    pub on_exit_code: HashMap<i32, Vec<HandlerStep>>,
}

impl NormalizedWorkflow {
    pub fn from_workflow_config(
        config: &WorkflowConfig,
        inputs: Vec<ExecutionInput>,
    ) -> Result<Self> {
        let steps = config
            .steps
            .iter()
            .enumerate()
            .map(|(index, cmd)| Self::normalize_command(cmd, index))
            .collect::<Result<Vec<_>>>()?;
            
        Ok(Self {
            name: config.name.clone().unwrap_or_else(|| "default".to_string()),
            steps,
            global_config: GlobalStepConfig::from_config(config),
            execution_inputs: inputs,
        })
    }
    
    fn normalize_command(cmd: &WorkflowCommand, index: usize) -> Result<NormalizedStep> {
        match cmd {
            WorkflowCommand::WorkflowStep(step) => {
                Ok(NormalizedStep {
                    id: format!("step_{}", index),
                    command: Self::extract_command_spec(step)?,
                    validation: step.validate.clone(),
                    handlers: Self::extract_handlers(step),
                    timeout: step.timeout,
                    working_dir: step.working_dir.clone(),
                    env: step.env.clone().unwrap_or_default(),
                    commit_required: step.commit_required,
                    parallel_safe: Self::is_parallel_safe(step),
                    dependencies: Vec::new(), // TODO: Extract from step configuration
                })
            }
            _ => {
                // Convert other command types to normalized format
                let mut command = cmd.to_command();
                crate::config::apply_command_defaults(&mut command);
                
                Ok(NormalizedStep {
                    id: format!("step_{}", index),
                    command: CommandSpec::Claude(command.name.clone()),
                    validation: None,
                    handlers: StepHandlers::default(),
                    timeout: command.timeout.map(Duration::from_secs),
                    working_dir: command.working_dir,
                    env: command.env.unwrap_or_default(),
                    commit_required: Self::determine_commit_required(cmd, &command),
                    parallel_safe: Self::is_command_parallel_safe(&command.name),
                    dependencies: Vec::new(),
                })
            }
        }
    }
    
    pub fn generate_parallel_inputs(&self) -> Result<Vec<ExecutionInput>> {
        if self.execution_inputs.is_empty() {
            // Single execution with no inputs
            Ok(vec![ExecutionInput::Empty])
        } else {
            Ok(self.execution_inputs.clone())
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExecutionInput {
    Empty,
    Arguments(Vec<String>),
    FilePattern(PathBuf),
    MapReduceItem { key: String, data: serde_json::Value },
    Custom(serde_json::Value),
}

#[derive(Debug, Clone)]
pub enum CommandSpec {
    Claude(String),
    Shell(String),
    Test { command: String, expected_exit_code: Option<i32> },
    Handler(String),
}
```

#### Phase 3: Execution Context Management

```rust
// src/cook/workflow/execution_context.rs
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub variables: HashMap<String, String>,
    pub environment: ExecutionEnvironment,
    pub iteration: usize,
    pub max_iterations: usize,
    pub current_input: Option<ExecutionInput>,
    pub accumulated_outputs: HashMap<String, String>,
    pub git_state: GitState,
}

#[derive(Debug, Clone)]
pub struct GitState {
    pub initial_commit: String,
    pub current_commit: String,
    pub pending_changes: bool,
    pub commits_since_start: Vec<String>,
}

impl ExecutionContext {
    pub fn new(env: &ExecutionEnvironment) -> Self {
        let mut variables = HashMap::new();
        
        // Standard variables available in all executions
        variables.insert("session_id".to_string(), env.session_id.clone());
        variables.insert("working_dir".to_string(), 
            env.working_dir.to_string_lossy().to_string());
        variables.insert("project_dir".to_string(),
            env.project_dir.to_string_lossy().to_string());
        
        if let Some(worktree_name) = &env.worktree_name {
            variables.insert("worktree_name".to_string(), worktree_name.clone());
        }
        
        Self {
            variables,
            environment: env.clone(),
            iteration: 1,
            max_iterations: 1,
            current_input: None,
            accumulated_outputs: HashMap::new(),
            git_state: GitState::initial(),
        }
    }
    
    pub fn with_input(&mut self, input: ExecutionInput) -> &mut Self {
        self.current_input = Some(input.clone());
        
        // Add input-specific variables
        match input {
            ExecutionInput::Arguments(args) => {
                for (i, arg) in args.iter().enumerate() {
                    self.variables.insert(format!("arg_{}", i), arg.clone());
                }
                self.variables.insert("args".to_string(), args.join(" "));
            }
            ExecutionInput::FilePattern(path) => {
                self.variables.insert("file_path".to_string(),
                    path.to_string_lossy().to_string());
                if let Some(file_name) = path.file_name() {
                    self.variables.insert("file_name".to_string(),
                        file_name.to_string_lossy().to_string());
                }
                if let Some(parent) = path.parent() {
                    self.variables.insert("file_dir".to_string(),
                        parent.to_string_lossy().to_string());
                }
            }
            ExecutionInput::MapReduceItem { key, data } => {
                self.variables.insert("mr_key".to_string(), key);
                self.variables.insert("mr_data".to_string(), data.to_string());
            }
            _ => {}
        }
        
        self
    }
    
    pub fn substitute_variables(&self, template: &str) -> String {
        let mut result = template.to_string();
        
        for (key, value) in &self.variables {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
        }
        
        result
    }
    
    pub async fn update_git_state(&mut self, git_ops: &dyn GitOperations) -> Result<()> {
        let current_commit = git_ops.get_current_commit_hash().await?;
        let has_changes = git_ops.has_uncommitted_changes().await?;
        
        if current_commit != self.git_state.current_commit {
            self.git_state.commits_since_start.push(current_commit.clone());
            self.git_state.current_commit = current_commit;
        }
        
        self.git_state.pending_changes = has_changes;
        Ok(())
    }
}
```

### Architecture Changes

#### Component Relationships
```
┌─────────────────────────────────────────────────────┐
│                CookOrchestrator                      │
│  ┌─────────────────────────────────────────────┐    │
│  │         Workflow Classification              │    │
│  │                                             │    │
│  │  classify_workflow_type() → ExecutionMode  │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────┘
                     │
┌─────────────────────▼───────────────────────────────┐
│             UnifiedWorkflowExecutor                 │
│  ┌─────────────────────────────────────────────┐    │
│  │         Workflow Normalization              │    │
│  │                                             │    │
│  │  WorkflowConfig → NormalizedWorkflow        │    │
│  │  ExecutionInput → ExecutionContext          │    │
│  └─────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────┐    │
│  │           Execution Engine                  │    │
│  │                                             │    │
│  │  • Sequential Executor                     │    │
│  │  • Parallel Executor                       │    │
│  │  • MapReduce Executor                      │    │
│  │  • Hybrid Executor                         │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────┘
                     │
┌─────────────────────▼───────────────────────────────┐
│              Command Execution Layer                 │
│  ┌─────────────────────────────────────────────┐    │
│  │            CommandExecutor                   │    │
│  │                                             │    │
│  │  • ClaudeExecutor                          │    │
│  │  • ShellExecutor                           │    │
│  │  • TestExecutor                            │    │
│  │  • HandlerExecutor                         │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

#### Data Flow
1. **Classification**: CookOrchestrator analyzes WorkflowConfig and determines ExecutionMode
2. **Normalization**: UnifiedWorkflowExecutor converts WorkflowConfig to NormalizedWorkflow
3. **Context Creation**: ExecutionContext created with environment and input-specific variables
4. **Execution**: Appropriate execution strategy invoked based on mode
5. **Step Processing**: Each step processed through single command execution pipeline
6. **Result Aggregation**: Results collected and formatted according to execution mode

### Data Structures

#### Configuration Schema
```rust
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub timeout: Option<Duration>,
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub commit_verification: bool,
    pub progress_reporting: bool,
    pub observability_enabled: bool,
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_memory_mb: Option<u64>,
    pub max_cpu_percent: Option<f32>,
    pub max_concurrent_operations: Option<usize>,
    pub disk_space_threshold_mb: Option<u64>,
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub mode: ExecutionMode,
    pub steps_executed: usize,
    pub steps_successful: usize,
    pub total_duration: Duration,
    pub individual_results: Vec<StepResult>,
    pub aggregated_outputs: HashMap<String, String>,
    pub git_commits_created: Vec<String>,
    pub errors: Vec<ExecutionError>,
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_id: String,
    pub command: CommandSpec,
    pub status: ExecutionStatus,
    pub duration: Duration,
    pub output: Option<String>,
    pub error: Option<ExecutionError>,
    pub git_commit: Option<String>,
    pub validation_result: Option<ValidationResult>,
}

#[derive(Debug, Clone)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Success,
    Failed { retryable: bool },
    Skipped { reason: String },
    TimedOut,
}
```

### APIs and Interfaces

#### Unified Executor Interface
```rust
#[async_trait]
pub trait WorkflowExecutor: Send + Sync {
    async fn execute(
        &mut self,
        workflow: NormalizedWorkflow,
        env: ExecutionEnvironment,
    ) -> Result<ExecutionResult>;
    
    async fn execute_step(
        &mut self,
        step: &NormalizedStep,
        context: &mut ExecutionContext,
    ) -> Result<StepResult>;
    
    fn supports_mode(&self, mode: &ExecutionMode) -> bool;
    
    async fn validate_workflow(&self, workflow: &NormalizedWorkflow) -> Result<Vec<ValidationIssue>>;
    
    async fn estimate_duration(&self, workflow: &NormalizedWorkflow) -> Result<Option<Duration>>;
}
```

#### Command Executor Abstraction
```rust
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn execute(
        &self,
        command: &CommandSpec,
        context: &ExecutionContext,
    ) -> Result<CommandResult>;
    
    fn supports_command(&self, command: &CommandSpec) -> bool;
    
    async fn validate_command(&self, command: &CommandSpec) -> Result<Vec<ValidationIssue>>;
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub status: ExecutionStatus,
    pub output: Option<String>,
    pub error_output: Option<String>,
    pub exit_code: Option<i32>,
    pub duration: Duration,
    pub resources_used: ResourceUsage,
}
```

## Dependencies

- **Prerequisites**: 
  - Specification 57: Claude Agent Observability (for progress reporting integration)
- **Affected Components**:
  - `src/cook/orchestrator.rs`: Remove duplicate execution methods
  - `src/cook/workflow/`: Complete refactoring of execution system
  - `src/cook/execution/`: Consolidate command executors
  - `src/config/`: Update configuration handling
- **External Dependencies**:
  - `tokio`: Async runtime for parallel execution
  - `futures`: Stream processing and concurrent execution
  - `serde`: Configuration serialization/deserialization

## Testing Strategy

### Unit Tests
- Workflow normalization from various input formats
- ExecutionContext variable substitution
- Command execution through unified interface
- Error handling and recovery mechanisms
- Resource limit enforcement

### Integration Tests
- End-to-end workflow execution in all modes
- Feature consistency across execution paths
- Backward compatibility with existing configurations
- Performance benchmarks for all execution modes
- Observability integration verification

### Performance Tests
- Sequential execution performance comparison
- Parallel execution scaling characteristics
- MapReduce overhead measurement
- Memory usage analysis under load
- Resource limit effectiveness testing

### Migration Tests
- Existing workflow configurations continue to work
- Feature parity verification across all modes
- Error message consistency and quality
- Progress reporting accuracy

## Documentation Requirements

### Code Documentation
- Unified execution model architecture
- Workflow normalization process
- Execution context management
- Command executor abstractions
- Error handling strategies

### User Documentation
- Migration guide from current architecture
- New execution mode capabilities
- Performance tuning recommendations
- Troubleshooting unified execution issues
- Best practices for workflow design

### Architecture Updates
- Update ARCHITECTURE.md with unified execution model
- Document execution flow diagrams
- Add component interaction diagrams
- Include performance characteristics documentation

## Implementation Notes

### Migration Strategy
- Phase 1: Implement unified executor alongside existing system
- Phase 2: Gradual migration of execution paths to use unified model
- Phase 3: Remove legacy execution code after validation
- Phase 4: Optimize unified system for performance and resource usage

### Backward Compatibility
- All existing WorkflowConfig formats supported
- No changes required to existing workflow files
- Legacy command line arguments continue to work
- Gradual deprecation of old internal APIs

### Performance Considerations
- Zero-copy optimization for sequential execution
- Efficient batching for parallel operations
- Smart scheduling for MapReduce workloads
- Resource pooling for command executors
- Lazy loading of execution contexts

### Error Handling
- Consistent error types across all execution modes
- Proper error propagation and aggregation
- Detailed error context for debugging
- Graceful degradation on resource constraints
- Recovery mechanisms for transient failures

## Migration and Compatibility

### Breaking Changes
- None for end users and standard workflow configurations
- Internal APIs for custom executors will need updating
- Advanced configuration options may have different names
- Some internal metrics and logging formats will change

### Migration Timeline
1. **Week 1-2**: Implement core unified executor and normalization
2. **Week 3**: Add parallel and MapReduce execution modes  
3. **Week 4**: Integrate observability and progress reporting
4. **Week 5**: Add comprehensive test coverage
5. **Week 6**: Migration from legacy execution paths
6. **Week 7**: Performance optimization and tuning
7. **Week 8**: Documentation and final validation

### Rollback Plan
- Feature flag to enable/disable unified execution
- Ability to fall back to legacy execution paths
- Monitoring to detect performance regressions
- Quick rollback mechanism for production issues