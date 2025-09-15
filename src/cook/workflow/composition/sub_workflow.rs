//! Sub-workflow execution support

use super::{ComposableWorkflow, WorkflowComposer};
use crate::cook::workflow::WorkflowContext;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for a sub-workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubWorkflow {
    /// Source path for the sub-workflow
    pub source: PathBuf,

    /// Parameters to pass to the sub-workflow
    #[serde(default)]
    pub parameters: HashMap<String, Value>,

    /// Input variables from parent context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,

    /// Output variables to extract
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<String>>,

    /// Whether to run in parallel with other sub-workflows
    #[serde(default)]
    pub parallel: bool,

    /// Continue parent workflow on sub-workflow failure
    #[serde(default)]
    pub continue_on_error: bool,

    /// Timeout for sub-workflow execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,

    /// Working directory for sub-workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<PathBuf>,
}

/// Result from sub-workflow execution
#[derive(Debug, Clone, Serialize)]
pub struct SubWorkflowResult {
    /// Whether the sub-workflow succeeded
    pub success: bool,

    /// Output variables from the sub-workflow
    pub outputs: HashMap<String, Value>,

    /// Execution duration
    pub duration: Duration,

    /// Error message if failed
    pub error: Option<String>,

    /// Sub-workflow execution logs
    pub logs: Vec<String>,
}

/// Executes sub-workflows within a parent workflow context
pub struct SubWorkflowExecutor {
    composer: Arc<WorkflowComposer>,
}

impl SubWorkflowExecutor {
    /// Create a new sub-workflow executor
    pub fn new(composer: Arc<WorkflowComposer>) -> Self {
        Self { composer }
    }

    /// Execute a sub-workflow within parent context
    pub async fn execute_sub_workflow(
        &self,
        parent_context: &mut WorkflowContext,
        name: &str,
        sub_workflow: &SubWorkflow,
    ) -> Result<SubWorkflowResult> {
        let start_time = std::time::Instant::now();
        let mut logs = Vec::new();

        tracing::info!(
            "Executing sub-workflow '{}' from {:?}",
            name,
            sub_workflow.source
        );
        logs.push(format!(
            "Starting sub-workflow '{}' from {:?}",
            name, sub_workflow.source
        ));

        // Create isolated context for sub-workflow
        let mut sub_context = self.create_sub_context(parent_context, &sub_workflow.inputs)?;

        // Set working directory if specified
        // Note: WorkflowContext doesn't have a working_directory field yet
        // This would need to be added to the WorkflowContext struct
        if let Some(_working_dir) = &sub_workflow.working_dir {
            // sub_context.working_directory = working_dir.clone();
            tracing::debug!("Working directory override not yet implemented");
        }

        // Compose the sub-workflow
        let composed = self
            .composer
            .compose(&sub_workflow.source, sub_workflow.parameters.clone())
            .await
            .with_context(|| format!("Failed to compose sub-workflow '{}'", name))?;

        logs.push(format!(
            "Composed sub-workflow with {} commands",
            composed.workflow.config.commands.len()
        ));

        // Execute with timeout if specified
        let result = if let Some(timeout) = sub_workflow.timeout {
            tokio::time::timeout(
                timeout,
                self.execute_composed(&composed.workflow, &mut sub_context),
            )
            .await
            .map_err(|_| anyhow::anyhow!("Sub-workflow '{}' timed out", name))
            .and_then(|r| r)
        } else {
            self.execute_composed(&composed.workflow, &mut sub_context)
                .await
        };

        let duration = start_time.elapsed();

        match result {
            Ok(_) => {
                logs.push(format!(
                    "Sub-workflow '{}' completed successfully in {:?}",
                    name, duration
                ));

                // Extract outputs
                let outputs = self.extract_outputs(&sub_context, &sub_workflow.outputs)?;

                // Merge outputs back to parent context
                self.merge_outputs(parent_context, &outputs)?;

                Ok(SubWorkflowResult {
                    success: true,
                    outputs,
                    duration,
                    error: None,
                    logs,
                })
            }
            Err(e) => {
                let error_msg = format!("Sub-workflow '{}' failed: {}", name, e);
                logs.push(error_msg.clone());

                if sub_workflow.continue_on_error {
                    tracing::warn!("{} (continuing due to continue_on_error)", error_msg);
                    Ok(SubWorkflowResult {
                        success: false,
                        outputs: HashMap::new(),
                        duration,
                        error: Some(e.to_string()),
                        logs,
                    })
                } else {
                    Err(anyhow::anyhow!(error_msg))
                }
            }
        }
    }

