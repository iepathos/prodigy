//! Execution planning for workflow orchestration
//!
//! This module provides the main planning function that composes mode detection
//! and resource allocation into complete execution plans. All functions are pure
//! and perform no I/O operations.

use super::mode_detection::{detect_execution_mode, ExecutionMode};
use super::resource_allocation::{calculate_resources, ResourceRequirements};
use crate::cook::orchestrator::CookConfig;

/// Complete execution plan for a workflow
///
/// Contains all information needed to execute a workflow:
/// - The detected execution mode
/// - Resource requirements
/// - Execution phases
/// - Parallel execution budget
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    /// The detected execution mode
    pub mode: ExecutionMode,
    /// Resource requirements for execution
    pub resource_needs: ResourceRequirements,
    /// Phases to execute in order
    pub phases: Vec<Phase>,
    /// Maximum parallel operations allowed
    pub parallel_budget: usize,
}

impl ExecutionPlan {
    /// Check if this is a dry run execution
    pub fn is_dry_run(&self) -> bool {
        self.mode == ExecutionMode::DryRun
    }

    /// Check if this plan requires worktrees
    pub fn requires_worktrees(&self) -> bool {
        self.resource_needs.worktrees > 0
    }

    /// Get the number of phases
    pub fn phase_count(&self) -> usize {
        self.phases.len()
    }

    /// Check if plan has a specific phase type
    pub fn has_phase(&self, phase_type: PhaseType) -> bool {
        self.phases.iter().any(|p| p.phase_type() == phase_type)
    }
}

/// Phase types for pattern matching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhaseType {
    /// Setup phase (preparation)
    Setup,
    /// Map phase (parallel processing)
    Map,
    /// Reduce phase (aggregation)
    Reduce,
    /// Commands phase (sequential execution)
    Commands,
    /// Dry run analysis phase
    DryRunAnalysis,
}

/// Execution phases
///
/// Lightweight enum representing what phase to execute.
/// Does not contain the actual phase configuration (which would include I/O-related data).
/// The actual configuration is retrieved from CookConfig during execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// Setup phase - preparation commands before map
    Setup {
        /// Number of commands in setup
        command_count: usize,
        /// Whether timeout is configured
        has_timeout: bool,
    },
    /// Map phase - parallel processing of work items
    Map {
        /// Maximum parallel agents
        max_parallel: usize,
        /// Whether filter is configured
        has_filter: bool,
        /// Whether sorting is configured
        has_sort: bool,
    },
    /// Reduce phase - aggregation after map
    Reduce {
        /// Number of commands in reduce
        command_count: usize,
    },
    /// Commands phase - sequential command execution
    Commands {
        /// Number of commands to execute
        command_count: usize,
    },
    /// Dry run analysis - preview without execution
    DryRunAnalysis,
}

impl Phase {
    /// Get the phase type for pattern matching
    pub fn phase_type(&self) -> PhaseType {
        match self {
            Phase::Setup { .. } => PhaseType::Setup,
            Phase::Map { .. } => PhaseType::Map,
            Phase::Reduce { .. } => PhaseType::Reduce,
            Phase::Commands { .. } => PhaseType::Commands,
            Phase::DryRunAnalysis => PhaseType::DryRunAnalysis,
        }
    }

    /// Get the command count for phases that have commands
    pub fn command_count(&self) -> Option<usize> {
        match self {
            Phase::Setup { command_count, .. } => Some(*command_count),
            Phase::Reduce { command_count } => Some(*command_count),
            Phase::Commands { command_count } => Some(*command_count),
            Phase::Map { .. } => None,
            Phase::DryRunAnalysis => None,
        }
    }
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Phase::Setup { command_count, .. } => {
                write!(f, "Setup ({} commands)", command_count)
            }
            Phase::Map { max_parallel, .. } => {
                write!(f, "Map (max {} parallel)", max_parallel)
            }
            Phase::Reduce { command_count } => {
                write!(f, "Reduce ({} commands)", command_count)
            }
            Phase::Commands { command_count } => {
                write!(f, "Commands ({} commands)", command_count)
            }
            Phase::DryRunAnalysis => write!(f, "DryRunAnalysis"),
        }
    }
}

/// Pure: Plan execution from cook configuration
///
/// This is the main entry point for execution planning. It composes:
/// 1. Mode detection
/// 2. Resource calculation
/// 3. Phase determination
/// 4. Parallel budget computation
///
/// # Arguments
///
/// * `config` - The cook configuration to plan for
///
/// # Returns
///
/// A complete execution plan
///
/// # Example
///
/// ```ignore
/// let config = CookConfig { /* ... */ };
/// let plan = plan_execution(&config);
///
/// match plan.mode {
///     ExecutionMode::MapReduce => {
///         println!("Running MapReduce with {} parallel", plan.parallel_budget);
///     }
///     _ => println!("Running in {} mode", plan.mode),
/// }
/// ```
pub fn plan_execution(config: &CookConfig) -> ExecutionPlan {
    let mode = detect_execution_mode(config);
    let resource_needs = calculate_resources(config, &mode);
    let phases = determine_phases(config, &mode);
    let parallel_budget = compute_parallel_budget(&resource_needs);

    ExecutionPlan {
        mode,
        resource_needs,
        phases,
        parallel_budget,
    }
}

