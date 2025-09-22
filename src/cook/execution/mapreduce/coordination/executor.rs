//! Core MapReduce execution coordinator
//!
//! This module coordinates the execution of MapReduce jobs,
//! managing phases and resource allocation.

use crate::cook::execution::mapreduce::{
    agent::{AgentLifecycleManager, AgentResult},
    aggregation::{AggregationSummary, ResultCollector, CollectionStrategy},
    state::StateManager,
    types::{MapReduceConfig, SetupPhase, MapPhase, ReducePhase},
};
use crate::cook::execution::errors::{MapReduceError, MapReduceResult, ErrorContext};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::interaction::UserInteraction;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Main coordinator for MapReduce execution
pub struct MapReduceCoordinator {
    /// Agent lifecycle manager
    agent_manager: Arc<dyn AgentLifecycleManager>,
    /// State manager for job state
    state_manager: Arc<StateManager>,
    /// User interaction handler
    user_interaction: Arc<dyn UserInteraction>,
    /// Result collector
    result_collector: Arc<ResultCollector>,
}

impl MapReduceCoordinator {
    /// Create a new coordinator
    pub fn new(
        agent_manager: Arc<dyn AgentLifecycleManager>,
        state_manager: Arc<StateManager>,
        user_interaction: Arc<dyn UserInteraction>,
    ) -> Self {
        let result_collector = Arc::new(
            ResultCollector::new(CollectionStrategy::InMemory)
        );

        Self {
            agent_manager,
            state_manager,
            user_interaction,
            result_collector,
        }
    }

    /// Execute a complete MapReduce job
    pub async fn execute_job(
        &self,
        setup: Option<SetupPhase>,
        map_phase: MapPhase,
        reduce: Option<ReducePhase>,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        info!("Starting MapReduce job execution");

        // Execute setup phase if present
        if let Some(setup_phase) = setup {
            self.execute_setup_phase(setup_phase, env).await?;
        }

        // Load work items
        let work_items = self.load_work_items(&map_phase).await?;

        if work_items.is_empty() {
            warn!("No work items to process");
            return Ok(Vec::new());
        }

        info!("Processing {} work items", work_items.len());

        // Execute map phase
        let map_results = self.execute_map_phase_internal(
            map_phase,
            work_items,
            env
        ).await?;

        // Execute reduce phase if present
        if let Some(reduce_phase) = reduce {
            self.execute_reduce_phase(reduce_phase, &map_results, env).await?;
        }

        Ok(map_results)
    }

    /// Execute the setup phase
    async fn execute_setup_phase(
        &self,
        setup: SetupPhase,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        info!("Executing setup phase");

        self.user_interaction.display_progress("Starting setup phase...");

        // In a real implementation, execute setup commands here
        // For now, this is a placeholder

        self.user_interaction.display_success("Setup phase completed");
        Ok(())
    }

    /// Load work items for processing
    async fn load_work_items(
        &self,
        map_phase: &MapPhase,
    ) -> MapReduceResult<Vec<Value>> {
        debug!("Loading work items from input source");

        // In a real implementation, load from input source
        // For now, return empty vector as placeholder
        Ok(Vec::new())
    }

    /// Execute the map phase
    async fn execute_map_phase_internal(
        &self,
        map_phase: MapPhase,
        work_items: Vec<Value>,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        info!("Executing map phase with {} items", work_items.len());

        let total_items = work_items.len();
        let max_parallel = map_phase.config.max_parallel.min(total_items);

        self.user_interaction.display_progress(&format!(
            "Processing {} items with {} parallel agents",
            total_items, max_parallel
        ));

        // Process items (simplified for extraction)
        let mut results = Vec::new();

        for (index, item) in work_items.into_iter().enumerate() {
            // In real implementation, this would spawn agents
            debug!("Processing item {}/{}", index + 1, total_items);

            // Placeholder for agent execution
            let result = AgentResult {
                item_id: format!("item_{}", index),
                status: crate::cook::execution::mapreduce::AgentStatus::Success,
                output: Some(format!("Processed item {}", index)),
                commits: vec![],
                duration: std::time::Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            };

            results.push(result);
            self.result_collector.add_result(results.last().unwrap().clone()).await;
        }

        let summary = AggregationSummary::from_results(&results);
        self.display_map_summary(&summary);

        Ok(results)
    }

    /// Execute the reduce phase
    async fn execute_reduce_phase(
        &self,
        reduce: ReducePhase,
        map_results: &[AgentResult],
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        info!("Executing reduce phase");

        self.user_interaction.display_progress("Starting reduce phase...");

        let summary = AggregationSummary::from_results(map_results);
        self.display_reduce_summary(&summary);

        // In real implementation, execute reduce commands here

        self.user_interaction.display_success("Reduce phase completed");
        Ok(())
    }

    /// Display map phase summary
    fn display_map_summary(&self, summary: &AggregationSummary) {
        let message = format!(
            "Map phase completed: {} successful, {} failed (total: {})",
            summary.successful, summary.failed, summary.total
        );

        if summary.failed > 0 {
            self.user_interaction.display_warning(&message);
        } else {
            self.user_interaction.display_success(&message);
        }
    }

    /// Display reduce phase summary
    fn display_reduce_summary(&self, summary: &AggregationSummary) {
        self.user_interaction.display_info(&format!(
            "Reduce phase input: {} items ({} successful, {} failed)",
            summary.total, summary.successful, summary.failed
        ));
    }

    /// Get collected results
    pub async fn get_results(&self) -> Vec<AgentResult> {
        self.result_collector.get_results().await
    }

    /// Clear collected results
    pub async fn clear_results(&self) {
        self.result_collector.clear().await;
    }
}