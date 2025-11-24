//! Mode detection for workflow execution
//!
//! This module provides pure functions for detecting the execution mode
//! from workflow configuration. No I/O operations are performed.

use crate::cook::command::CookCommand;
use crate::cook::orchestrator::CookConfig;

/// Execution modes for workflow processing
///
/// Each mode determines how the workflow will be executed:
/// - Standard: Sequential command execution
/// - MapReduce: Parallel execution across work items
/// - Iterative: Sequential execution with arguments
/// - DryRun: Preview without actual execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ExecutionMode {
    /// Standard sequential execution
    #[default]
    Standard,
    /// MapReduce parallel execution with work items
    MapReduce,
    /// Iterative execution with arguments or file mappings
    Iterative,
    /// Dry run mode - preview without execution
    DryRun,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Standard => write!(f, "Standard"),
            ExecutionMode::MapReduce => write!(f, "MapReduce"),
            ExecutionMode::Iterative => write!(f, "Iterative"),
            ExecutionMode::DryRun => write!(f, "DryRun"),
        }
    }
}

/// Pure: Detect execution mode from cook configuration
///
/// Mode priority (highest to lowest):
/// 1. DryRun - if dry_run flag is set
/// 2. MapReduce - if mapreduce_config is present
/// 3. Iterative - if args or map patterns are provided
/// 4. Standard - default mode
///
/// # Arguments
///
/// * `config` - The cook configuration to analyze
///
/// # Returns
///
/// The detected execution mode
///
/// # Example
///
/// ```ignore
/// let config = CookConfig { /* ... */ };
/// let mode = detect_execution_mode(&config);
/// assert_eq!(mode, ExecutionMode::Standard);
/// ```
pub fn detect_execution_mode(config: &CookConfig) -> ExecutionMode {
    if config.command.dry_run {
        ExecutionMode::DryRun
    } else if config.mapreduce_config.is_some() {
        ExecutionMode::MapReduce
    } else if has_iteration_arguments(&config.command) {
        ExecutionMode::Iterative
    } else {
        ExecutionMode::Standard
    }
}

/// Pure: Check if the command has iteration arguments
///
/// Returns true if either args or map patterns are provided
fn has_iteration_arguments(command: &CookCommand) -> bool {
    !command.args.is_empty() || !command.map.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::mapreduce::{AgentTemplate, MapPhaseYaml, MapReduceWorkflowConfig};
    use crate::config::WorkflowConfig;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn create_default_workflow_config() -> WorkflowConfig {
        WorkflowConfig {
            name: None,
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        }
    }

    fn create_default_mapreduce_config() -> MapReduceWorkflowConfig {
        MapReduceWorkflowConfig {
            name: "test".to_string(),
            mode: "mapreduce".to_string(),
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            setup: None,
            map: MapPhaseYaml {
                input: "items.json".to_string(),
                json_path: "$.items[*]".to_string(),
                agent_template: AgentTemplate { commands: vec![] },
                max_parallel: "10".to_string(),
                filter: None,
                sort_by: None,
                max_items: None,
                offset: None,
                distinct: None,
                agent_timeout_secs: None,
                timeout_config: None,
            },
            reduce: None,
            error_policy: Default::default(),
            on_item_failure: None,
            continue_on_failure: None,
            max_failures: None,
            failure_threshold: None,
            error_collection: None,
            merge: None,
        }
    }

    fn create_default_config() -> CookConfig {
        CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("workflow.yml"),
                path: None,
                max_iterations: 1,
                map: vec![],
                args: vec![],
                fail_fast: false,
                auto_accept: false,
                resume: None,
                verbosity: 0,
                quiet: false,
                dry_run: false,
                params: Default::default(),
            },
            project_path: Arc::new(PathBuf::from(".")),
            workflow: Arc::new(create_default_workflow_config()),
            mapreduce_config: None,
        }
    }

    #[test]
    fn test_detect_dry_run_mode() {
        let mut config = create_default_config();
        config.command.dry_run = true;

        assert_eq!(detect_execution_mode(&config), ExecutionMode::DryRun);
    }

    #[test]
    fn test_detect_dry_run_takes_priority_over_mapreduce() {
        let mut config = create_default_config();
        config.command.dry_run = true;
        config.mapreduce_config = Some(Arc::new(create_default_mapreduce_config()));

        // DryRun should take priority
        assert_eq!(detect_execution_mode(&config), ExecutionMode::DryRun);
    }

    #[test]
    fn test_detect_mapreduce_mode() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_default_mapreduce_config()));

        assert_eq!(detect_execution_mode(&config), ExecutionMode::MapReduce);
    }

    #[test]
    fn test_detect_iterative_mode_with_args() {
        let mut config = create_default_config();
        config.command.args = vec!["arg1".to_string(), "arg2".to_string()];

        assert_eq!(detect_execution_mode(&config), ExecutionMode::Iterative);
    }

    #[test]
    fn test_detect_iterative_mode_with_map() {
        let mut config = create_default_config();
        config.command.map = vec!["src/**/*.rs".to_string()];

        assert_eq!(detect_execution_mode(&config), ExecutionMode::Iterative);
    }

    #[test]
    fn test_detect_standard_mode() {
        let config = create_default_config();
        assert_eq!(detect_execution_mode(&config), ExecutionMode::Standard);
    }

    #[test]
    fn test_mapreduce_takes_priority_over_iterative() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_default_mapreduce_config()));
        config.command.args = vec!["arg1".to_string()];

        // MapReduce should take priority over Iterative
        assert_eq!(detect_execution_mode(&config), ExecutionMode::MapReduce);
    }

    #[test]
    fn test_execution_mode_display() {
        assert_eq!(format!("{}", ExecutionMode::Standard), "Standard");
        assert_eq!(format!("{}", ExecutionMode::MapReduce), "MapReduce");
        assert_eq!(format!("{}", ExecutionMode::Iterative), "Iterative");
        assert_eq!(format!("{}", ExecutionMode::DryRun), "DryRun");
    }

    #[test]
    fn test_execution_mode_default() {
        assert_eq!(ExecutionMode::default(), ExecutionMode::Standard);
    }

    // Property test: mode detection is deterministic
    #[test]
    fn test_mode_detection_is_deterministic() {
        let config = create_default_config();

        let mode1 = detect_execution_mode(&config);
        let mode2 = detect_execution_mode(&config);

        assert_eq!(mode1, mode2, "Mode detection must be deterministic");
    }

    // Property test: mode detection is deterministic with mapreduce
    #[test]
    fn test_mode_detection_deterministic_mapreduce() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_default_mapreduce_config()));

        let mode1 = detect_execution_mode(&config);
        let mode2 = detect_execution_mode(&config);

        assert_eq!(mode1, mode2, "Mode detection must be deterministic");
    }
}
