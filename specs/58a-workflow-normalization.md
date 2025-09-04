---
number: 58a
title: Workflow Normalization
category: architecture
priority: high
status: draft
parent: 58
created: 2025-09-04
---

# Specification 58a: Workflow Normalization

**Category**: architecture
**Priority**: high
**Status**: draft
**Parent**: [58 - Unified Execution Model]

## Context

As identified in Specification 58, the root cause of the validation bug is that we have 4 different execution paths in `DefaultCookOrchestrator` that handle workflows differently:

1. **Standard Path** (`execute_workflow`) - Works correctly with validation
2. **Structured Path** (`execute_structured_workflow`) - Loses validation during conversion
3. **Args/Map Path** (`execute_workflow_with_args`) - Loses validation during WorkflowCommand → Command conversion
4. **MapReduce Path** (`execute_mapreduce_workflow`) - Separate implementation, missing features

The key issue is that fields like `validation`, `handlers`, and `outputs` are lost during various conversions between WorkflowCommand and Command types.

## Objective

Create a normalized workflow representation that preserves ALL fields from the original configuration, ensuring no information is lost regardless of execution path.

## Technical Details

### Pure Functional Normalization

```rust
// src/cook/workflow/normalized.rs
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use anyhow::{Result, anyhow};
use uuid::Uuid;

/// Immutable normalized workflow representation
#[derive(Debug, Clone)]
pub struct NormalizedWorkflow {
    pub name: String,
    pub steps: Vec<NormalizedStep>,
    pub execution_mode: ExecutionMode,
    pub variables: HashMap<String, String>,
}

/// Immutable normalized step - preserves ALL fields
#[derive(Debug, Clone)]
pub struct NormalizedStep {
    pub id: String,
    pub command: Command,
    pub validation: Option<ValidationConfig>,  // NEVER lost
    pub handlers: StepHandlers,                // ALWAYS preserved
    pub timeout: Option<Duration>,
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub outputs: Option<OutputConfig>,         // For structured workflows
    pub commit_required: bool,
}

#[derive(Debug, Clone)]
pub enum ExecutionMode {
    Sequential,
    WithArguments { args: Vec<String> },
    WithFilePattern { pattern: String },
    MapReduce { config: MapReduceConfig },
}

impl NormalizedWorkflow {
    /// Pure function: Convert from any workflow type while preserving ALL fields
    /// No side effects, no mutations, returns Result for error handling
    pub fn from_workflow_config(
        config: &WorkflowConfig,
        mode: ExecutionMode,
    ) -> Result<Self> {
        // Use iterator combinators for functional transformation
        let steps = config.commands
            .iter()
            .map(|cmd| Self::normalize_command(cmd))
            .collect::<Result<Vec<_>>>()?;
            
        Ok(Self {
            name: config.name.clone(),
            steps,
            execution_mode: mode,
            variables: config.env.clone().unwrap_or_default(),
        })
    }
    
    /// Pure function: Transform WorkflowCommand to NormalizedStep
    /// Preserves ALL fields, no information loss
    fn normalize_command(cmd: &WorkflowCommand) -> Result<NormalizedStep> {
        match cmd {
            WorkflowCommand::WorkflowStep(step) => {
                // Direct preservation - immutable transformation
                Ok(NormalizedStep {
                    id: step.name.clone()
                        .unwrap_or_else(|| Uuid::new_v4().to_string()),
                    command: step.command.clone(),
                    validation: step.validate.clone(),  // PRESERVED
                    handlers: StepHandlers {
                        on_failure: step.on_failure.clone(),
                        on_success: step.on_success.clone(),
                        on_exit_code: step.on_exit_code.clone(),
                    },
                    timeout: step.timeout,
                    working_dir: step.working_directory.clone(),
                    env: step.env.clone().unwrap_or_default(),
                    outputs: step.outputs.clone(),
                    commit_required: step.commit.unwrap_or(false),
                })
            }
            WorkflowCommand::SimpleCommand(cmd) => {
                // Simple commands have minimal fields
                Ok(NormalizedStep {
                    id: Uuid::new_v4().to_string(),
                    command: cmd.clone(),
                    validation: None,
                    handlers: StepHandlers::default(),
                    timeout: None,
                    working_dir: None,
                    env: HashMap::new(),
                    outputs: None,
                    commit_required: false,
                })
            }
        }
    }
    
    /// Pure function: Classify workflow type based on configuration
    /// No side effects, deterministic output
    pub fn classify_workflow_type(config: &CookConfig) -> WorkflowType {
        if config.mapreduce_config.is_some() {
            WorkflowType::MapReduce
        } else if !config.args.is_empty() || !config.map.is_empty() {
            WorkflowType::WithArguments
        } else if Self::has_output_definitions(&config.workflow) {
            WorkflowType::StructuredWithOutputs
        } else {
            WorkflowType::Standard
        }
    }
    
    /// Pure function: Check if workflow has output definitions
    fn has_output_definitions(workflow: &WorkflowConfig) -> bool {
        workflow.commands.iter().any(|cmd| {
            matches!(cmd, WorkflowCommand::WorkflowStep(step) if step.outputs.is_some())
        })
    }
}
```

