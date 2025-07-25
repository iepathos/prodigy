use anyhow::{Context as AnyhowContext, Result};
use async_trait::async_trait;
use futures::future::{join_all, try_join_all};
use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, Instant};
use tokio::time::timeout;

use super::{
    FailureStrategy, Stage, StageResult, Step, StepResult, StepType, Workflow, WorkflowContext,
    WorkflowResult, WorkflowStatus,
};

#[async_trait]
pub trait WorkflowExecutor: Send + Sync {
    async fn execute(
        &self,
        workflow: &Workflow,
        context: &mut WorkflowContext,
    ) -> Result<WorkflowResult>;
    async fn execute_stage(
        &self,
        stage: &Stage,
        context: &mut WorkflowContext,
    ) -> Result<StageResult>;
    async fn execute_step(&self, step: &Step, context: &mut WorkflowContext) -> Result<StepResult>;
}

pub struct SequentialExecutor {
    condition_evaluator: super::condition::ConditionEvaluator,
}

impl SequentialExecutor {
    pub fn new() -> Self {
        Self {
            condition_evaluator: super::condition::ConditionEvaluator::new(),
        }
    }
}

#[async_trait]
impl WorkflowExecutor for SequentialExecutor {
    async fn execute(
        &self,
        workflow: &Workflow,
        context: &mut WorkflowContext,
    ) -> Result<WorkflowResult> {
        let start_time = Instant::now();
        let mut outputs = HashMap::new();
        let mut last_error = None;

        for stage in &workflow.stages {
            if let Some(condition) = &stage.condition {
                if !self.condition_evaluator.evaluate(condition, context)? {
                    continue;
                }
            }

            match self.execute_stage(stage, context).await {
                Ok(result) => {
                    outputs.extend(result.outputs);
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    break;
                }
            }
        }

        let status = if last_error.is_some() {
            WorkflowStatus::Failed
        } else {
            WorkflowStatus::Completed
        };

        Ok(WorkflowResult {
            status,
            outputs,
            duration: start_time.elapsed(),
            error: last_error,
        })
    }

    async fn execute_stage(
        &self,
        stage: &Stage,
        context: &mut WorkflowContext,
    ) -> Result<StageResult> {
        let mut outputs = HashMap::new();
        let total_steps = stage.steps.len();
        let mut completed_steps = 0;
        let mut last_error = None;

        if stage.parallel.unwrap_or(false) {
            // For parallel execution, execute steps sequentially for now
            // TODO: Implement proper parallel execution with shared context
            for step in &stage.steps {
                match self.execute_step(step, context).await {
                    Ok(step_result) => {
                        outputs.extend(step_result.outputs.clone());
                        completed_steps += 1;
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                    }
                }
            }
        } else {
            for step in &stage.steps {
                match self.execute_step(step, context).await {
                    Ok(result) => {
                        completed_steps += 1;

                        for (key, value) in &result.outputs {
                            context.outputs.insert(key.clone(), value.clone());
                        }
                        outputs.extend(result.outputs);
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                        break;
                    }
                }
            }
        }

        let status = if last_error.is_some() {
            WorkflowStatus::Failed
        } else {
            WorkflowStatus::Completed
        };

        Ok(StageResult {
            name: stage.name.clone(),
            status,
            outputs,
            steps_completed: completed_steps,
            steps_total: total_steps,
        })
    }

