//! Resource allocation for workflow execution
//!
//! This module provides pure functions for calculating resource requirements
//! based on execution mode and configuration. No I/O operations are performed.

use super::mode_detection::ExecutionMode;
use crate::cook::orchestrator::CookConfig;

/// Memory estimate per parallel task (100MB)
const MEMORY_PER_PARALLEL_TASK: usize = 100_000_000;

/// Memory estimate per iteration (50MB)
const MEMORY_PER_ITERATION: usize = 50_000_000;

/// Disk space estimate per worktree (500MB)
const DISK_SPACE_PER_WORKTREE: usize = 500_000_000;

/// Resource requirements for workflow execution
///
/// Contains estimates for worktrees, memory, disk space, and concurrency limits.
/// These are planning estimates, not hard guarantees.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceRequirements {
    /// Number of worktrees required
    pub worktrees: usize,
    /// Estimated memory usage in bytes
    pub memory_estimate: usize,
    /// Estimated disk space usage in bytes
    pub disk_space: usize,
    /// Maximum number of concurrent commands
    pub max_concurrent_commands: usize,
}

impl ResourceRequirements {
    /// Create minimal resource requirements
    ///
    /// Used for Standard and DryRun modes that don't require
    /// special resource allocation.
    pub fn minimal() -> Self {
        Self {
            worktrees: 0,
            memory_estimate: 0,
            disk_space: 0,
            max_concurrent_commands: 1,
        }
    }

    /// Check if resources are within given limits
    pub fn fits_within(&self, max_worktrees: usize, max_memory: usize, max_disk: usize) -> bool {
        self.worktrees <= max_worktrees
            && self.memory_estimate <= max_memory
            && self.disk_space <= max_disk
    }
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self::minimal()
    }
}

/// Pure: Calculate resource requirements from configuration and mode
///
/// # Arguments
///
/// * `config` - The cook configuration
/// * `mode` - The detected execution mode
///
/// # Returns
///
/// Resource requirements for the execution plan
///
/// # Resource Calculation Rules
///
/// - **MapReduce**: worktrees = max_parallel + 1 (for parent), high memory/disk
/// - **Iterative**: worktrees = 1, memory based on number of iterations
/// - **Standard/DryRun**: minimal resources
pub fn calculate_resources(config: &CookConfig, mode: &ExecutionMode) -> ResourceRequirements {
    match mode {
        ExecutionMode::MapReduce => calculate_mapreduce_resources(config),
        ExecutionMode::Iterative => calculate_iterative_resources(config),
        ExecutionMode::Standard | ExecutionMode::DryRun => ResourceRequirements::minimal(),
    }
}

/// Calculate resources for MapReduce execution
fn calculate_mapreduce_resources(config: &CookConfig) -> ResourceRequirements {
    let max_parallel = get_max_parallel(config);

    ResourceRequirements {
        worktrees: max_parallel + 1, // +1 for parent worktree
        memory_estimate: estimate_memory_for_parallel(max_parallel),
        disk_space: estimate_disk_space_for_worktrees(max_parallel + 1),
        max_concurrent_commands: max_parallel,
    }
}

/// Calculate resources for Iterative execution
fn calculate_iterative_resources(config: &CookConfig) -> ResourceRequirements {
    let iterations = calculate_iteration_count(config);

    ResourceRequirements {
        worktrees: 1, // Single worktree for iterative mode
        memory_estimate: estimate_memory_for_iterations(iterations),
        disk_space: 0, // Iterative mode reuses same worktree
        max_concurrent_commands: 1,
    }
}

/// Get max_parallel value from MapReduce config
///
/// Returns a default of 10 if not specified
fn get_max_parallel(config: &CookConfig) -> usize {
    config
        .mapreduce_config
        .as_ref()
        .map(|mr| {
            // Parse max_parallel as usize, defaulting to 10
            mr.map.max_parallel.parse::<usize>().unwrap_or(10)
        })
        .unwrap_or(10)
}

/// Calculate the number of iterations from config
fn calculate_iteration_count(config: &CookConfig) -> usize {
    let from_args = config.command.args.len();
    let from_map = config.command.map.len();

    // Use args count if provided, otherwise use map count
    // If neither, assume 1 iteration
    if from_args > 0 {
        from_args
    } else if from_map > 0 {
        from_map
    } else {
        1
    }
}

/// Estimate memory requirements for parallel execution
fn estimate_memory_for_parallel(max_parallel: usize) -> usize {
    max_parallel * MEMORY_PER_PARALLEL_TASK
}

/// Estimate memory requirements for iterative execution
fn estimate_memory_for_iterations(iterations: usize) -> usize {
    iterations * MEMORY_PER_ITERATION
}

