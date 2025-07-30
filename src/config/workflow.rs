use super::command::WorkflowCommand;
use serde::{Deserialize, Serialize};

/// Configuration for workflow execution
///
/// Contains a list of commands to execute in sequence for a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// Commands to execute in order
    pub commands: Vec<WorkflowCommand>,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            commands: vec![
                WorkflowCommand::Simple("mmm-code-review".to_string()),
                WorkflowCommand::Simple("mmm-implement-spec".to_string()),
                WorkflowCommand::Simple("mmm-lint".to_string()),
            ],
        }
    }
}
