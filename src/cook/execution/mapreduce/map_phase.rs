//! Map phase execution functionality
//!
//! This module handles the execution of the map phase in MapReduce workflows,
//! including agent spawning, work distribution, and result collection.

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::execution::mapreduce::agent::AgentResult;
use crate::cook::execution::mapreduce::{AgentContext, MapPhase, MapReduceJobState};
use crate::worktree::{WorktreeHandle, WorktreePool};
use futures::stream::{FuturesUnordered, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tracing::{info, warn};

/// Configuration for map phase execution
pub struct MapPhaseConfig {
    pub job_id: String,
    pub max_parallel: usize,
    pub item_filter: Option<String>,
    pub error_policy: crate::cook::workflow::WorkflowErrorPolicy,
}

/// Result from map phase execution
pub struct MapPhaseResult {
    pub agent_results: Vec<AgentResult>,
    pub total_items: usize,
    pub successful: usize,
    pub failed: usize,
}

/// Execute map phase with state management
pub async fn execute_with_state<F>(
    job_id: &str,
    _map_phase: &MapPhase,
    work_items: Vec<Value>,
    state: Arc<Mutex<MapReduceJobState>>,
    max_parallel: usize,
    worktree_pool: Option<Arc<WorktreePool>>,
    agent_executor: F,
) -> MapReduceResult<MapPhaseResult>
where
    F: Fn(AgentContext) -> futures::future::BoxFuture<'static, MapReduceResult<AgentResult>>
        + Send
        + Sync
        + 'static,
{
    let semaphore = Arc::new(Semaphore::new(max_parallel));
    let mut futures = FuturesUnordered::new();
    let agent_executor = Arc::new(agent_executor);

    let total_items = work_items.len();
    info!(
        "Executing map phase with {} work items (max parallel: {})",
        total_items,
        max_parallel
    );

    // Launch agents for each work item
    for (index, item) in work_items.into_iter().enumerate() {
        let sem_permit = semaphore.clone().acquire_owned().await.unwrap();
        let state_clone = state.clone();
        let pool_clone = worktree_pool.clone();
        let agent_executor = agent_executor.clone();
        let agent_id = format!("agent-{}-{}", job_id, index);

        // Create agent context
        let context = create_agent_context(&agent_id, &item, index);

        futures.push(tokio::spawn(async move {
            let result = execute_agent_with_pool(
                context,
                pool_clone,
                agent_executor,
            )
            .await;

            // Update state
            if let Ok(ref agent_result) = result {
                let mut state = state_clone.lock().await;
                state.completed_agents.insert(agent_result.item_id.clone());
            }

            drop(sem_permit);
            result
        }));
    }

    // Collect results
    let mut agent_results = Vec::new();
    while let Some(result) = futures.next().await {
        match result {
            Ok(Ok(agent_result)) => agent_results.push(agent_result),
            Ok(Err(e)) => {
                warn!("Agent execution failed: {}", e);
                // Handle error based on policy
            }
            Err(e) => {
                warn!("Agent task panicked: {}", e);
            }
        }
    }

    let successful = agent_results.iter().filter(|r| r.is_success()).count();
    let failed = agent_results.len() - successful;

    Ok(MapPhaseResult {
        agent_results,
        total_items,
        successful,
        failed,
    })
}


/// Execute single agent with worktree pool
async fn execute_agent_with_pool<F>(
    context: AgentContext,
    worktree_pool: Option<Arc<WorktreePool>>,
    executor: Arc<F>,
) -> MapReduceResult<AgentResult>
where
    F: Fn(AgentContext) -> futures::future::BoxFuture<'static, MapReduceResult<AgentResult>>
        + Send
        + Sync,
{
    // Acquire worktree if pool is available
    let _worktree_session = if let Some(pool) = worktree_pool {
        Some(acquire_worktree_from_pool(pool, &context.item_id).await?)
    } else {
        None
    };

    // Execute agent
    executor(context).await
}

/// Acquire worktree from pool
async fn acquire_worktree_from_pool(
    pool: Arc<WorktreePool>,
    agent_id: &str,
) -> MapReduceResult<WorktreeHandle> {
    pool.acquire(crate::worktree::WorktreeRequest::Named(agent_id.to_string()))
        .await
        .map_err(|e| MapReduceError::General {
            message: format!("Failed to acquire worktree for {}: {}", agent_id, e),
            source: None,
        })
}

/// Create agent context from work item
fn create_agent_context(
    agent_id: &str,
    item: &Value,
    index: usize,
) -> AgentContext {
    let mut context = AgentContext::new(
        agent_id.to_string(),
        std::path::PathBuf::from("."),
        agent_id.to_string(),
        crate::cook::orchestrator::ExecutionEnvironment {
            working_dir: std::path::PathBuf::from("."),
            project_dir: std::path::PathBuf::from("."),
            worktree_name: Some(agent_id.to_string()),
            session_id: format!("agent-session-{}", agent_id),
        },
    );

    // Add item variables
    context.variables = extract_item_variables(item);
    context.variables.insert("ITEM_INDEX".to_string(), index.to_string());

    context
}

/// Extract variables from work item
fn extract_item_variables(item: &Value) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    if let Value::Object(map) = item {
        for (key, value) in map {
            let str_value = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => serde_json::to_string(value).unwrap_or_default(),
            };
            vars.insert(format!("item.{}", key), str_value);
        }
    }

    vars
}