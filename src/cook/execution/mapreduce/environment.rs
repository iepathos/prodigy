//! Environment types for MapReduce effect execution
//!
//! This module defines environment types used with Stillwater's Effect pattern
//! for dependency injection in MapReduce operations.
//!
//! NOTE (Spec 173): This is a foundational implementation demonstrating the pattern.
//! Full integration with existing MapReduce coordinator will be done incrementally.

use serde_json::Value;
use std::collections::HashMap;

/// Environment for map phase operations
///
/// Provides dependencies needed for executing agents.
/// This is a simplified type demonstrating the Effect pattern.
///
/// Future work will integrate with:
/// - WorktreeManager for git worktree operations
/// - AgentCommandExecutor for command execution
/// - Storage for checkpointing
#[derive(Clone, Debug)]
pub struct MapEnv {
    /// Workflow environment variables
    pub workflow_env: HashMap<String, Value>,
    /// Additional configuration
    pub config: HashMap<String, Value>,
}

/// Environment for phase operations (setup/reduce)
///
/// Provides dependencies for non-agent phases.
/// This is a simplified type demonstrating the Effect pattern.
#[derive(Clone, Debug)]
pub struct PhaseEnv {
    /// Variables from workflow and previous phases
    pub variables: HashMap<String, Value>,
    /// Workflow environment variables
    pub workflow_env: HashMap<String, Value>,
}

impl MapEnv {
    /// Create a new map environment
    pub fn new(workflow_env: HashMap<String, Value>, config: HashMap<String, Value>) -> Self {
        Self {
            workflow_env,
            config,
        }
    }
}

impl PhaseEnv {
    /// Create a new phase environment
    pub fn new(variables: HashMap<String, Value>, workflow_env: HashMap<String, Value>) -> Self {
        Self {
            variables,
            workflow_env,
        }
    }
}
