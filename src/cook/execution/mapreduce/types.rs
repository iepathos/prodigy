//! Shared types and traits for MapReduce operations
//!
//! This module contains type definitions shared across the MapReduce
//! implementation, promoting consistency and reducing coupling.

use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::execution::variables::{Variable, VariableContext};
use crate::cook::orchestrator::ExecutionEnvironment;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for MapReduce execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceConfig {
    /// Input source (file path or command)
    #[serde(default)]
    pub input: String,
    /// JSON path expression to extract work items
    #[serde(default)]
    pub json_path: String,
    /// Maximum number of parallel agents
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,
    /// Timeout for individual agent execution (in seconds)
    pub agent_timeout_secs: Option<u64>,
    /// Whether to continue on agent failures
    pub continue_on_failure: bool,
    /// Batch size for processing work items
    pub batch_size: Option<usize>,
    /// Enable checkpoint saving
    pub enable_checkpoints: bool,
    /// Maximum number of items to process
    pub max_items: Option<usize>,
    /// Number of items to skip
    pub offset: Option<usize>,
}

fn default_max_parallel() -> usize {
    10
}

impl Default for MapReduceConfig {
    fn default() -> Self {
        Self {
            input: String::new(),
            json_path: String::new(),
            max_parallel: default_max_parallel(),
            agent_timeout_secs: Some(300),
            continue_on_failure: false,
            batch_size: None,
            enable_checkpoints: true,
            max_items: None,
            offset: None,
        }
    }
}

/// Setup phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupPhase {
    /// Commands to execute during setup
    pub commands: Vec<crate::cook::workflow::WorkflowStep>,
    /// Timeout for setup phase (in seconds)
    pub timeout: u64,
    /// Variables to capture from setup commands
    #[serde(default)]
    pub capture_outputs: HashMap<String, usize>,
}

/// Map phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPhase {
    /// Input source specification
    #[serde(flatten)]
    pub config: MapReduceConfig,
    /// JSONPath expression for data extraction
    pub json_path: Option<String>,
    /// Agent template commands
    pub agent_template: Vec<crate::cook::workflow::WorkflowStep>,
    /// Filter expression for work items
    pub filter: Option<String>,
    /// Sort expression for work items
    pub sort_by: Option<String>,
    /// Maximum items to process
    pub max_items: Option<usize>,
    /// Optional distinct field for deduplication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct: Option<String>,
}

/// Reduce phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducePhase {
    /// Commands to execute during reduction
    pub commands: Vec<crate::cook::workflow::WorkflowStep>,
    /// Timeout for reduce phase (in seconds)
    pub timeout_secs: Option<u64>,
}

/// Options for resuming MapReduce jobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeOptions {
    /// Whether to reprocess failed items
    pub reprocess_failed: bool,
    /// Maximum parallel agents for resume
    pub max_parallel: Option<usize>,
    /// Skip validation of checkpoint
    pub skip_validation: bool,
    /// Custom timeout for resumed agents
    pub agent_timeout_secs: Option<u64>,
}

impl Default for ResumeOptions {
    fn default() -> Self {
        Self {
            reprocess_failed: false,
            max_parallel: None,
            skip_validation: false,
            agent_timeout_secs: None,
        }
    }
}

/// Result of a resume operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeResult {
    /// Job ID that was resumed
    pub job_id: String,
    /// Checkpoint version resumed from
    pub resumed_from_version: u32,
    /// Total number of work items
    pub total_items: usize,
    /// Number of already completed items
    pub already_completed: usize,
    /// Number of remaining items to process
    pub remaining_items: usize,
    /// Final results after resumption
    pub final_results: Vec<crate::cook::execution::mapreduce::AgentResult>,
}

