use super::command::WorkflowCommand;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub commands: Vec<WorkflowCommand>,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
}

fn default_max_iterations() -> u32 {
    10
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            commands: vec![
                WorkflowCommand::Simple("mmm-code-review".to_string()),
                WorkflowCommand::Simple("mmm-implement-spec".to_string()),
                WorkflowCommand::Simple("mmm-lint".to_string()),
            ],
            max_iterations: 10,
        }
    }
}
