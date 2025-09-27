//! Resource estimation for dry-run mode
//!
//! Estimates memory, disk, network, and other resource requirements for MapReduce workflows.

use super::types::{
    DiskEstimate, MemoryEstimate, NetworkEstimate, ResourceEstimates, StorageEstimate,
};
use crate::cook::execution::mapreduce::{MapPhase, ReducePhase, SetupPhase};
use serde_json::Value;
use tracing::debug;

/// Estimator for resource requirements
pub struct ResourceEstimator;

impl ResourceEstimator {
    /// Create a new resource estimator
    pub fn new() -> Self {
        Self
    }

    /// Estimate resources for a MapReduce workflow
    pub fn estimate_resources(
        &self,
        map_phase: &MapPhase,
        work_items: &[Value],
        setup_phase: Option<&SetupPhase>,
        reduce_phase: Option<&ReducePhase>,
    ) -> ResourceEstimates {
        debug!(
            "Estimating resource requirements for {} work items",
            work_items.len()
        );

        let memory_usage = self.estimate_memory(map_phase, work_items);
        let disk_usage = self.estimate_disk(map_phase, work_items, setup_phase, reduce_phase);
        let network_usage = self.estimate_network(map_phase, work_items, setup_phase, reduce_phase);
        let worktree_count = self.calculate_worktree_count(map_phase, work_items);
        let checkpoint_storage = self.estimate_checkpoint_storage(work_items, map_phase);

        ResourceEstimates {
            memory_usage,
            disk_usage,
            network_usage,
            worktree_count,
            checkpoint_storage,
        }
    }

    /// Estimate memory requirements
    fn estimate_memory(&self, map_phase: &MapPhase, work_items: &[Value]) -> MemoryEstimate {
        // Base memory per agent (MB)
        const BASE_MEMORY_PER_AGENT: usize = 50;
        const MEMORY_PER_COMMAND: usize = 10;
        const CLAUDE_COMMAND_MEMORY: usize = 100; // Claude commands use more memory

        // Calculate data size per item
        let max_item_size = work_items
            .iter()
            .map(|item| serde_json::to_string(item).unwrap_or_default().len())
            .max()
            .unwrap_or(1024);

        let data_memory_per_item = max_item_size / 1024 / 1024; // Convert to MB

        // Calculate command memory requirements
        let mut command_memory = 0;
        for command in &map_phase.agent_template {
            if command.claude.is_some() {
                command_memory += CLAUDE_COMMAND_MEMORY;
            } else {
                command_memory += MEMORY_PER_COMMAND;
            }
        }

        let memory_per_agent = BASE_MEMORY_PER_AGENT + command_memory + data_memory_per_item;
        let peak_concurrent_agents = map_phase.config.max_parallel.min(work_items.len());
        let total_mb = peak_concurrent_agents * memory_per_agent;

        MemoryEstimate {
            total_mb,
            per_agent_mb: memory_per_agent,
            peak_concurrent_agents,
        }
    }

    /// Estimate disk requirements
    fn estimate_disk(
        &self,
        map_phase: &MapPhase,
        work_items: &[Value],
        setup_phase: Option<&SetupPhase>,
        reduce_phase: Option<&ReducePhase>,
    ) -> DiskEstimate {
        // Base disk space per worktree (MB)
        const BASE_WORKTREE_SIZE: usize = 100; // Assume 100MB per worktree
        const LOG_SPACE_PER_AGENT: usize = 10; // 10MB for logs per agent
        const TEMP_SPACE_PER_COMMAND: usize = 5; // 5MB temp space per command

        let worktree_count = self.calculate_worktree_count(map_phase, work_items);
        let per_worktree_mb = BASE_WORKTREE_SIZE + LOG_SPACE_PER_AGENT;

        // Calculate temp space requirements
        let mut total_commands = map_phase.agent_template.len() * work_items.len();

        if let Some(setup) = setup_phase {
            total_commands += setup.commands.len();
        }

        if let Some(reduce) = reduce_phase {
            total_commands += reduce.commands.len();
        }

        let temp_space_mb = total_commands * TEMP_SPACE_PER_COMMAND;
        let total_mb = (worktree_count * per_worktree_mb) + temp_space_mb;

        DiskEstimate {
            total_mb,
            per_worktree_mb,
            temp_space_mb,
        }
    }

    /// Estimate network requirements
    fn estimate_network(
        &self,
        map_phase: &MapPhase,
        work_items: &[Value],
        setup_phase: Option<&SetupPhase>,
        reduce_phase: Option<&ReducePhase>,
    ) -> NetworkEstimate {
        // Estimate based on Claude API calls
        let mut api_calls = 0;
        let mut data_transfer_mb = 0;

        // Count Claude commands in agent template
        for command in &map_phase.agent_template {
            if command.claude.is_some() {
                api_calls += work_items.len(); // One API call per item
                data_transfer_mb += 1; // Estimate 1MB per Claude call
            }
        }

        // Count Claude commands in setup
        if let Some(setup) = setup_phase {
            for command in &setup.commands {
                if command.claude.is_some() {
                    api_calls += 1;
                    data_transfer_mb += 1;
                }
            }
        }

        // Count Claude commands in reduce
        if let Some(reduce) = reduce_phase {
            for command in &reduce.commands {
                if command.claude.is_some() {
                    api_calls += 1;
                    data_transfer_mb += 1;
                }
            }
        }

        // Add git operations
        let worktree_count = self.calculate_worktree_count(map_phase, work_items);
        let git_operations = worktree_count * 2; // Clone and push for each worktree
        data_transfer_mb += git_operations * 10; // Estimate 10MB per git operation

        let parallel_operations = map_phase.config.max_parallel;

        NetworkEstimate {
            data_transfer_mb,
            api_calls,
            parallel_operations,
        }
    }

    /// Calculate number of worktrees needed
    fn calculate_worktree_count(&self, map_phase: &MapPhase, work_items: &[Value]) -> usize {
        map_phase.config.max_parallel.min(work_items.len())
    }

    /// Estimate checkpoint storage requirements
    fn estimate_checkpoint_storage(
        &self,
        work_items: &[Value],
        _map_phase: &MapPhase,
    ) -> StorageEstimate {
        // Estimate checkpoint size based on work item data
        let avg_item_size = if work_items.is_empty() {
            1024
        } else {
            work_items
                .iter()
                .take(10) // Sample first 10 items
                .map(|item| serde_json::to_string(item).unwrap_or_default().len())
                .sum::<usize>()
                / work_items.len().min(10)
        };

        let checkpoint_size_kb = (avg_item_size * work_items.len()) / 1024;

        // Checkpoints are saved periodically
        let checkpoint_interval = 10; // Every 10 items
        let checkpoint_count = work_items.len().div_ceil(checkpoint_interval);

        let total_mb = (checkpoint_size_kb * checkpoint_count) / 1024;

        StorageEstimate {
            checkpoint_size_kb,
            checkpoint_count,
            total_mb,
        }
    }
}

impl Default for ResourceEstimator {
    fn default() -> Self {
        Self::new()
    }
}
