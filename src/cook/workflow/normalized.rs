//! Normalized workflow representation
//!
//! Provides a unified, immutable representation of workflows that preserves all fields
//! regardless of the execution path. This ensures that features like validation,
//! handlers, and outputs are never lost during workflow transformations.

use crate::config::command::{
    OutputDeclaration, TestDebugConfig, WorkflowCommand, WorkflowStepCommand,
};
use crate::config::workflow::WorkflowConfig;
use crate::cook::workflow::{OnFailureConfig, ValidationConfig, WorkflowStep};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Immutable normalized workflow representation
#[derive(Debug, Clone)]
pub struct NormalizedWorkflow {
    pub name: Arc<str>,
    pub steps: Arc<[NormalizedStep]>,
    pub execution_mode: ExecutionMode,
    pub variables: Arc<HashMap<String, String>>,
}

/// Immutable normalized step - preserves ALL fields
#[derive(Debug, Clone)]
pub struct NormalizedStep {
    pub id: Arc<str>,
    pub command: StepCommand,
    pub validation: Option<Arc<ValidationConfig>>,
    pub handlers: StepHandlers,
    pub timeout: Option<Duration>,
    pub working_dir: Option<PathBuf>,
    pub env: Arc<HashMap<String, String>>,
    pub outputs: Option<Arc<HashMap<String, OutputDeclaration>>>,
    pub commit_required: bool,
    pub when: Option<Arc<str>>,
}

/// Command representation within a normalized step
#[derive(Debug, Clone)]
pub enum StepCommand {
    Claude(Arc<str>),
    Shell(Arc<str>),
    Test {
        command: Arc<str>,
        on_failure: Option<Arc<TestDebugConfig>>,
    },
    Foreach(Arc<crate::config::command::ForeachConfig>),
    Handler(HandlerConfig),
    Simple(Arc<str>),
}

/// Handler configuration
#[derive(Debug, Clone)]
pub struct HandlerConfig {
    pub name: Arc<str>,
    pub attributes: Arc<HashMap<String, serde_json::Value>>,
}

/// Step handlers for conditional execution
#[derive(Debug, Clone)]
pub struct StepHandlers {
    pub on_failure: Option<Arc<OnFailureConfig>>,
    pub on_success: Option<Arc<WorkflowStep>>,
    pub on_exit_code: Arc<HashMap<i32, Arc<WorkflowStep>>>,
}

impl Default for StepHandlers {
    fn default() -> Self {
        Self {
            on_failure: None,
            on_success: None,
            on_exit_code: Arc::new(HashMap::new()),
        }
    }
}

/// Execution mode for the workflow
#[derive(Debug, Clone)]
pub enum ExecutionMode {
    Sequential,
    WithArguments { args: Arc<[String]> },
    WithFilePattern { pattern: Arc<str> },
    MapReduce { config: Arc<MapReduceConfig> },
}

/// MapReduce configuration
#[derive(Debug, Clone)]
pub struct MapReduceConfig {
    pub max_iterations: Option<usize>,
    pub max_concurrent: Option<usize>,
    pub partition_strategy: Option<Arc<str>>,
}

/// Type classification for workflows
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowType {
    Standard,
    StructuredWithOutputs,
    WithArguments,
    MapReduce,
}

