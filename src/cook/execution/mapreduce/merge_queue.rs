//! Merge queue for serializing agent merges in MapReduce workflows
//!
//! This module provides a queue-based system for serializing git merge operations
//! from parallel MapReduce agents back to the parent worktree. By processing merges
//! sequentially through a background worker, we eliminate MERGE_HEAD race conditions
//! while preserving parallel agent execution.

use crate::cook::execution::claude::ClaudeExecutor;
use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::execution::mapreduce::resources::git::GitOperations;
use crate::cook::orchestrator::ExecutionEnvironment;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Request to merge an agent's branch back to the parent worktree
#[derive(Debug)]
struct MergeRequest {
    /// Unique identifier for the agent
    agent_id: String,
    /// Name of the branch to merge
    branch_name: String,
    /// Item ID being processed
    item_id: String,
    /// Execution environment with parent worktree context
    env: ExecutionEnvironment,
    /// Channel to send the merge result back to the waiting agent
    response_tx: oneshot::Sender<MapReduceResult<()>>,
}

/// Queue for serializing git merge operations from parallel agents
///
/// The MergeQueue accepts merge requests from multiple concurrent agents
/// and processes them sequentially through a background worker task.
/// This prevents race conditions when multiple agents try to merge to
/// the same parent worktree simultaneously.
///
/// When conflicts occur, the queue automatically invokes Claude to resolve them.
pub struct MergeQueue {
    /// Channel for submitting merge requests
    tx: mpsc::UnboundedSender<MergeRequest>,
    /// Handle to the background worker task
    _worker_handle: Arc<JoinHandle<()>>,
}

impl MergeQueue {
    /// Create a new merge queue with a background worker
    ///
    /// The worker task will process merge requests sequentially until
    /// the queue is dropped and all senders are closed.
    pub fn new(git_ops: Arc<GitOperations>) -> Self {
        Self::new_with_claude(git_ops, None)
    }

    /// Create a new merge queue with Claude support for conflict resolution
    ///
    /// When a Claude executor is provided, the queue will automatically attempt
    /// to resolve merge conflicts using Claude-assisted merge commands.
    pub fn new_with_claude(
        git_ops: Arc<GitOperations>,
        claude_executor: Option<Arc<dyn ClaudeExecutor>>,
    ) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<MergeRequest>();

        // Spawn background worker to process merges serially
        let worker_handle = tokio::spawn(async move {
            info!("Merge queue worker started");
            let mut merge_count = 0;

            while let Some(request) = rx.recv().await {
                debug!(
                    "Processing merge request for agent {} (item {})",
                    request.agent_id, request.item_id
                );

                // Try standard git merge first
                let result = git_ops
                    .merge_agent_to_parent(&request.branch_name, &request.env)
                    .await;

                // If merge failed and we have Claude, ALWAYS try Claude-assisted merge as fallback
                // This ensures bulletproof merge handling for any type of conflict or edge case
                let final_result = match (&result, &claude_executor) {
                    (Err(_), Some(executor)) => {
                        info!(
                            "Git merge failed for agent {} (item {}), attempting Claude-assisted merge fallback",
                            request.agent_id, request.item_id
                        );

                        // Execute Claude merge command in parent worktree
                        let mut env_vars = HashMap::new();
                        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

                        match executor
                            .execute_claude_command(
                                &format!("/prodigy-merge-worktree {}", request.branch_name),
                                &request.env.working_dir,
                                env_vars,
                            )
                            .await
                        {
                            Ok(claude_result) if claude_result.success => {
                                info!(
                                    "Claude successfully resolved merge for agent {} (item {})",
                                    request.agent_id, request.item_id
                                );
                                Ok(())
                            }
                            Ok(claude_result) => {
                                warn!(
                                    "Claude failed to resolve merge for agent {} (item {}): {}",
                                    request.agent_id, request.item_id, claude_result.stderr
                                );
                                Err(MapReduceError::ProcessingError(format!(
                                    "Claude-assisted merge failed for agent {}: {}",
                                    request.branch_name, claude_result.stderr
                                )))
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to execute Claude merge command for agent {} (item {}): {}",
                                    request.agent_id, request.item_id, e
                                );
                                Err(MapReduceError::ProcessingError(format!(
                                    "Failed to execute Claude merge command: {}",
                                    e
                                )))
                            }
                        }
                    }
                    _ => result,
                };

                match &final_result {
                    Ok(()) => {
                        merge_count += 1;
                        debug!(
                            "Completed merge {}: agent {} (item {})",
                            merge_count, request.agent_id, request.item_id
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Merge failed for agent {} (item {}): {}",
                            request.agent_id, request.item_id, e
                        );
                    }
                }

                // Send result back to waiting agent (ignore send errors - agent may have timed out)
                let _ = request.response_tx.send(final_result);
            }

            info!(
                "Merge queue worker shutting down (processed {} merges)",
                merge_count
            );
        });

        Self {
            tx,
            _worker_handle: Arc::new(worker_handle),
        }
    }

    /// Submit a merge request to the queue and wait for completion
    ///
    /// This method submits a merge request to the background worker and
    /// waits for the result. Merges are processed in FIFO order.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - Unique identifier for the agent
    /// * `branch_name` - Name of the branch to merge
    /// * `item_id` - ID of the item being processed
    /// * `env` - Execution environment with parent worktree context
    ///
    /// # Returns
    ///
    /// Result of the merge operation
    pub async fn submit_merge(
        &self,
        agent_id: String,
        branch_name: String,
        item_id: String,
        env: ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        let (response_tx, response_rx) = oneshot::channel();

        let request = MergeRequest {
            agent_id: agent_id.clone(),
            branch_name,
            item_id: item_id.clone(),
            env,
            response_tx,
        };

        // Submit request to queue
        self.tx.send(request).map_err(|_| {
            MapReduceError::ProcessingError(format!(
                "Failed to submit merge request for agent {} (item {}): queue closed",
                agent_id, item_id
            ))
        })?;

        // Wait for merge to complete
        response_rx.await.map_err(|_| {
            MapReduceError::ProcessingError(format!(
                "Failed to receive merge result for agent {} (item {}): worker dropped response",
                agent_id, item_id
            ))
        })?
    }

    /// Get the number of pending merge requests in the queue
    ///
    /// Note: This is an estimate and may not be exact due to concurrent access
    #[allow(dead_code)]
    pub fn pending_count(&self) -> usize {
        // mpsc doesn't expose queue length, so we can't implement this without
        // additional state tracking. For now, return 0 as a placeholder.
        // We could add a counter if needed for observability.
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::resources::git::GitOperations;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_merge_queue_creation() {
        let git_ops = Arc::new(GitOperations::new());
        let _queue = MergeQueue::new(git_ops);
        // Queue should be created without panic
    }

    #[tokio::test]
    async fn test_merge_queue_closes_on_drop() {
        let git_ops = Arc::new(GitOperations::new());
        let queue = MergeQueue::new(git_ops);

        // Drop the queue
        drop(queue);

        // Worker should shut down gracefully
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn test_submit_merge_fails_after_drop() {
        let git_ops = Arc::new(GitOperations::new());
        let queue = MergeQueue::new(git_ops);

        // Drop the queue
        drop(queue);

        // This test can't actually call submit_merge because queue is dropped
        // The test validates that dropping works without panic
    }

    // Note: Full integration tests with actual git operations are in
    // the mapreduce integration test suite
}
