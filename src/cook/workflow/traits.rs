//! Trait definitions for workflow execution

use crate::cook::orchestrator::ExecutionEnvironment;
use anyhow::Result;
use async_trait::async_trait;

use super::{ExtendedWorkflowConfig, StepResult, WorkflowContext, WorkflowStep};

/// Trait for workflow execution
#[async_trait]
pub trait WorkflowExecutor: Send + Sync {
    /// Execute a complete workflow with iterations
    async fn execute(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()>;

    /// Execute a single workflow step
    async fn execute_step(
        &mut self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        context: &mut WorkflowContext,
    ) -> Result<StepResult>;
}