/// Pure: Determine execution phases based on mode and config
fn determine_phases(config: &CookConfig, mode: &ExecutionMode) -> Vec<Phase> {
    match mode {
        ExecutionMode::MapReduce => determine_mapreduce_phases(config),
        ExecutionMode::Standard | ExecutionMode::Iterative => determine_command_phases(config),
        ExecutionMode::DryRun => vec![Phase::DryRunAnalysis],
    }
}

/// Determine phases for MapReduce execution
fn determine_mapreduce_phases(config: &CookConfig) -> Vec<Phase> {
    let mut phases = Vec::new();

    if let Some(mr_config) = &config.mapreduce_config {
        // Setup phase (if present)
        if let Some(setup) = &mr_config.setup {
            phases.push(Phase::Setup {
                command_count: setup.commands.len(),
                has_timeout: setup.timeout.is_some(),
            });
        }

        // Map phase (always present in MapReduce)
        let max_parallel = mr_config.map.max_parallel.parse::<usize>().unwrap_or(10);
        phases.push(Phase::Map {
            max_parallel,
            has_filter: mr_config.map.filter.is_some(),
            has_sort: mr_config.map.sort_by.is_some(),
        });

        // Reduce phase (if present)
        if let Some(reduce) = &mr_config.reduce {
            phases.push(Phase::Reduce {
                command_count: reduce.commands.len(),
            });
        }
    }

    phases
}

/// Determine phases for Standard/Iterative execution
fn determine_command_phases(config: &CookConfig) -> Vec<Phase> {
    let command_count = config.workflow.commands.len();
    vec![Phase::Commands { command_count }]
}

