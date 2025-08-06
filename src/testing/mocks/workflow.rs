//! Mock workflow executor for testing

use crate::cook::workflow::WorkflowStep;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Mock workflow executor for testing - since WorkflowExecutor is a struct not a trait,
/// we create a simple mock struct to use in tests
pub struct MockWorkflowExecutor {
    pub steps_executed: Arc<Mutex<Vec<WorkflowStep>>>,
    pub should_fail: bool,
    pub outputs: HashMap<String, String>,
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
