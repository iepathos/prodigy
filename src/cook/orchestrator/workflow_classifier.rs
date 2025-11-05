//! Workflow classification and normalization
//!
//! Pure functions for classifying workflow types and converting between formats.
//! These functions have no side effects and can be easily tested.

use crate::config::command::WorkflowCommand;

use super::core::{CookConfig, WorkflowType};

/// Classify the workflow type based on configuration
pub(crate) fn classify_workflow_type(config: &CookConfig) -> WorkflowType {
    // MapReduce takes precedence
    if config.mapreduce_config.is_some() {
        return WorkflowType::MapReduce;
    }

    // Check for structured commands with outputs
    let has_structured_outputs = config.workflow.commands.iter().any(|cmd| {
        matches!(cmd, WorkflowCommand::Structured(c)
            if c.outputs.is_some())
    });

    if has_structured_outputs {
        return WorkflowType::StructuredWithOutputs;
    }

    // Check for args or map parameters
    if !config.command.args.is_empty() || !config.command.map.is_empty() {
        return WorkflowType::WithArguments;
    }

    WorkflowType::Standard
}

/// Determine if a command requires a commit
pub fn determine_commit_required(
    cmd: &WorkflowCommand,
    command: &crate::config::command::Command,
) -> bool {
    match cmd {
        WorkflowCommand::SimpleObject(simple) => {
            // If explicitly set in YAML, use that value
            if let Some(cr) = simple.commit_required {
                cr
            } else if crate::config::command_validator::COMMAND_REGISTRY
                .get(&command.name)
                .is_some()
            {
                // Command is in registry, use its configured default
                command.metadata.commit_required
            } else {
                // Command not in registry, use WorkflowStep's default
                true
            }
        }
        WorkflowCommand::Structured(_) => {
            // Structured commands already have metadata
            command.metadata.commit_required
        }
        _ => {
            // For string commands, check registry or use WorkflowStep default
            if crate::config::command_validator::COMMAND_REGISTRY
                .get(&command.name)
                .is_some()
            {
                command.metadata.commit_required
            } else {
                true
            }
        }
    }
}

/// Simple glob pattern matching for file filtering
pub fn matches_glob_pattern(file: &str, pattern: &str) -> bool {
    // Simple glob matching for common cases
    if pattern == "*" {
        return true;
    }

    // Handle patterns like "*.md" or "*test*"
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 2 {
        let prefix = parts[0];
        let suffix = parts[1];
        let filename = file.split('/').next_back().unwrap_or(file);
        return filename.starts_with(prefix) && filename.ends_with(suffix);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkflowConfig;
    use crate::cook::command::CookCommand;
    use crate::cook::orchestrator::core::CookConfig;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn create_test_config(commands: Vec<WorkflowCommand>) -> CookConfig {
        CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("test.yaml"),
                path: None,
                max_iterations: 1,
                map: vec![],
                args: vec![],
                fail_fast: false,
                auto_accept: false,
                metrics: false,
                resume: None,
                verbosity: 0,
                quiet: false,
                dry_run: false,
                params: std::collections::HashMap::new(),
            },
            project_path: Arc::new(PathBuf::from("/test")),
            workflow: Arc::new(WorkflowConfig {
                name: None,
                commands,
                env: None,
                secrets: None,
                env_files: None,
                profiles: None,
                merge: None,
            }),
            mapreduce_config: None,
        }
    }

    #[test]
    fn test_classify_workflow_standard() {
        let config = create_test_config(vec![WorkflowCommand::Simple("/test".to_string())]);
        assert_eq!(classify_workflow_type(&config), WorkflowType::Standard);
    }

    #[test]
    fn test_classify_workflow_with_arguments() {
        let mut config = create_test_config(vec![WorkflowCommand::Simple("/test".to_string())]);
        config.command.args = vec!["arg1".to_string()];
        assert_eq!(classify_workflow_type(&config), WorkflowType::WithArguments);
    }

    #[test]
    fn test_classify_workflow_with_map_patterns() {
        let mut config = create_test_config(vec![WorkflowCommand::Simple("/test".to_string())]);
        config.command.map = vec!["*.md".to_string()];
        assert_eq!(classify_workflow_type(&config), WorkflowType::WithArguments);
    }

    #[test]
    fn test_matches_glob_pattern_wildcard() {
        assert!(matches_glob_pattern("test.md", "*"));
        assert!(matches_glob_pattern("test.rs", "*"));
    }

    #[test]
    fn test_matches_glob_pattern_extension() {
        assert!(matches_glob_pattern("test.md", "*.md"));
        assert!(!matches_glob_pattern("test.rs", "*.md"));
    }

    #[test]
    fn test_matches_glob_pattern_prefix() {
        assert!(matches_glob_pattern("test_file.md", "test*"));
        assert!(!matches_glob_pattern("other.md", "test*"));
    }
}
