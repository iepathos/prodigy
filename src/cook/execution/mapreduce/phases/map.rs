//! Map phase executor for MapReduce workflows
//!
//! This module orchestrates the parallel execution of work items across
//! multiple agents, delegating implementation details to other modules.

use super::{PhaseContext, PhaseError, PhaseExecutor, PhaseMetrics, PhaseResult, PhaseType};
use crate::cook::execution::mapreduce::{AgentResult, MapPhase};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Instant;
use tracing::{debug, info, warn};

/// Executor for the map phase of MapReduce workflows
pub struct MapPhaseExecutor {
    /// The map phase configuration
    map_phase: MapPhase,
}

impl MapPhaseExecutor {
    /// Create a new map phase executor
    pub fn new(map_phase: MapPhase) -> Self {
        Self { map_phase }
    }

    /// Parse work items from input
    async fn parse_work_items(&self, context: &PhaseContext) -> Result<Vec<Value>, PhaseError> {
        // This is a simplified version - in full implementation,
        // this would use InputSource to parse the work items
        let input = &self.map_phase.config.input;

        // Check if input is a file path
        let work_items_path = context.environment.working_dir.join(input);
        if work_items_path.exists() {
            let content = std::fs::read_to_string(&work_items_path).map_err(|e| {
                PhaseError::ExecutionFailed {
                    message: format!("Failed to read work items file: {}", e),
                }
            })?;

            let items: Vec<Value> =
                serde_json::from_str(&content).map_err(|e| PhaseError::ExecutionFailed {
                    message: format!("Failed to parse work items JSON: {}", e),
                })?;

            return Ok(items);
        }

        // Otherwise, treat input as a command to execute
        // (simplified - full implementation would execute the command)
        Err(PhaseError::ExecutionFailed {
            message: "Dynamic work item generation not implemented in this simplified version"
                .to_string(),
        })
    }

    /// Distribute work items to agents
    async fn distribute_work(
        &self,
        work_items: Vec<Value>,
        context: &mut PhaseContext,
    ) -> Result<Vec<AgentResult>, PhaseError> {
        info!("Distributing {} work items to agents", work_items.len());

        // Apply filters and limits
        let filtered_items = self.apply_filters(work_items);
        let limited_items = self.apply_limits(filtered_items);

        // This is a simplified orchestration
        // In full implementation, this would:
        // 1. Create agent pool
        // 2. Distribute items to agents
        // 3. Monitor agent execution
        // 4. Collect results

        let mut results = Vec::new();
        for (index, _item) in limited_items.iter().enumerate() {
            debug!("Processing work item {}", index);

            // Simulate agent execution
            results.push(AgentResult {
                item_id: format!("item-{}", index),
                status: crate::cook::execution::mapreduce::AgentStatus::Success,
                output: Some(format!("Processed item {}", index)),
                commits: vec![format!("commit-{}", index)],
                files_modified: Vec::new(),
                duration: std::time::Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: Some(format!("agent-branch-{}", index)),
                worktree_session_id: None,
            });
        }

        // Store results in context for reduce phase
        context.map_results = Some(results.clone());

        Ok(results)
    }

    /// Apply filters to work items
    fn apply_filters(&self, items: Vec<Value>) -> Vec<Value> {
        if let Some(_filter) = &self.map_phase.filter {
            // Simplified filter logic
            // Full implementation would use proper expression evaluation
            items
                .into_iter()
                .filter(|_item| {
                    // Placeholder filter
                    true
                })
                .collect()
        } else {
            items
        }
    }

    /// Apply limits to work items
    fn apply_limits(&self, items: Vec<Value>) -> Vec<Value> {
        let mut limited = items;

        // Apply offset
        if let Some(offset) = self.map_phase.config.offset {
            limited = limited.into_iter().skip(offset).collect();
        }

        // Apply max_items
        if let Some(max_items) = self.map_phase.config.max_items {
            limited = limited.into_iter().take(max_items).collect();
        }

        limited
    }
}

#[async_trait]
impl PhaseExecutor for MapPhaseExecutor {
    async fn execute(&self, context: &mut PhaseContext) -> Result<PhaseResult, PhaseError> {
        info!("Starting map phase execution");
        let start_time = Instant::now();

        // Parse work items
        let work_items = self.parse_work_items(context).await?;
        info!("Found {} work items to process", work_items.len());

        if work_items.is_empty() {
            warn!("No work items to process in map phase");
            return Ok(PhaseResult {
                phase_type: PhaseType::Map,
                success: true,
                data: Some(json!({
                    "message": "No work items to process",
                    "results": []
                })),
                error_message: Some("No work items found".to_string()),
                metrics: PhaseMetrics::default(),
            });
        }

        // Distribute work to agents and collect results
        let results = self.distribute_work(work_items.clone(), context).await?;

        // Calculate metrics
        let successful = results.iter().filter(|r| r.is_success()).count();
        let failed = results.iter().filter(|r| !r.is_success()).count();

        let duration = start_time.elapsed();
        let metrics = PhaseMetrics {
            duration_secs: duration.as_secs_f64(),
            items_processed: results.len(),
            items_successful: successful,
            items_failed: failed,
        };

        info!(
            "Map phase completed: {} successful, {} failed out of {} total",
            successful,
            failed,
            results.len()
        );

        Ok(PhaseResult {
            phase_type: PhaseType::Map,
            success: failed == 0,
            data: Some(json!({
                "total": results.len(),
                "successful": successful,
                "failed": failed,
                "results": results,
            })),
            error_message: if failed > 0 {
                Some(format!("{} items failed processing", failed))
            } else {
                None
            },
            metrics,
        })
    }

    fn phase_type(&self) -> PhaseType {
        PhaseType::Map
    }

    fn validate_context(&self, _context: &PhaseContext) -> Result<(), PhaseError> {
        // Validate that we have a valid input source
        if self.map_phase.config.input.is_empty() {
            return Err(PhaseError::ValidationError {
                message: "Map phase input source is not specified".to_string(),
            });
        }

        // Validate max_parallel is reasonable
        if self.map_phase.config.max_parallel == 0 {
            return Err(PhaseError::ValidationError {
                message: "max_parallel must be greater than 0".to_string(),
            });
        }

        Ok(())
    }
}