    async fn execute_step(&self, step: &Step, context: &mut WorkflowContext) -> Result<StepResult> {
        if let Some(condition) = &step.condition {
            if !self.condition_evaluator.evaluate(condition, context)? {
                return Ok(StepResult {
                    name: step.name.clone(),
                    status: WorkflowStatus::Completed,
                    outputs: HashMap::new(),
                    error: None,
                    retry_count: 0,
                });
            }
        }

        let mut retry_count = 0;
        let max_retries = step.max_retries.unwrap_or(0);
        let mut last_error = None;

        loop {
            match self.execute_step_once(step, context).await {
                Ok(mut result) => {
                    result.retry_count = retry_count;
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e);

                    if retry_count >= max_retries {
                        break;
                    }

                    if let Some(FailureStrategy::Simple(strategy)) = &step.on_failure {
                        if strategy == "retry" {
                            retry_count += 1;
                            tokio::time::sleep(Duration::from_secs(2_u64.pow(retry_count))).await;
                            continue;
                        }
                    }

                    break;
                }
            }
        }

        Ok(StepResult {
            name: step.name.clone(),
            status: WorkflowStatus::Failed,
            outputs: HashMap::new(),
            error: last_error.map(|e| e.to_string()),
            retry_count,
        })
    }
}

impl SequentialExecutor {
    async fn execute_step_once(
        &self,
        step: &Step,
        context: &mut WorkflowContext,
    ) -> Result<StepResult> {
        let step_type = step.step_type.as_ref().unwrap_or(&StepType::Command);

        match step_type {
            StepType::Command => {
                if let Some(command) = &step.command {
                    let expanded_command = self.expand_command(command, context)?;
                    let output = self.execute_command(&expanded_command).await?;

                    let mut outputs = HashMap::new();
                    if let Some(output_names) = &step.outputs {
                        for name in output_names {
                            outputs.insert(name.clone(), serde_json::Value::String(output.clone()));
                        }
                    }

                    Ok(StepResult {
                        name: step.name.clone(),
                        status: WorkflowStatus::Completed,
                        outputs,
                        error: None,
                        retry_count: 0,
                    })
                } else {
                    anyhow::bail!("Command step must have a command");
                }
            }
            StepType::Checkpoint => Ok(StepResult {
                name: step.name.clone(),
                status: WorkflowStatus::WaitingForCheckpoint,
                outputs: HashMap::new(),
                error: None,
                retry_count: 0,
            }),
            _ => anyhow::bail!("Unsupported step type: {:?}", step_type),
        }
    }

    fn expand_command(&self, command: &str, context: &WorkflowContext) -> Result<String> {
        let mut template_engine = tera::Tera::default();
        let mut tera_context = tera::Context::new();

        tera_context.insert("parameters", &context.parameters);
        tera_context.insert("variables", &context.variables);
        tera_context.insert("outputs", &context.outputs);
        tera_context.insert("project", &context.project);

        template_engine
            .render_str(command, &tera_context)
            .context("Failed to expand command template")
    }

    async fn execute_command(&self, command: &str) -> Result<String> {
        let command = command.to_string();
        tokio::task::spawn_blocking(move || {
            let output = Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output()
                .context("Failed to execute command")?;

            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                anyhow::bail!(
                    "Command failed with status {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                )
            }
        })
        .await
        .context("Failed to spawn command task")?
    }
}

pub struct ParallelExecutor {
    sequential_executor: SequentialExecutor,
}

impl ParallelExecutor {
    pub fn new() -> Self {
        Self {
            sequential_executor: SequentialExecutor::new(),
        }
    }
}

#[async_trait]
impl WorkflowExecutor for ParallelExecutor {
    async fn execute(
        &self,
        workflow: &Workflow,
        context: &mut WorkflowContext,
    ) -> Result<WorkflowResult> {
        self.sequential_executor.execute(workflow, context).await
    }

    async fn execute_stage(
        &self,
        stage: &Stage,
        context: &mut WorkflowContext,
    ) -> Result<StageResult> {
        let mut stage_with_parallel = stage.clone();
        stage_with_parallel.parallel = Some(true);
        self.sequential_executor
            .execute_stage(&stage_with_parallel, context)
            .await
    }

    async fn execute_step(&self, step: &Step, context: &mut WorkflowContext) -> Result<StepResult> {
        self.sequential_executor.execute_step(step, context).await
    }
}
