//! Agent resource management module for MapReduce
//!
//! Handles agent lifecycle, context management, and execution coordination.

use crate::cook::execution::errors::{ErrorContext, MapReduceError, SpanInfo};
use crate::worktree::WorktreeSession;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent-specific resource manager
pub struct AgentResourceManager {
    /// Active agent contexts
    active_contexts: Arc<RwLock<HashMap<String, AgentContext>>>,
}

impl AgentResourceManager {
    /// Create a new agent resource manager
    pub fn new() -> Self {
        Self {
            active_contexts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create worktree error with context
    pub fn create_worktree_error(
        &self,
        agent_id: &str,
        reason: String,
        correlation_id: &str,
    ) -> MapReduceError {
        let context = create_error_context("worktree_creation", correlation_id);
        MapReduceError::WorktreeCreationFailed {
            agent_id: agent_id.to_string(),
            reason: reason.clone(),
            source: std::io::Error::other(reason),
        }
        .with_context(context)
        .error
    }

    /// Initialize agent context
    pub fn initialize_agent_context(
        &self,
        agent_id: &str,
        item: &Value,
        item_index: usize,
        worktree_session: &WorktreeSession,
        correlation_id: &str,
    ) -> HashMap<String, Value> {
        let mut context = HashMap::new();

        // Add item data
        context.insert("item".to_string(), item.clone());
        context.insert("item_index".to_string(), Value::Number(item_index.into()));
        context.insert("agent_id".to_string(), Value::String(agent_id.to_string()));

        // Add worktree info
        context.insert(
            "worktree_name".to_string(),
            Value::String(worktree_session.name.clone()),
        );
        context.insert(
            "worktree_path".to_string(),
            Value::String(worktree_session.path.display().to_string()),
        );

        // Add MapReduce context
        let mapreduce_context = serde_json::json!({
            "job_id": correlation_id,
            "agent": {
                "id": agent_id,
                "index": item_index,
                "worktree": &worktree_session.name,
            },
            "item": item,
        });
        context.insert("map".to_string(), mapreduce_context);

        context
    }

    /// Register an active agent context
    pub async fn register_context(&self, agent_id: String, context: AgentContext) {
        let mut contexts = self.active_contexts.write().await;
        contexts.insert(agent_id, context);
    }

    /// Unregister an agent context
    pub async fn unregister_context(&self, agent_id: &str) -> Option<AgentContext> {
        let mut contexts = self.active_contexts.write().await;
        contexts.remove(agent_id)
    }

    /// Get all active contexts
    pub async fn get_active_contexts(&self) -> Vec<(String, AgentContext)> {
        let contexts = self.active_contexts.read().await;
        contexts
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get context for a specific agent
    pub async fn get_context(&self, agent_id: &str) -> Option<AgentContext> {
        let contexts = self.active_contexts.read().await;
        contexts.get(agent_id).cloned()
    }
}

/// Agent execution context
#[derive(Debug, Clone)]
pub struct AgentContext {
    pub agent_id: String,
    pub item: Value,
    pub item_index: usize,
    pub worktree_session: WorktreeSession,
    pub variables: HashMap<String, Value>,
}

impl AgentContext {
    /// Create a new agent context
    pub fn new(
        agent_id: String,
        item: Value,
        item_index: usize,
        worktree_session: WorktreeSession,
        variables: HashMap<String, Value>,
    ) -> Self {
        Self {
            agent_id,
            item,
            item_index,
            worktree_session,
            variables,
        }
    }

    /// Get the working directory for this agent
    pub fn working_directory(&self) -> std::path::PathBuf {
        self.worktree_session.path.clone()
    }

    /// Get the branch name for this agent
    pub fn branch_name(&self) -> String {
        self.worktree_session.branch.clone()
    }
}

/// Create error context with correlation ID
fn create_error_context(span_name: &str, correlation_id: &str) -> ErrorContext {
    ErrorContext {
        correlation_id: correlation_id.to_string(),
        timestamp: Utc::now(),
        hostname: std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string()),
        thread_id: format!("{:?}", std::thread::current().id()),
        span_trace: vec![SpanInfo {
            name: span_name.to_string(),
            start: Utc::now(),
            attributes: HashMap::new(),
        }],
    }
}

impl Default for AgentResourceManager {
    fn default() -> Self {
        Self::new()
    }
}