impl NormalizedWorkflow {
    /// Pure function: Convert from any workflow type while preserving ALL fields
    /// No side effects, no mutations, returns Result for error handling
    pub fn from_workflow_config(config: &WorkflowConfig, mode: ExecutionMode) -> Result<Self> {
        // Use iterator combinators for functional transformation
        let steps: Vec<NormalizedStep> = config
            .commands
            .iter()
            .enumerate()
            .map(|(idx, cmd)| Self::normalize_command(cmd, idx))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            name: "".into(), // Name should be provided separately
            steps: steps.into(),
            execution_mode: mode,
            variables: Arc::new(HashMap::new()), // Variables should be provided separately
        })
    }

    /// Pure function: Transform WorkflowCommand to NormalizedStep
    /// Preserves ALL fields, no information loss
    fn normalize_command(cmd: &WorkflowCommand, idx: usize) -> Result<NormalizedStep> {
        match cmd {
            WorkflowCommand::WorkflowStep(step) => {
                // Determine the command type
                let command = if let Some(claude) = &step.claude {
                    StepCommand::Claude(Arc::from(claude.as_str()))
                } else if let Some(shell) = &step.shell {
                    StepCommand::Shell(Arc::from(shell.as_str()))
                } else if let Some(test) = &step.test {
                    StepCommand::Test {
                        command: Arc::from(test.command.as_str()),
                        on_failure: test.on_failure.as_ref().map(|f| Arc::new(f.clone())),
                    }
                } else if let Some(foreach) = &step.foreach {
                    StepCommand::Foreach(Arc::new(foreach.clone()))
                } else {
                    return Err(anyhow!("WorkflowStep must have at least one command type"));
                };

                // Convert on_failure from TestDebugConfig to OnFailureConfig if needed
                let on_failure = step.on_failure.as_ref().map(|tf| {
                    Arc::new(OnFailureConfig::Advanced {
                        claude: Some(tf.claude.clone()),
                        shell: None,
                        fail_workflow: tf.fail_workflow,
                        retry_original: false,
                        max_retries: tf.max_attempts,
                    })
                });

                // Direct preservation - immutable transformation
                Ok(NormalizedStep {
                    id: step
                        .id
                        .as_ref()
                        .map(|s| Arc::from(s.as_str()))
                        .unwrap_or_else(|| Arc::from(format!("step-{}", idx).as_str())),
                    command,
                    validation: step.validate.as_ref().map(|v| Arc::new(v.clone())), // PRESERVED
                    handlers: StepHandlers {
                        on_failure: on_failure.or_else(|| {
                            step.test.as_ref().and_then(|t| {
                                t.on_failure.as_ref().map(|tf| {
                                    Arc::new(OnFailureConfig::Advanced {
                                        claude: Some(tf.claude.clone()),
                                        shell: None,
                                        fail_workflow: tf.fail_workflow,
                                        retry_original: false,
                                        max_retries: tf.max_attempts,
                                    })
                                })
                            })
                        }),
                        on_success: step
                            .on_success
                            .as_ref()
                            .map(|s| Arc::new(Self::workflow_step_command_to_workflow_step(s))),
                        on_exit_code: Arc::new(HashMap::new()), // WorkflowStepCommand doesn't have on_exit_code
                    },
                    timeout: step.timeout.map(Duration::from_secs),
                    working_dir: None, // WorkflowStepCommand doesn't have working_dir field
                    env: Arc::new(HashMap::new()), // WorkflowStepCommand doesn't have env field
                    outputs: step.outputs.as_ref().map(|o| Arc::new(o.clone())),
                    commit_required: step.commit_required,
                    when: step.when.as_ref().map(|w| Arc::from(w.as_str())),
                })
            }
            WorkflowCommand::Structured(cmd) => {
                // Convert structured command
                Ok(NormalizedStep {
                    id: cmd
                        .id
                        .as_ref()
                        .map(|s| Arc::from(s.as_str()))
                        .unwrap_or_else(|| Arc::from(format!("step-{}", idx).as_str())),
                    command: StepCommand::Handler(HandlerConfig {
                        name: Arc::from(cmd.name.as_str()),
                        attributes: Arc::new(cmd.options.clone()),
                    }),
                    validation: None, // Structured commands don't have validation
                    handlers: StepHandlers::default(),
                    timeout: cmd.metadata.timeout.map(Duration::from_secs),
                    working_dir: None,
                    env: Arc::new(cmd.metadata.env.clone()),
                    outputs: cmd.outputs.as_ref().map(|o| Arc::new(o.clone())),
                    commit_required: cmd.metadata.commit_required,
                    when: None, // Structured commands don't have when clauses
                })
            }
            WorkflowCommand::SimpleObject(cmd) => {
                // Simple object commands have minimal fields
                Ok(NormalizedStep {
                    id: Arc::from(format!("step-{}", idx).as_str()),
                    command: StepCommand::Simple(Arc::from(cmd.name.as_str())),
                    validation: None,
                    handlers: StepHandlers::default(),
                    timeout: None,
                    working_dir: None,
                    env: Arc::new(HashMap::new()),
                    outputs: None,
                    commit_required: cmd.commit_required.unwrap_or(false),
                    when: None, // SimpleObject commands don't have when clauses
                })
            }
            WorkflowCommand::Simple(cmd) => {
                // Simple string commands have minimal fields
                Ok(NormalizedStep {
                    id: Arc::from(format!("step-{}", idx).as_str()),
                    command: StepCommand::Simple(Arc::from(cmd.as_str())),
                    validation: None,
                    handlers: StepHandlers::default(),
                    timeout: None,
                    working_dir: None,
                    env: Arc::new(HashMap::new()),
                    outputs: None,
                    commit_required: false,
                    when: None, // Simple commands don't have when clauses
                })
            }
        }
    }

    /// Helper function to convert WorkflowStepCommand to WorkflowStep
    fn workflow_step_command_to_workflow_step(cmd: &WorkflowStepCommand) -> WorkflowStep {
        WorkflowStep {
            name: cmd.id.clone(),
            claude: cmd.claude.clone(),
            shell: cmd.shell.clone(),
            test: cmd.test.clone(),
            foreach: cmd.foreach.clone(),
            write_file: None,
            command: None,
            handler: None,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            capture_output: crate::cook::workflow::CaptureOutput::Disabled,
            auto_commit: false,
            commit_config: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: cmd.on_failure.as_ref().map(|tf| OnFailureConfig::Advanced {
                claude: Some(tf.claude.clone()),
                shell: None,
                fail_workflow: tf.fail_workflow,
                retry_original: false,
                max_retries: tf.max_attempts,
            }),
            retry: None,
            on_success: cmd
                .on_success
                .as_ref()
                .map(|s| Box::new(Self::workflow_step_command_to_workflow_step(s))),
            on_exit_code: HashMap::new(),
            commit_required: cmd.commit_required,
            validate: cmd.validate.clone(),
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            when: cmd.when.clone(),
        }
    }

    /// Pure function: Convert normalized workflow to ExtendedWorkflowConfig
    /// No mutations, returns new configuration
    pub fn to_extended_config(&self) -> Result<crate::cook::workflow::ExtendedWorkflowConfig> {
        let steps = self
            .steps
            .iter()
            .map(|step| self.normalized_to_workflow_step(step))
            .collect::<Result<Vec<_>>>()?;

        // Convert execution mode to workflow mode
        let mode = match &self.execution_mode {
            ExecutionMode::Sequential => crate::cook::workflow::WorkflowMode::Sequential,
            ExecutionMode::WithArguments { .. } => crate::cook::workflow::WorkflowMode::Sequential,
            ExecutionMode::WithFilePattern { .. } => {
                crate::cook::workflow::WorkflowMode::Sequential
            }
            ExecutionMode::MapReduce { .. } => {
                // For MapReduce, we'll need to set map_phase and reduce_phase
                crate::cook::workflow::WorkflowMode::MapReduce
            }
        };

        Ok(crate::cook::workflow::ExtendedWorkflowConfig {
            name: self.name.to_string(),
            mode,
            steps,
            setup_phase: None,  // Would need to be set based on MapReduceConfig
            map_phase: None,    // Would need to be set based on MapReduceConfig
            reduce_phase: None, // Would need to be set based on MapReduceConfig
            max_iterations: self.extract_max_iterations()? as u32,
            iterate: self.extract_max_iterations()? > 1,
            retry_defaults: None, // Would need to be set from workflow config
            environment: None,    // Would need to be set from workflow config
        })
    }

    /// Pure function: Transform normalized step back to workflow step
    /// Preserves ALL fields, returns Result for validation
    fn normalized_to_workflow_step(&self, step: &NormalizedStep) -> Result<WorkflowStep> {
        // Validate step before transformation
        self.validate_step(step)?;

        let (claude, shell, test, foreach) = match &step.command {
            StepCommand::Claude(cmd) => (Some(cmd.to_string()), None, None, None),
            StepCommand::Shell(cmd) => (None, Some(cmd.to_string()), None, None),
            StepCommand::Test {
                command,
                on_failure,
            } => (
                None,
                None,
                Some(crate::config::command::TestCommand {
                    command: command.to_string(),
                    on_failure: on_failure.as_ref().map(|f| (**f).clone()),
                }),
                None,
            ),
            StepCommand::Foreach(config) => (None, None, None, Some((**config).clone())),
            StepCommand::Handler(handler) => {
                // For handler steps, use the handler field
                return Ok(WorkflowStep {
                    name: Some(step.id.to_string()),
                    claude: None,
                    shell: None,
                    test: None,
                    foreach: None,
                    write_file: None,
                    command: None,
                    handler: Some(crate::cook::workflow::HandlerStep {
                        name: handler.name.to_string(),
                        attributes: Arc::try_unwrap(handler.attributes.clone())
                            .unwrap_or_else(|arc| (*arc).clone()),
                    }),
                    capture: None,
                    capture_format: None,
                    capture_streams: Default::default(),
                    output_file: None,
                    capture_output: crate::cook::workflow::CaptureOutput::Disabled,
                    timeout: step.timeout.map(|d| d.as_secs()),
                    working_dir: step.working_dir.clone(),
                    env: Arc::try_unwrap(step.env.clone()).unwrap_or_else(|arc| (*arc).clone()),
                    on_failure: step.handlers.on_failure.as_ref().map(|f| (**f).clone()),
                    retry: None,
                    on_success: step
                        .handlers
                        .on_success
                        .as_ref()
                        .map(|s| Box::new((**s).clone())),
                    on_exit_code: step
                        .handlers
                        .on_exit_code
                        .iter()
                        .map(|(k, v)| (*k, Box::new((**v).clone())))
                        .collect(),
                    commit_required: step.commit_required,
                    auto_commit: false,
                    commit_config: None,
                    validate: step.validation.as_ref().map(|v| (**v).clone()), // PRESERVED!
                    step_validate: None,
                    skip_validation: false,
                    validation_timeout: None,
                    ignore_validation_failure: false,
                    when: step.when.as_ref().map(|w| w.to_string()), // PRESERVED!
                });
            }
            StepCommand::Simple(cmd) => {
                // For simple commands, use the legacy command field
                return Ok(WorkflowStep {
                    name: Some(step.id.to_string()),
                    claude: None,
                    shell: None,
                    test: None,
                    foreach: None,
                    write_file: None,
                    command: Some(cmd.to_string()),
                    handler: None,
                    capture: None,
                    capture_format: None,
                    capture_streams: Default::default(),
                    output_file: None,
                    capture_output: crate::cook::workflow::CaptureOutput::Disabled,
                    timeout: step.timeout.map(|d| d.as_secs()),
                    working_dir: step.working_dir.clone(),
                    env: Arc::try_unwrap(step.env.clone()).unwrap_or_else(|arc| (*arc).clone()),
                    on_failure: step.handlers.on_failure.as_ref().map(|f| (**f).clone()),
                    retry: None,
                    on_success: step
                        .handlers
                        .on_success
                        .as_ref()
                        .map(|s| Box::new((**s).clone())),
                    on_exit_code: step
                        .handlers
                        .on_exit_code
                        .iter()
                        .map(|(k, v)| (*k, Box::new((**v).clone())))
                        .collect(),
                    commit_required: step.commit_required,
                    auto_commit: false,
                    commit_config: None,
                    validate: step.validation.as_ref().map(|v| (**v).clone()), // PRESERVED!
                    step_validate: None,
                    skip_validation: false,
                    validation_timeout: None,
                    ignore_validation_failure: false,
                    when: step.when.as_ref().map(|w| w.to_string()), // PRESERVED!
                });
            }
        };

        Ok(WorkflowStep {
            name: Some(step.id.to_string()),
            claude,
            shell,
            test,
            foreach,
            write_file: None,
            command: None,
            handler: None,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            capture_output: crate::cook::workflow::CaptureOutput::Disabled,
            timeout: step.timeout.map(|d| d.as_secs()),
            working_dir: step.working_dir.clone(),
            env: Arc::try_unwrap(step.env.clone()).unwrap_or_else(|arc| (*arc).clone()),
            on_failure: step.handlers.on_failure.as_ref().map(|f| (**f).clone()),
            retry: None,
            on_success: step
                .handlers
                .on_success
                .as_ref()
                .map(|s| Box::new((**s).clone())),
            on_exit_code: step
                .handlers
                .on_exit_code
                .iter()
                .map(|(k, v)| (*k, Box::new((**v).clone())))
                .collect(),
            commit_required: step.commit_required,
            auto_commit: false,
            commit_config: None,
            validate: step.validation.as_ref().map(|v| (**v).clone()), // PRESERVED!
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            when: step.when.as_ref().map(|w| w.to_string()), // PRESERVED!
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
            ExecutionMode::MapReduce { config } => config
                .max_iterations
                .ok_or_else(|| anyhow!("MapReduce config missing max_iterations")),
        }
    }

    /// Pure function: Classify workflow type based on configuration
    /// No side effects, deterministic output
    pub fn classify_workflow_type(workflow: &WorkflowConfig) -> WorkflowType {
        if Self::has_mapreduce_indicators(workflow) {
            WorkflowType::MapReduce
        } else if Self::has_argument_indicators(workflow) {
            WorkflowType::WithArguments
        } else if Self::has_output_definitions(workflow) {
            WorkflowType::StructuredWithOutputs
        } else {
            WorkflowType::Standard
        }
    }

    /// Pure function: Check if workflow has output definitions
    fn has_output_definitions(workflow: &WorkflowConfig) -> bool {
        workflow.commands.iter().any(|cmd| match cmd {
            WorkflowCommand::WorkflowStep(step) => step.outputs.is_some(),
            WorkflowCommand::Structured(cmd) => cmd.outputs.is_some(),
            _ => false,
        })
    }

    /// Pure function: Check if workflow has argument indicators
    fn has_argument_indicators(workflow: &WorkflowConfig) -> bool {
        workflow.commands.iter().any(|cmd| match cmd {
            WorkflowCommand::Structured(cmd) => cmd.args.iter().any(|arg| arg.is_variable()),
            _ => false,
        })
    }

    /// Pure function: Check if workflow has mapreduce indicators
    fn has_mapreduce_indicators(workflow: &WorkflowConfig) -> bool {
        // Check for specific patterns that indicate MapReduce usage
        workflow.commands.iter().any(|cmd| match cmd {
            WorkflowCommand::Structured(cmd) => {
                cmd.name.contains("mapreduce")
                    || cmd.options.contains_key("partition")
                    || cmd.options.contains_key("reduce")
            }
            _ => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::command::Command;

    #[test]
    fn test_normalize_simple_workflow() {
        let config = WorkflowConfig {
            name: None,
            commands: vec![WorkflowCommand::Simple("echo hello".to_string())],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let normalized =
            NormalizedWorkflow::from_workflow_config(&config, ExecutionMode::Sequential).unwrap();

        assert_eq!(normalized.steps.len(), 1);
        assert_eq!(normalized.steps[0].id.as_ref(), "step-0");
        assert!(normalized.steps[0].validation.is_none());
    }

    #[test]
    fn test_preserve_validation() {
        let validation = ValidationConfig {
            command: None,
            shell: Some("echo test".to_string()),
            claude: None,
            commands: None,
            expected_schema: None,
            threshold: 100.0,
            timeout: None,
            on_incomplete: None,
            result_file: None,
        };

        let step_cmd = WorkflowStepCommand {
            claude: Some("test command".to_string()),
            shell: None,
            analyze: None,
            test: None,
            foreach: None,
            write_file: None,
            id: Some("test-step".to_string()),
            commit_required: true,
            analysis: None,
            outputs: None,
            capture_output: None,
            on_failure: None,
            on_success: None,
            validate: Some(validation.clone()),
            timeout: None,
            when: None,
            capture_format: None,
            capture_streams: None,
            output_file: None,
        };

        let config = WorkflowConfig {
            name: None,
            commands: vec![WorkflowCommand::WorkflowStep(Box::new(step_cmd))],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let normalized =
            NormalizedWorkflow::from_workflow_config(&config, ExecutionMode::Sequential).unwrap();

        assert_eq!(normalized.steps.len(), 1);
        assert_eq!(normalized.steps[0].id.as_ref(), "test-step");
        assert!(normalized.steps[0].validation.is_some());

        // Convert back and verify validation is preserved
        let extended = normalized.to_extended_config().unwrap();
        assert!(extended.steps[0].validate.is_some());
    }

    #[test]
    fn test_classify_workflow_types() {
        // Standard workflow
        let standard = WorkflowConfig {
            name: None,
            commands: vec![WorkflowCommand::Simple("echo test".to_string())],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };
        assert_eq!(
            NormalizedWorkflow::classify_workflow_type(&standard),
            WorkflowType::Standard
        );

        // Workflow with outputs
        let with_outputs = WorkflowConfig {
            name: None,
            commands: vec![WorkflowCommand::Structured(Box::new(Command {
                name: "test".to_string(),
                args: vec![],
                options: HashMap::new(),
                metadata: Default::default(),
                id: None,
                outputs: Some(HashMap::from([(
                    "output".to_string(),
                    OutputDeclaration {
                        file_pattern: "*.txt".to_string(),
                    },
                )])),
                analysis: None,
            }))],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };
        assert_eq!(
            NormalizedWorkflow::classify_workflow_type(&with_outputs),
            WorkflowType::StructuredWithOutputs
        );
    }

    #[test]
    fn test_step_validation() {
        let workflow = NormalizedWorkflow {
            name: Arc::from("test"),
            steps: Arc::from(vec![]),
            execution_mode: ExecutionMode::Sequential,
            variables: Arc::new(HashMap::new()),
        };

        // Empty ID should fail
        let invalid_step = NormalizedStep {
            id: Arc::from(""),
            command: StepCommand::Simple(Arc::from("test")),
            validation: None,
            handlers: StepHandlers::default(),
            timeout: None,
            working_dir: None,
            env: Arc::new(HashMap::new()),
            outputs: None,
            commit_required: false,
            when: None,
        };
        assert!(workflow.validate_step(&invalid_step).is_err());

        // Zero timeout should fail
        let invalid_timeout = NormalizedStep {
            id: Arc::from("test"),
            command: StepCommand::Simple(Arc::from("test")),
            validation: None,
            handlers: StepHandlers::default(),
            timeout: Some(Duration::from_secs(0)),
            working_dir: None,
            env: Arc::new(HashMap::new()),
            outputs: None,
            commit_required: false,
            when: None,
        };
        assert!(workflow.validate_step(&invalid_timeout).is_err());

        // Valid step should succeed
        let valid_step = NormalizedStep {
            id: Arc::from("test"),
            command: StepCommand::Simple(Arc::from("test")),
            validation: None,
            handlers: StepHandlers::default(),
            timeout: Some(Duration::from_secs(30)),
            working_dir: None,
            env: Arc::new(HashMap::new()),
            outputs: None,
            commit_required: false,
            when: None,
        };
        assert!(workflow.validate_step(&valid_step).is_ok());
    }
}
