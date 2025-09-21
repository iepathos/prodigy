use super::command::WorkflowCommand;
use super::mapreduce::MergeWorkflow;
use crate::cook::environment::{EnvProfile, SecretValue};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for workflow execution
///
/// Contains a list of commands to execute in sequence for a workflow
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowConfig {
    /// Commands to execute in order
    pub commands: Vec<WorkflowCommand>,

    /// Global environment variables for all commands
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Secret environment variables (masked in logs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, SecretValue>>,

    /// Environment files to load (.env format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_files: Option<Vec<PathBuf>>,

    /// Environment profiles for different contexts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profiles: Option<HashMap<String, EnvProfile>>,

    /// Optional custom merge workflow for worktree integration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge: Option<MergeWorkflow>,
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
            // Full format: object with commands and environment fields
            Full {
                commands: Vec<WorkflowCommand>,
                #[serde(default)]
                env: Option<HashMap<String, String>>,
                #[serde(default)]
                secrets: Option<HashMap<String, SecretValue>>,
                #[serde(default)]
                env_files: Option<Vec<PathBuf>>,
                #[serde(default)]
                profiles: Option<HashMap<String, EnvProfile>>,
                #[serde(default)]
                merge: Option<MergeWorkflow>,
            },
            // Old format: object with commands field only
            WithCommandsField {
                commands: Vec<WorkflowCommand>,
            },
        }

        let helper = WorkflowConfigHelper::deserialize(deserializer)?;
        match helper {
            WorkflowConfigHelper::Commands(cmds) => Ok(WorkflowConfig {
                commands: cmds,
                env: None,
                secrets: None,
                env_files: None,
                profiles: None,
                merge: None,
            }),
            WorkflowConfigHelper::Full {
                commands,
                env,
                secrets,
                env_files,
                profiles,
                merge,
            } => Ok(WorkflowConfig {
                commands,
                env,
                secrets,
                env_files,
                profiles,
                merge,
            }),
            WorkflowConfigHelper::WithCommandsField { commands } => Ok(WorkflowConfig {
                commands,
                env: None,
                secrets: None,
                env_files: None,
                profiles: None,
                merge: None,
            }),
        }
    }
}

// Remove default implementation - workflows must now be explicitly defined
