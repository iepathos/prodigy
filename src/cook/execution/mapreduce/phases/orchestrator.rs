//! Pure planning logic for MapReduce phase execution
//!
//! This module contains pure functions for planning and validating MapReduce
//! workflow execution without any I/O operations. All functions here are
//! deterministic and side-effect free, making them easy to test and reason about.

use crate::cook::execution::mapreduce::types::{MapPhase, ReducePhase, SetupPhase};
use anyhow::{anyhow, Result};

/// Execution plan for a MapReduce workflow
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    /// Ordered sequence of phases to execute
    pub phases: Vec<PhaseSpec>,
    /// Recommended parallelism level
    pub parallelism: usize,
    /// Estimated resource requirements
    pub resource_requirements: ResourceEstimate,
}

/// Specification for a phase in the execution plan
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseSpec {
    /// Setup phase for environment preparation
    Setup,
    /// Map phase for parallel processing
    Map,
    /// Reduce phase for result aggregation
    Reduce,
}

/// Estimated resource requirements for workflow execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceEstimate {
    /// Maximum concurrent agents
    pub max_agents: usize,
    /// Estimated memory per agent (MB)
    pub memory_per_agent_mb: usize,
    /// Estimated total memory (MB)
    pub total_memory_mb: usize,
}

/// Determine the execution order of phases based on workflow configuration
///
/// This is a pure function that analyzes the workflow structure and returns
/// an ordered plan of phases to execute.
///
/// # Arguments
///
/// * `has_setup` - Whether the workflow defines a setup phase
/// * `has_reduce` - Whether the workflow defines a reduce phase
///
/// # Returns
///
/// An `ExecutionPlan` containing the phase sequence and resource estimates
///
/// # Examples
///
/// ```
/// use prodigy::cook::execution::mapreduce::phases::orchestrator::plan_phases;
///
/// // Workflow with all phases
/// let plan = plan_phases(true, true);
/// assert_eq!(plan.phases.len(), 3); // Setup, Map, Reduce
///
/// // Workflow with only map phase
/// let plan = plan_phases(false, false);
/// assert_eq!(plan.phases.len(), 1); // Map only
/// ```
pub fn plan_phases(has_setup: bool, has_reduce: bool) -> ExecutionPlan {
    let phases = build_phase_sequence(has_setup, has_reduce);
    let parallelism = calculate_default_parallelism();
    let resource_requirements = estimate_resources(parallelism);

    ExecutionPlan {
        phases,
        parallelism,
        resource_requirements,
    }
}

/// Build the sequence of phases to execute
///
/// This is a pure function that constructs the phase ordering based on
/// which phases are present in the workflow.
///
/// # Phase Ordering Rules
///
/// 1. Setup phase executes first (if present)
/// 2. Map phase always executes
/// 3. Reduce phase executes last (if present)
///
/// # Arguments
///
/// * `has_setup` - Whether to include the setup phase
/// * `has_reduce` - Whether to include the reduce phase
///
/// # Returns
///
/// A `Vec<PhaseSpec>` containing the ordered phases
fn build_phase_sequence(has_setup: bool, has_reduce: bool) -> Vec<PhaseSpec> {
    let mut phases = Vec::with_capacity(3);

    if has_setup {
        phases.push(PhaseSpec::Setup);
    }

    // Map phase is always required
    phases.push(PhaseSpec::Map);

    if has_reduce {
        phases.push(PhaseSpec::Reduce);
    }

    phases
}

