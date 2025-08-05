use super::command::WorkflowCommand;
use serde::{Deserialize, Deserializer, Serialize};

/// Configuration for workflow execution
///
/// Contains a list of commands to execute in sequence for a workflow
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowConfig {
    /// Commands to execute in order
    pub commands: Vec<WorkflowCommand>,
}

impl<'de> Deserialize<'de> for WorkflowConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum WorkflowConfigHelper {
            // New format: direct array of commands
            Commands(Vec<WorkflowCommand>),
            // Old format: object with commands field
            WithCommandsField { commands: Vec<WorkflowCommand> },
        }

        let helper = WorkflowConfigHelper::deserialize(deserializer)?;
        let commands = match helper {
            WorkflowConfigHelper::Commands(cmds) => cmds,
            WorkflowConfigHelper::WithCommandsField { commands } => commands,
        };

        Ok(WorkflowConfig { commands })
    }
}

// Remove default implementation - workflows must now be explicitly defined