/// Pure: Compute parallel execution budget from resources
fn compute_parallel_budget(resources: &ResourceRequirements) -> usize {
    resources.max_concurrent_commands
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

    fn create_mapreduce_config_with_setup_and_reduce() -> MapReduceWorkflowConfig {
        use crate::config::mapreduce::{ReducePhaseYaml, SetupPhaseConfig};
        use crate::cook::workflow::WorkflowStep;

        fn shell_step(cmd: &str) -> WorkflowStep {
            WorkflowStep {
                shell: Some(cmd.to_string()),
                ..Default::default()
            }
        }

        MapReduceWorkflowConfig {
            name: "test".to_string(),
            mode: "mapreduce".to_string(),
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            setup: Some(SetupPhaseConfig {
                commands: vec![shell_step("echo setup"), shell_step("echo setup2")],
                timeout: Some("300".to_string()),
                capture_outputs: Default::default(),
            }),
            map: MapPhaseYaml {
                input: "items.json".to_string(),
                json_path: "$.items[*]".to_string(),
                agent_template: AgentTemplate { commands: vec![] },
                max_parallel: "10".to_string(),
                filter: Some("status == 'active'".to_string()),
                sort_by: Some("priority DESC".to_string()),
                max_items: None,
                offset: None,
                distinct: None,
                agent_timeout_secs: None,
                timeout_config: None,
            },
            reduce: Some(ReducePhaseYaml {
                commands: vec![shell_step("echo reduce")],
            }),
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
    fn test_plan_execution_mapreduce() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_mapreduce_config_with_parallel(10)));

        let plan = plan_execution(&config);

        assert_eq!(plan.mode, ExecutionMode::MapReduce);
        assert_eq!(plan.parallel_budget, 10);
        assert_eq!(plan.resource_needs.worktrees, 11);
        assert_eq!(plan.phases.len(), 1); // Just Map phase
        assert!(plan.has_phase(PhaseType::Map));
    }

    #[test]
    fn test_plan_execution_mapreduce_with_all_phases() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_mapreduce_config_with_setup_and_reduce()));

        let plan = plan_execution(&config);

        assert_eq!(plan.mode, ExecutionMode::MapReduce);
        assert_eq!(plan.phases.len(), 3); // Setup, Map, Reduce
        assert!(plan.has_phase(PhaseType::Setup));
        assert!(plan.has_phase(PhaseType::Map));
        assert!(plan.has_phase(PhaseType::Reduce));

        // Check Setup phase details
        match &plan.phases[0] {
            Phase::Setup {
                command_count,
                has_timeout,
            } => {
                assert_eq!(*command_count, 2);
                assert!(*has_timeout);
            }
            _ => panic!("Expected Setup phase"),
        }

        // Check Map phase details
        match &plan.phases[1] {
            Phase::Map {
                max_parallel,
                has_filter,
                has_sort,
            } => {
                assert_eq!(*max_parallel, 10);
                assert!(*has_filter);
                assert!(*has_sort);
            }
            _ => panic!("Expected Map phase"),
        }

        // Check Reduce phase details
        match &plan.phases[2] {
            Phase::Reduce { command_count } => {
                assert_eq!(*command_count, 1);
            }
            _ => panic!("Expected Reduce phase"),
        }
    }

    #[test]
    fn test_plan_execution_standard() {
        let config = create_default_config();

        let plan = plan_execution(&config);

        assert_eq!(plan.mode, ExecutionMode::Standard);
        assert_eq!(plan.parallel_budget, 1);
        assert_eq!(plan.phases.len(), 1);
        assert!(plan.has_phase(PhaseType::Commands));
    }

    #[test]
    fn test_plan_execution_dryrun() {
        let mut config = create_default_config();
        config.command.dry_run = true;

        let plan = plan_execution(&config);

        assert_eq!(plan.mode, ExecutionMode::DryRun);
        assert!(plan.is_dry_run());
        assert_eq!(plan.phases.len(), 1);
        assert!(plan.has_phase(PhaseType::DryRunAnalysis));
    }

    #[test]
    fn test_plan_execution_iterative() {
        let mut config = create_default_config();
        config.command.args = vec!["arg1".to_string(), "arg2".to_string()];

        let plan = plan_execution(&config);

        assert_eq!(plan.mode, ExecutionMode::Iterative);
        assert_eq!(plan.parallel_budget, 1);
        assert!(plan.has_phase(PhaseType::Commands));
    }

    #[test]
    fn test_execution_plan_requires_worktrees() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_mapreduce_config_with_parallel(5)));

        let plan = plan_execution(&config);

        assert!(plan.requires_worktrees());

        // Standard mode doesn't require worktrees
        let standard_config = create_default_config();
        let standard_plan = plan_execution(&standard_config);
        assert!(!standard_plan.requires_worktrees());
    }

    #[test]
    fn test_execution_plan_phase_count() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_mapreduce_config_with_setup_and_reduce()));

        let plan = plan_execution(&config);

        assert_eq!(plan.phase_count(), 3);
    }

    #[test]
    fn test_phase_display() {
        assert_eq!(
            format!(
                "{}",
                Phase::Setup {
                    command_count: 2,
                    has_timeout: true
                }
            ),
            "Setup (2 commands)"
        );
        assert_eq!(
            format!(
                "{}",
                Phase::Map {
                    max_parallel: 10,
                    has_filter: false,
                    has_sort: false
                }
            ),
            "Map (max 10 parallel)"
        );
        assert_eq!(
            format!("{}", Phase::Reduce { command_count: 1 }),
            "Reduce (1 commands)"
        );
        assert_eq!(
            format!("{}", Phase::Commands { command_count: 3 }),
            "Commands (3 commands)"
        );
        assert_eq!(format!("{}", Phase::DryRunAnalysis), "DryRunAnalysis");
    }

    #[test]
    fn test_phase_type() {
        assert_eq!(
            Phase::Setup {
                command_count: 0,
                has_timeout: false
            }
            .phase_type(),
            PhaseType::Setup
        );
        assert_eq!(
            Phase::Map {
                max_parallel: 1,
                has_filter: false,
                has_sort: false
            }
            .phase_type(),
            PhaseType::Map
        );
        assert_eq!(
            Phase::Reduce { command_count: 0 }.phase_type(),
            PhaseType::Reduce
        );
        assert_eq!(
            Phase::Commands { command_count: 0 }.phase_type(),
            PhaseType::Commands
        );
        assert_eq!(
            Phase::DryRunAnalysis.phase_type(),
            PhaseType::DryRunAnalysis
        );
    }

    #[test]
    fn test_phase_command_count() {
        assert_eq!(
            Phase::Setup {
                command_count: 2,
                has_timeout: true
            }
            .command_count(),
            Some(2)
        );
        assert_eq!(Phase::Reduce { command_count: 3 }.command_count(), Some(3));
        assert_eq!(
            Phase::Commands { command_count: 5 }.command_count(),
            Some(5)
        );
        assert_eq!(
            Phase::Map {
                max_parallel: 10,
                has_filter: false,
                has_sort: false
            }
            .command_count(),
            None
        );
        assert_eq!(Phase::DryRunAnalysis.command_count(), None);
    }

    // Property tests for determinism
    #[test]
    fn test_planning_is_deterministic() {
        let config = create_default_config();

        let plan1 = plan_execution(&config);
        let plan2 = plan_execution(&config);

        assert_eq!(plan1, plan2, "Planning must be deterministic");
    }

    #[test]
    fn test_planning_deterministic_mapreduce() {
        let mut config = create_default_config();
        config.mapreduce_config = Some(Arc::new(create_mapreduce_config_with_setup_and_reduce()));

        let plan1 = plan_execution(&config);
        let plan2 = plan_execution(&config);

        assert_eq!(plan1, plan2, "Planning must be deterministic");
    }

    #[test]
    fn test_planning_deterministic_dryrun() {
        let mut config = create_default_config();
        config.command.dry_run = true;

        let plan1 = plan_execution(&config);
        let plan2 = plan_execution(&config);

        assert_eq!(plan1, plan2, "Planning must be deterministic");
    }
}