    /// Execute multiple sub-workflows in parallel
    pub async fn execute_parallel_sub_workflows(
        &self,
        parent_context: &mut WorkflowContext,
        sub_workflows: Vec<(&str, &SubWorkflow)>,
    ) -> Result<Vec<(String, SubWorkflowResult)>> {
        let mut handles = Vec::new();

        for (name, sub_workflow) in sub_workflows {
            let name = name.to_string();
            let sub_workflow = sub_workflow.clone();
            let parent_ctx = parent_context.clone();
            let executor = self.clone();

            let handle = tokio::spawn(async move {
                let mut ctx = parent_ctx;
                let result = executor
                    .execute_sub_workflow(&mut ctx, &name, &sub_workflow)
                    .await;
                (name, result)
            });

            handles.push(handle);
        }

        let mut results = Vec::new();

        for handle in handles {
            let (name, result) = handle
                .await
                .map_err(|e| anyhow::anyhow!("Failed to join sub-workflow task: {}", e))?;

            match result {
                Ok(sub_result) => {
                    results.push((name, sub_result));
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Sub-workflow '{}' failed: {}", name, e));
                }
            }
        }

        // Merge all outputs back to parent context
        for (_, result) in &results {
            self.merge_outputs(parent_context, &result.outputs)?;
        }

        Ok(results)
    }

    async fn execute_composed(
        &self,
        workflow: &ComposableWorkflow,
        _context: &mut WorkflowContext,
    ) -> Result<()> {
        // Placeholder for actual execution
        // In a real implementation, this would execute the composed workflow
        tracing::info!(
            "Would execute composed workflow with {} commands",
            workflow.config.commands.len()
        );
        Ok(())
    }

    fn create_sub_context(
        &self,
        parent_context: &WorkflowContext,
        inputs: &Option<HashMap<String, String>>,
    ) -> Result<WorkflowContext> {
        let mut sub_context = parent_context.clone();

        // Clear parent-specific state
        sub_context.variables.clear();

        // Copy specified input variables
        if let Some(inputs) = inputs {
            for (key, var_name) in inputs {
                if let Some(value) = parent_context.variables.get(var_name) {
                    sub_context.variables.insert(key.clone(), value.clone());
                }
            }
        }

        Ok(sub_context)
    }

    fn extract_outputs(
        &self,
        sub_context: &WorkflowContext,
        outputs: &Option<Vec<String>>,
    ) -> Result<HashMap<String, Value>> {
        let mut result = HashMap::new();

        if let Some(output_names) = outputs {
            for name in output_names {
                if let Some(value) = sub_context.variables.get(name) {
                    // Convert to JSON Value
                    let json_value = serde_json::to_value(value)
                        .with_context(|| format!("Failed to serialize output '{}'", name))?;
                    result.insert(name.clone(), json_value);
                }
            }
        }

        Ok(result)
    }

    fn merge_outputs(
        &self,
        parent_context: &mut WorkflowContext,
        outputs: &HashMap<String, Value>,
    ) -> Result<()> {
        for (key, value) in outputs {
            // Convert JSON Value back to string for variable storage
            let str_value = match value {
                Value::String(s) => s.clone(),
                _ => value.to_string(),
            };

            parent_context.variables.insert(key.clone(), str_value);
        }

        Ok(())
    }
}

impl Clone for SubWorkflowExecutor {
    fn clone(&self) -> Self {
        Self {
            composer: Arc::clone(&self.composer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_workflow_configuration() {
        let sub = SubWorkflow {
            source: PathBuf::from("test.yml"),
            parameters: HashMap::new(),
            inputs: Some(HashMap::from([("input1".to_string(), "var1".to_string())])),
            outputs: Some(vec!["result".to_string()]),
            parallel: false,
            continue_on_error: false,
            timeout: Some(Duration::from_secs(60)),
            working_dir: None,
        };

        assert_eq!(sub.source, PathBuf::from("test.yml"));
        assert!(sub.inputs.is_some());
        assert!(sub.outputs.is_some());
    }

    #[test]
    fn test_sub_workflow_result() {
        let result = SubWorkflowResult {
            success: true,
            outputs: HashMap::from([("key".to_string(), Value::String("value".to_string()))]),
            duration: Duration::from_secs(10),
            error: None,
            logs: vec!["Log entry".to_string()],
        };

        assert!(result.success);
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.duration, Duration::from_secs(10));
    }
}
