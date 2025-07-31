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

// Remove default implementation - workflows must now be explicitly defined