/// Estimate disk space for worktrees
fn estimate_disk_space_for_worktrees(worktree_count: usize) -> usize {
    worktree_count * DISK_SPACE_PER_WORKTREE
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

    fn create_default_config() -> CookConfig {
        CookConfig {
            command: crate::cook::command::CookCommand {
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

    fn create_mapreduce_config_with_parallel(max_parallel: usize) -> MapReduceWorkflowConfig {
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
                max_parallel: max_parallel.to_string(),
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

    #[test]
    fn test_calculate_mapreduce_resources() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_mapreduce_config_with_parallel(10)));
        let mode = ExecutionMode::MapReduce;

        let resources = calculate_resources(&config, &mode);

        assert_eq!(resources.worktrees, 11); // 10 + 1 parent
        assert_eq!(resources.max_concurrent_commands, 10);
        assert!(resources.memory_estimate > 0);
        assert!(resources.disk_space > 0);
    }

    #[test]
    fn test_calculate_mapreduce_resources_with_5_parallel() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_mapreduce_config_with_parallel(5)));
        let mode = ExecutionMode::MapReduce;

        let resources = calculate_resources(&config, &mode);

        assert_eq!(resources.worktrees, 6); // 5 + 1 parent
        assert_eq!(resources.max_concurrent_commands, 5);
        assert_eq!(resources.memory_estimate, 5 * MEMORY_PER_PARALLEL_TASK);
        assert_eq!(resources.disk_space, 6 * DISK_SPACE_PER_WORKTREE);
    }

    #[test]
    fn test_calculate_iterative_resources_with_args() {
        let mut config = create_default_config();
        config.command.args = vec!["a".into(), "b".into(), "c".into()];
        let mode = ExecutionMode::Iterative;

        let resources = calculate_resources(&config, &mode);

        assert_eq!(resources.worktrees, 1);
        assert_eq!(resources.max_concurrent_commands, 1);
        assert_eq!(resources.memory_estimate, 3 * MEMORY_PER_ITERATION);
        assert_eq!(resources.disk_space, 0);
    }

    #[test]
    fn test_calculate_iterative_resources_with_map() {
        let mut config = create_default_config();
        config.command.map = vec!["*.rs".into(), "*.md".into()];
        let mode = ExecutionMode::Iterative;

        let resources = calculate_resources(&config, &mode);

        assert_eq!(resources.worktrees, 1);
        assert_eq!(resources.memory_estimate, 2 * MEMORY_PER_ITERATION);
    }

    #[test]
    fn test_minimal_resources_for_standard_mode() {
        let config = create_default_config();
        let mode = ExecutionMode::Standard;

        let resources = calculate_resources(&config, &mode);

        assert_eq!(resources, ResourceRequirements::minimal());
    }

    #[test]
    fn test_minimal_resources_for_dryrun_mode() {
        let config = create_default_config();
        let mode = ExecutionMode::DryRun;

        let resources = calculate_resources(&config, &mode);

        assert_eq!(resources, ResourceRequirements::minimal());
    }

    #[test]
    fn test_resource_requirements_minimal() {
        let minimal = ResourceRequirements::minimal();

        assert_eq!(minimal.worktrees, 0);
        assert_eq!(minimal.memory_estimate, 0);
        assert_eq!(minimal.disk_space, 0);
        assert_eq!(minimal.max_concurrent_commands, 1);
    }

    #[test]
    fn test_resource_requirements_default() {
        let default = ResourceRequirements::default();
        assert_eq!(default, ResourceRequirements::minimal());
    }

    #[test]
    fn test_resource_requirements_fits_within() {
        let resources = ResourceRequirements {
            worktrees: 5,
            memory_estimate: 500_000_000,
            disk_space: 2_500_000_000,
            max_concurrent_commands: 5,
        };

        assert!(resources.fits_within(10, 1_000_000_000, 5_000_000_000));
        assert!(!resources.fits_within(3, 1_000_000_000, 5_000_000_000)); // Too few worktrees
        assert!(!resources.fits_within(10, 100_000_000, 5_000_000_000)); // Not enough memory
        assert!(!resources.fits_within(10, 1_000_000_000, 1_000_000_000)); // Not enough disk
    }

    // Property test: resource calculation is deterministic
    #[test]
    fn test_resource_calculation_is_deterministic() {
        let config = create_default_config();
        let mode = ExecutionMode::Standard;

        let resources1 = calculate_resources(&config, &mode);
        let resources2 = calculate_resources(&config, &mode);

        assert_eq!(
            resources1, resources2,
            "Resource calculation must be deterministic"
        );
    }

    // Property test: resource calculation is deterministic with mapreduce
    #[test]
    fn test_resource_calculation_deterministic_mapreduce() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_mapreduce_config_with_parallel(10)));
        let mode = ExecutionMode::MapReduce;

        let resources1 = calculate_resources(&config, &mode);
        let resources2 = calculate_resources(&config, &mode);

        assert_eq!(
            resources1, resources2,
            "Resource calculation must be deterministic"
        );
    }
}
