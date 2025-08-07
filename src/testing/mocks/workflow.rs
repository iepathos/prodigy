//! Mock workflow executor for testing

use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::{
    ExtendedWorkflowConfig, StepResult, WorkflowContext, WorkflowExecutor, WorkflowStep,
};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Mock workflow executor for testing
pub struct MockWorkflowExecutor {
    pub steps_executed: Arc<Mutex<Vec<WorkflowStep>>>,
    pub should_fail: bool,
    pub outputs: HashMap<String, String>,
}

impl Default for MockWorkflowExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl MockWorkflowExecutor {
    /// Create new mock workflow executor
    pub fn new() -> Self {
        Self {
            steps_executed: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
            outputs: HashMap::new(),
        }
    }

    /// Create a failing mock
    pub fn failing() -> Self {
        let mut mock = Self::new();
        mock.should_fail = true;
        mock
    }

    /// Set expected outputs
    pub fn with_outputs(mut self, outputs: HashMap<String, String>) -> Self {
        self.outputs = outputs;
        self
    }

    /// Get executed steps for verification
    pub fn get_executed_steps(&self) -> Vec<WorkflowStep> {
        self.steps_executed.lock().unwrap().clone()
    }

    /// Mock execute_step method
    pub async fn execute_step(
        &self,
        step: &WorkflowStep,
        _working_dir: &Path,
        _env_vars: HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }

        // Record the step
        self.steps_executed.lock().unwrap().push(step.clone());

        // Return configured outputs
        Ok(self.outputs.clone())
    }

    /// Mock execute_workflow method
    pub async fn execute_workflow(
        &self,
        steps: Vec<WorkflowStep>,
        _working_dir: &Path,
        _env_vars: HashMap<String, String>,
    ) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }

        // Record all steps
        for step in steps {
            self.steps_executed.lock().unwrap().push(step);
        }

        Ok(())
    }

    /// Mock validate_step method
    pub fn validate_step(&self, _step: &WorkflowStep) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }
        Ok(())
    }
}

// Implement the WorkflowExecutor trait for MockWorkflowExecutor
#[async_trait]
impl WorkflowExecutor for MockWorkflowExecutor {
    async fn execute(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        _env: &ExecutionEnvironment,
    ) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock workflow execution failed"));
        }

        // Record all steps from the workflow
        for step in &workflow.steps {
            self.steps_executed.lock().unwrap().push(step.clone());
        }

        Ok(())
    }

    async fn execute_step(
        &mut self,
        step: &WorkflowStep,
        _env: &ExecutionEnvironment,
        _context: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Record that this step was executed
        self.steps_executed.lock().unwrap().push(step.clone());

        if self.should_fail {
            return Err(anyhow::anyhow!("Mock step execution failed"));
        }

        Ok(StepResult {
            success: true,
            exit_code: Some(0),
            stdout: self.outputs.get("stdout").cloned().unwrap_or_default(),
            stderr: String::new(),
        })
    }
}