### Converting Back to Extended Config

```rust
impl NormalizedWorkflow {
    /// Pure function: Convert to ExtendedWorkflowConfig
    /// No mutations, returns new configuration
    pub fn to_extended_config(&self) -> Result<ExtendedWorkflowConfig> {
        let steps = self.steps
            .iter()
            .map(|step| self.normalized_to_workflow_step(step))
            .collect::<Result<Vec<_>>>()?;
            
        Ok(ExtendedWorkflowConfig {
            name: self.name.clone(),
            steps,
            env: Some(self.variables.clone()),
            max_iterations: self.extract_max_iterations()?,
        })
    }
    
    /// Pure function: Transform normalized step back to workflow step
    /// Preserves ALL fields, returns Result for validation
    fn normalized_to_workflow_step(&self, step: &NormalizedStep) -> Result<WorkflowStep> {
        // Validate step before transformation
        self.validate_step(step)?;
        
        Ok(WorkflowStep {
            name: Some(step.id.clone()),
            command: step.command.clone(),
            validate: step.validation.clone(),  // PRESERVED!
            on_failure: step.handlers.on_failure.clone(),
            on_success: step.handlers.on_success.clone(),
            on_exit_code: step.handlers.on_exit_code.clone(),
            timeout: step.timeout,
            working_directory: step.working_dir.clone(),
            env: Some(step.env.clone()),
            outputs: step.outputs.clone(),
            commit: Some(step.commit_required),
        })
    }
    
    /// Pure function: Validate step configuration
    fn validate_step(&self, step: &NormalizedStep) -> Result<()> {
        if step.id.is_empty() {
            return Err(anyhow!("Step ID cannot be empty"));
        }
        
        if let Some(timeout) = step.timeout {
            if timeout.as_secs() == 0 {
                return Err(anyhow!("Step timeout must be greater than 0"));
            }
        }
        
        Ok(())
    }
    
    /// Pure function: Extract max iterations from execution mode
    fn extract_max_iterations(&self) -> Result<usize> {
        match &self.execution_mode {
            ExecutionMode::Sequential => Ok(1),
            ExecutionMode::WithArguments { args } => Ok(args.len()),
            ExecutionMode::WithFilePattern { .. } => Ok(1),
            ExecutionMode::MapReduce { config } => {
                config.max_iterations
                    .ok_or_else(|| anyhow!("MapReduce config missing max_iterations"))
            }
        }
    }
}
```

## Key Insight

The problem isn't with the executors (ClaudeExecutor, CommandExecutor, MapReduceExecutor) - they work fine. The problem is that the orchestrator loses information when converting between different command representations. This normalization layer ensures ALL fields are preserved.

## Success Criteria

- [ ] No fields are lost during workflow normalization
- [ ] All 4 execution paths can use normalized workflows
- [ ] Validation configuration is preserved in all paths
- [ ] Pure functions with no side effects
- [ ] Comprehensive error handling with Result types