/// Calculate the default parallelism level for workflow execution
///
/// This is a pure function that determines the recommended parallelism
/// based on system capabilities.
///
/// # Returns
///
/// The recommended number of parallel agents
fn calculate_default_parallelism() -> usize {
    // Use number of CPU cores as a sensible default
    // In production, this might be configurable or environment-dependent
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

/// Estimate resource requirements for workflow execution
///
/// This is a pure function that calculates resource needs based on
/// parallelism level.
///
/// # Arguments
///
/// * `parallelism` - Number of parallel agents
///
/// # Returns
///
/// A `ResourceEstimate` with memory and resource projections
fn estimate_resources(parallelism: usize) -> ResourceEstimate {
    // Conservative estimate: 512MB per agent
    const MEMORY_PER_AGENT_MB: usize = 512;

    ResourceEstimate {
        max_agents: parallelism,
        memory_per_agent_mb: MEMORY_PER_AGENT_MB,
        total_memory_mb: parallelism * MEMORY_PER_AGENT_MB,
    }
}

/// Validate the MapReduce workflow phases
///
/// This is a pure function that checks if the phase configuration is valid
/// and internally consistent.
///
/// # Arguments
///
/// * `setup` - Optional setup phase configuration
/// * `map` - Map phase configuration (required)
/// * `reduce` - Optional reduce phase configuration
///
/// # Returns
///
/// `Ok(())` if configuration is valid, otherwise an error describing the problem
pub fn validate_phase_config(
    setup: Option<&SetupPhase>,
    map: &MapPhase,
    reduce: Option<&ReducePhase>,
) -> Result<()> {
    // Validate map phase (required)
    if map.agent_template.is_empty() {
        return Err(anyhow!("Map phase requires at least one command"));
    }

    // Validate input source
    if map.config.input.trim().is_empty() {
        return Err(anyhow!("Map phase requires an input source"));
    }

    // Validate setup if present
    if let Some(setup_phase) = setup {
        if setup_phase.commands.is_empty() {
            return Err(anyhow!("Setup phase requires at least one command"));
        }
    }

    // Validate reduce if present
    if let Some(reduce_phase) = reduce {
        if reduce_phase.commands.is_empty() {
            return Err(anyhow!("Reduce phase requires at least one command"));
        }
    }

    // Validate parallelism constraints
    if map.config.max_parallel == 0 {
        return Err(anyhow!("Map phase parallelism must be greater than 0"));
    }

    Ok(())
}

/// Calculate the optimal parallelism based on configuration and system resources
///
/// This is a pure function that determines the best parallelism level
/// considering both user preferences and system constraints.
///
/// # Arguments
///
/// * `requested_parallelism` - User-requested parallelism level
/// * `work_item_count` - Number of work items to process
///
/// # Returns
///
/// The optimal parallelism level
///
/// # Examples
///
/// ```
/// use prodigy::cook::execution::mapreduce::phases::orchestrator::calculate_optimal_parallelism;
///
/// // With few items, parallelism is capped by item count
/// assert_eq!(calculate_optimal_parallelism(10, 3), 3);
///
/// // With many items, use requested parallelism
/// assert_eq!(calculate_optimal_parallelism(10, 100), 10);
/// ```
pub fn calculate_optimal_parallelism(
    requested_parallelism: usize,
    work_item_count: usize,
) -> usize {
    // Never use more agents than work items
    requested_parallelism.min(work_item_count).max(1)
}

/// Check if a phase should be skipped based on workflow state
///
/// This is a pure function that determines whether a phase can be safely
/// skipped without affecting workflow correctness.
///
/// # Arguments
///
/// * `phase` - The phase to check
/// * `has_setup_commands` - Whether setup commands are defined
/// * `has_reduce_commands` - Whether reduce commands are defined
/// * `has_map_results` - Whether the map phase produced results
///
/// # Returns
///
/// `true` if the phase should be skipped, `false` otherwise
pub fn should_skip_phase(
    phase: PhaseSpec,
    has_setup_commands: bool,
    has_reduce_commands: bool,
    has_map_results: bool,
) -> bool {
    match phase {
        PhaseSpec::Setup => !has_setup_commands,
        PhaseSpec::Map => false, // Map phase never skipped
        PhaseSpec::Reduce => !has_reduce_commands || !has_map_results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_phase_sequence_all_phases() {
        let phases = build_phase_sequence(true, true);
        assert_eq!(
            phases,
            vec![PhaseSpec::Setup, PhaseSpec::Map, PhaseSpec::Reduce]
        );
    }

    #[test]
    fn test_build_phase_sequence_map_only() {
        let phases = build_phase_sequence(false, false);
        assert_eq!(phases, vec![PhaseSpec::Map]);
    }

    #[test]
    fn test_build_phase_sequence_setup_and_map() {
        let phases = build_phase_sequence(true, false);
        assert_eq!(phases, vec![PhaseSpec::Setup, PhaseSpec::Map]);
    }

    #[test]
    fn test_build_phase_sequence_map_and_reduce() {
        let phases = build_phase_sequence(false, true);
        assert_eq!(phases, vec![PhaseSpec::Map, PhaseSpec::Reduce]);
    }

    #[test]
    fn test_plan_phases_returns_expected_structure() {
        let plan = plan_phases(true, true);
        assert_eq!(plan.phases.len(), 3);
        assert!(plan.parallelism > 0);
        assert!(plan.resource_requirements.max_agents > 0);
    }

    #[test]
    fn test_calculate_optimal_parallelism_respects_item_count() {
        assert_eq!(calculate_optimal_parallelism(10, 5), 5);
        assert_eq!(calculate_optimal_parallelism(10, 20), 10);
    }

    #[test]
    fn test_calculate_optimal_parallelism_minimum_one() {
        assert_eq!(calculate_optimal_parallelism(0, 10), 1);
        assert_eq!(calculate_optimal_parallelism(10, 0), 1);
    }

    #[test]
    fn test_should_skip_phase_setup() {
        assert!(should_skip_phase(PhaseSpec::Setup, false, true, true));
        assert!(!should_skip_phase(PhaseSpec::Setup, true, true, true));
    }

    #[test]
    fn test_should_skip_phase_map_never() {
        assert!(!should_skip_phase(PhaseSpec::Map, false, false, false));
        assert!(!should_skip_phase(PhaseSpec::Map, true, true, true));
    }

    #[test]
    fn test_should_skip_phase_reduce() {
        assert!(should_skip_phase(PhaseSpec::Reduce, true, false, true));
        assert!(should_skip_phase(PhaseSpec::Reduce, true, true, false));
        assert!(!should_skip_phase(PhaseSpec::Reduce, true, true, true));
    }

    #[test]
    fn test_estimate_resources() {
        let estimate = estimate_resources(4);
        assert_eq!(estimate.max_agents, 4);
        assert_eq!(estimate.memory_per_agent_mb, 512);
        assert_eq!(estimate.total_memory_mb, 2048);
    }
}