/// Context for agent execution
#[derive(Clone)]
pub struct AgentContext {
    /// Unique identifier for this agent
    pub item_id: String,
    /// Path to the agent's isolated worktree
    pub worktree_path: PathBuf,
    /// Name of the agent's worktree
    pub worktree_name: String,
    /// Variables available for interpolation
    pub variables: HashMap<String, String>,
    /// Last shell command output
    pub shell_output: Option<String>,
    /// Environment for command execution
    pub environment: ExecutionEnvironment,
    /// Current retry count for failed commands
    pub retry_count: u32,
    /// Captured outputs from previous steps
    pub captured_outputs: HashMap<String, String>,
    /// Iteration-specific variables
    pub iteration_vars: HashMap<String, String>,
    /// Variable store for structured capture data
    pub variable_store: crate::cook::workflow::variables::VariableStore,
}

impl AgentContext {
    /// Create a new agent context
    pub fn new(
        item_id: String,
        worktree_path: PathBuf,
        worktree_name: String,
        environment: ExecutionEnvironment,
    ) -> Self {
        Self {
            item_id,
            worktree_path,
            worktree_name,
            variables: HashMap::new(),
            shell_output: None,
            environment,
            retry_count: 0,
            captured_outputs: HashMap::new(),
            iteration_vars: HashMap::new(),
            variable_store: crate::cook::workflow::variables::VariableStore::new(),
        }
    }

    /// Update context with command output
    pub fn update_with_output(&mut self, output: Option<String>) {
        if let Some(out) = output {
            self.variables
                .insert("shell.output".to_string(), out.clone());
            self.variables
                .insert("shell.last_output".to_string(), out.clone());
            self.shell_output = Some(out);
        }
    }

    /// Convert to InterpolationContext
    pub fn to_interpolation_context(&self) -> InterpolationContext {
        let mut context = InterpolationContext::new();

        for (key, value) in &self.variables {
            context.set(key.clone(), Value::String(value.as_str().into()));
        }

        if let Some(ref output) = self.shell_output {
            context.set(
                "shell",
                json!({
                    "output": output,
                    "last_output": output
                }),
            );
        }

        for (key, value) in &self.captured_outputs {
            context.set(key.clone(), Value::String(value.as_str().into()));
        }

        for (key, value) in &self.iteration_vars {
            context.set(key.clone(), Value::String(value.as_str().into()));
        }

        context
    }

    /// Convert to enhanced variable context
    pub async fn to_variable_context(&self) -> VariableContext {
        let mut context = VariableContext::new();

        for (key, value) in &self.variables {
            if key.starts_with("map.") {
                if let Ok(num) = value.parse::<f64>() {
                    context.set_phase(
                        key.clone(),
                        Variable::Static(Value::Number(
                            serde_json::Number::from_f64(num).unwrap_or(0.into()),
                        )),
                    );
                } else {
                    context.set_phase(key.clone(), Variable::Static(Value::String(value.clone())));
                }
            } else {
                context.set_phase(key.clone(), Variable::Static(Value::String(value.clone())));
            }
        }

        let store_vars = self.variable_store.get_all().await;
        for (key, captured_value) in store_vars {
            let value = captured_value.to_json();

            if key.starts_with("map.") {
                context.set_phase(key.clone(), Variable::Static(value));
            } else {
                context.set_local(key.clone(), Variable::Static(value));
            }
        }

        if let Some(ref output) = self.shell_output {
            context.set_phase(
                "shell",
                Variable::Static(json!({
                    "output": output,
                    "last_output": output
                })),
            );
        }

        for (key, value) in &self.captured_outputs {
            context.set_local(key.clone(), Variable::Static(Value::String(value.clone())));
        }

        for (key, value) in &self.iteration_vars {
            context.set_local(key.clone(), Variable::Static(Value::String(value.clone())));
        }

        context.set_local(
            "workflow",
            Variable::Static(json!({
                "id": self.item_id.clone(),
                "worktree": Value::String(self.worktree_name.clone()),
                "path": self.worktree_path.to_string_lossy()
            })),
        );

        context
    }
}

/// Trait for components that can be initialized with configuration
pub trait Configurable {
    type Config;

    /// Initialize with configuration
    fn configure(config: Self::Config) -> Self;

    /// Get current configuration
    fn configuration(&self) -> &Self::Config;
}

/// Trait for components that can be reset to initial state
pub trait Resettable {
    /// Reset to initial state
    fn reset(&mut self);

    /// Check if component needs reset
    fn needs_reset(&self) -> bool;
}