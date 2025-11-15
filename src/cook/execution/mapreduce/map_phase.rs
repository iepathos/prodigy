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
#[allow(clippy::too_many_arguments)]
pub async fn execute_with_state<F>(
    job_id: &str,
    _map_phase: &MapPhase,
    work_items: Vec<Value>,
    state: Arc<Mutex<MapReduceJobState>>,
    max_parallel: usize,
    worktree_pool: Option<Arc<WorktreePool>>,
    workflow_env: &HashMap<String, String>,
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
        total_items, max_parallel
    );

    // Launch agents for each work item
    for (index, item) in work_items.into_iter().enumerate() {
        let sem_permit = match semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(e) => {
                warn!(
                    "Failed to acquire semaphore permit for agent {}: {}",
                    index, e
                );
                return Err(MapReduceError::General {
                    message: format!("Failed to acquire semaphore permit: {}", e),
                    source: None,
                });
            }
        };
        let state_clone = state.clone();
        let pool_clone = worktree_pool.clone();
        let agent_executor = agent_executor.clone();
        let agent_id = format!("agent-{}-{}", job_id, index);

        // Create agent context with workflow environment variables
        let context = create_agent_context(&agent_id, &item, index, workflow_env);

        futures.push(tokio::spawn(async move {
            let result = execute_agent_with_pool(context, pool_clone, agent_executor).await;

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
    pool.acquire(crate::worktree::WorktreeRequest::Named(
        agent_id.to_string(),
    ))
    .await
    .map_err(|e| MapReduceError::General {
        message: format!("Failed to acquire worktree for {}: {}", agent_id, e),
        source: None,
    })
}

/// Create agent context from work item and workflow environment
///
/// Variable precedence (highest to lowest):
/// 1. Item variables (e.g., ${item.name})
/// 2. Agent-specific variables (e.g., ${ITEM_INDEX})
/// 3. Workflow environment variables (e.g., ${BLOG_POST})
fn create_agent_context(
    agent_id: &str,
    item: &Value,
    index: usize,
    workflow_env: &HashMap<String, String>,
) -> AgentContext {
    let mut context = AgentContext::new(
        agent_id.to_string(),
        std::path::PathBuf::from("."),
        agent_id.to_string(),
        crate::cook::orchestrator::ExecutionEnvironment {
            working_dir: Arc::new(std::path::PathBuf::from(".")),
            project_dir: Arc::new(std::path::PathBuf::from(".")),
            worktree_name: Some(agent_id.to_string().into()),
            session_id: format!("agent-session-{}", agent_id).into(),
        },
    );

    // Start with workflow environment variables (lowest precedence)
    context.variables = workflow_env.clone();

    // Add item variables (these override workflow env)
    let item_vars = extract_item_variables(item);
    context.variables.extend(item_vars);

    // Add agent-specific variables (highest precedence)
    context
        .variables
        .insert("ITEM_INDEX".to_string(), index.to_string());

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_item_variables_string_fields() {
        let item = json!({
            "name": "test-item",
            "description": "A test item"
        });

        let vars = extract_item_variables(&item);

        assert_eq!(vars.get("item.name"), Some(&"test-item".to_string()));
        assert_eq!(
            vars.get("item.description"),
            Some(&"A test item".to_string())
        );
    }

    #[test]
    fn test_extract_item_variables_number_fields() {
        let item = json!({
            "priority": 5,
            "score": 42.5
        });

        let vars = extract_item_variables(&item);

        assert_eq!(vars.get("item.priority"), Some(&"5".to_string()));
        assert_eq!(vars.get("item.score"), Some(&"42.5".to_string()));
    }

    #[test]
    fn test_extract_item_variables_bool_fields() {
        let item = json!({
            "enabled": true,
            "completed": false
        });

        let vars = extract_item_variables(&item);

        assert_eq!(vars.get("item.enabled"), Some(&"true".to_string()));
        assert_eq!(vars.get("item.completed"), Some(&"false".to_string()));
    }

    #[test]
    fn test_extract_item_variables_complex_types() {
        let item = json!({
            "tags": ["tag1", "tag2", "tag3"],
            "metadata": {"author": "alice", "version": 2}
        });

        let vars = extract_item_variables(&item);

        // Arrays and objects should be JSON-serialized
        assert!(vars.contains_key("item.tags"));
        assert!(vars.contains_key("item.metadata"));

        // Verify they contain expected JSON structure
        let tags = vars.get("item.tags").unwrap();
        assert!(tags.contains("tag1"));
        assert!(tags.contains("tag2"));

        let metadata = vars.get("item.metadata").unwrap();
        assert!(metadata.contains("author"));
        assert!(metadata.contains("alice"));
    }

    #[test]
    fn test_extract_item_variables_mixed_types() {
        let item = json!({
            "string_field": "text",
            "number_field": 42,
            "bool_field": true,
            "array_field": [1, 2, 3],
            "object_field": {"nested": "value"}
        });

        let vars = extract_item_variables(&item);

        assert_eq!(vars.len(), 5);
        assert_eq!(vars.get("item.string_field"), Some(&"text".to_string()));
        assert_eq!(vars.get("item.number_field"), Some(&"42".to_string()));
        assert_eq!(vars.get("item.bool_field"), Some(&"true".to_string()));
        assert!(vars.contains_key("item.array_field"));
        assert!(vars.contains_key("item.object_field"));
    }

    #[test]
    fn test_extract_item_variables_empty_object() {
        let item = json!({});

        let vars = extract_item_variables(&item);

        assert!(vars.is_empty());
    }

    #[test]
    fn test_extract_item_variables_non_object() {
        // When item is not an object, should return empty map
        let item = json!("not-an-object");

        let vars = extract_item_variables(&item);

        assert!(vars.is_empty());
    }

    #[test]
    fn test_create_agent_context_basic() {
        let workflow_env = HashMap::new();
        let item = json!({
            "name": "test-item"
        });

        let context = create_agent_context("agent-1", &item, 0, &workflow_env);

        assert_eq!(context.item_id, "agent-1");
        assert_eq!(
            context.variables.get("item.name"),
            Some(&"test-item".to_string())
        );
        assert_eq!(context.variables.get("ITEM_INDEX"), Some(&"0".to_string()));
    }

    #[test]
    fn test_create_agent_context_with_workflow_env() {
        let mut workflow_env = HashMap::new();
        workflow_env.insert("BLOG_POST".to_string(), "my-blog-post".to_string());
        workflow_env.insert("SITE_URL".to_string(), "https://example.com".to_string());

        let item = json!({
            "name": "test-item"
        });

        let context = create_agent_context("agent-1", &item, 0, &workflow_env);

        // Should have workflow env vars
        assert_eq!(
            context.variables.get("BLOG_POST"),
            Some(&"my-blog-post".to_string())
        );
        assert_eq!(
            context.variables.get("SITE_URL"),
            Some(&"https://example.com".to_string())
        );
        // Should have item vars
        assert_eq!(
            context.variables.get("item.name"),
            Some(&"test-item".to_string())
        );
        // Should have agent-specific vars
        assert_eq!(context.variables.get("ITEM_INDEX"), Some(&"0".to_string()));
    }

    #[test]
    fn test_create_agent_context_variable_precedence() {
        // Test that item variables override workflow env variables
        let mut workflow_env = HashMap::new();
        workflow_env.insert("item.name".to_string(), "workflow-value".to_string());
        workflow_env.insert("BLOG_POST".to_string(), "workflow-blog".to_string());

        let item = json!({
            "name": "item-value",
            "priority": 5
        });

        let context = create_agent_context("agent-1", &item, 0, &workflow_env);

        // Item variables should override workflow env
        assert_eq!(
            context.variables.get("item.name"),
            Some(&"item-value".to_string()),
            "Item variable should override workflow env"
        );
        // Workflow env should still be present for non-conflicting vars
        assert_eq!(
            context.variables.get("BLOG_POST"),
            Some(&"workflow-blog".to_string())
        );
        // Item-specific vars should exist
        assert_eq!(
            context.variables.get("item.priority"),
            Some(&"5".to_string())
        );
        // Agent-specific vars should exist (highest precedence)
        assert_eq!(context.variables.get("ITEM_INDEX"), Some(&"0".to_string()));
    }

    #[test]
    fn test_create_agent_context_item_index_increments() {
        let workflow_env = HashMap::new();
        let item = json!({"name": "test"});

        let context0 = create_agent_context("agent-0", &item, 0, &workflow_env);
        let context5 = create_agent_context("agent-5", &item, 5, &workflow_env);
        let context10 = create_agent_context("agent-10", &item, 10, &workflow_env);

        assert_eq!(context0.variables.get("ITEM_INDEX"), Some(&"0".to_string()));
        assert_eq!(context5.variables.get("ITEM_INDEX"), Some(&"5".to_string()));
        assert_eq!(
            context10.variables.get("ITEM_INDEX"),
            Some(&"10".to_string())
        );
    }

    #[test]
    fn test_create_agent_context_all_precedence_levels() {
        // Test all three precedence levels in one scenario
        let mut workflow_env = HashMap::new();
        workflow_env.insert("WORKFLOW_VAR".to_string(), "workflow-value".to_string());
        workflow_env.insert("SHARED_VAR".to_string(), "from-workflow".to_string());
        workflow_env.insert("ITEM_INDEX".to_string(), "should-be-overridden".to_string());

        let item = json!({
            "SHARED_VAR": "from-item",
            "item_specific": "item-only"
        });

        let context = create_agent_context("agent-1", &item, 99, &workflow_env);

        // Level 3 (lowest): Workflow env var that's not overridden
        assert_eq!(
            context.variables.get("WORKFLOW_VAR"),
            Some(&"workflow-value".to_string())
        );

        // Level 2: Item variable overrides workflow env
        assert_eq!(
            context.variables.get("item.SHARED_VAR"),
            Some(&"from-item".to_string())
        );

        // Level 1 (highest): Agent-specific variable overrides everything
        assert_eq!(
            context.variables.get("ITEM_INDEX"),
            Some(&"99".to_string()),
            "Agent-specific ITEM_INDEX should override workflow env"
        );

        // Item-specific var that doesn't conflict
        assert_eq!(
            context.variables.get("item.item_specific"),
            Some(&"item-only".to_string())
        );
    }

    #[test]
    fn test_create_agent_context_empty_workflow_env() {
        let workflow_env = HashMap::new();
        let item = json!({
            "name": "test",
            "priority": 1
        });

        let context = create_agent_context("agent-1", &item, 0, &workflow_env);

        // Should only have item vars and ITEM_INDEX
        assert_eq!(
            context.variables.get("item.name"),
            Some(&"test".to_string())
        );
        assert_eq!(
            context.variables.get("item.priority"),
            Some(&"1".to_string())
        );
        assert_eq!(context.variables.get("ITEM_INDEX"), Some(&"0".to_string()));
        assert_eq!(context.variables.len(), 3); // item.name, item.priority, ITEM_INDEX
    }

    #[test]
    fn test_create_agent_context_empty_item() {
        let mut workflow_env = HashMap::new();
        workflow_env.insert("ENV_VAR".to_string(), "env-value".to_string());

        let item = json!({});

        let context = create_agent_context("agent-1", &item, 0, &workflow_env);

        // Should have workflow env and ITEM_INDEX
        assert_eq!(
            context.variables.get("ENV_VAR"),
            Some(&"env-value".to_string())
        );
        assert_eq!(context.variables.get("ITEM_INDEX"), Some(&"0".to_string()));
        assert_eq!(context.variables.len(), 2); // ENV_VAR, ITEM_INDEX
    }

    #[test]
    fn test_create_agent_context_special_characters_in_values() {
        let mut workflow_env = HashMap::new();
        workflow_env.insert(
            "URL".to_string(),
            "https://example.com/path?query=value&other=123".to_string(),
        );

        let item = json!({
            "path": "/path/to/file with spaces.txt",
            "command": "echo 'hello world' && ls -la"
        });

        let context = create_agent_context("agent-1", &item, 0, &workflow_env);

        assert_eq!(
            context.variables.get("URL"),
            Some(&"https://example.com/path?query=value&other=123".to_string())
        );
        assert_eq!(
            context.variables.get("item.path"),
            Some(&"/path/to/file with spaces.txt".to_string())
        );
        assert_eq!(
            context.variables.get("item.command"),
            Some(&"echo 'hello world' && ls -la".to_string())
        );
    }
}
