//! MapReduce executor for parallel workflow execution
//!
//! Implements parallel execution of workflow steps across multiple agents
//! using isolated git worktrees for fault isolation and parallelism.

// Declare the agent module for agent lifecycle management
pub mod agent;

// Declare the no-op writer module for fallback event logging
pub mod noop_writer;

// Declare the utils module for pure functions
pub mod utils;

// Declare the command module for command execution
pub mod command;

// Declare map and reduce phase modules
pub mod map_phase;
pub mod reduce_phase;

// Declare the resources module for resource management
pub mod resources;

// Declare the phases module for phase execution orchestration
pub mod phases;

// Declare the agent command executor module
pub mod agent_command_executor;

// Declare the progress module for progress management
pub mod progress;

// Declare the state management module
pub mod state;

// Import the PhaseExecutor trait
use self::phases::PhaseExecutor;

// Import agent types and functionality
use agent::{
    AgentLifecycleManager, AgentResultAggregator, DefaultLifecycleManager, DefaultResultAggregator,
};

// Re-export public types for external use
pub use agent::{AgentResult, AgentStatus};

// Import progress management components
use progress::{operations::AgentOperation, tracker::ProgressTracker};

// Import utility functions from utils module
use utils::{calculate_map_result_summary, generate_agent_branch_name, generate_agent_id};

// Import state management components
use self::state::{persistence::DefaultStateStore, StateManager};

use crate::commands::CommandRegistry;
use crate::cook::execution::data_pipeline::DataPipeline;
use crate::cook::execution::dlq::{DeadLetterQueue, DeadLetteredItem, ErrorType, FailureDetail};
use crate::cook::execution::errors::{ErrorContext, MapReduceError, MapReduceResult, SpanInfo};
use crate::cook::execution::events::{EventLogger, EventWriter, JsonlEventWriter, MapReduceEvent};
use crate::cook::execution::input_source::InputSource;
use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use crate::cook::execution::progress::{
    AgentProgress, AgentState as ProgressAgentState, CLIProgressViewer, EnhancedProgressTracker,
    ProgressUpdate, UpdateType,
};
use crate::cook::execution::progress_display::{DisplayMode, MultiProgressDisplay};
use crate::cook::execution::progress_tracker::ProgressTracker as NewProgressTracker;
use crate::cook::execution::state::{DefaultJobStateManager, JobStateManager, MapReduceJobState};
use crate::cook::execution::variables::{Variable, VariableContext};
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{ErrorPolicyExecutor, StepResult, WorkflowErrorPolicy, WorkflowStep};
use crate::subprocess::SubprocessManager;
use crate::worktree::{WorktreeManager, WorktreePool, WorktreePoolConfig};
// Keep anyhow imports for backwards compatibility with state.rs which still uses anyhow::Result
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for MapReduce execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceConfig {
    /// Input source: either a file path or command to execute
    #[serde(default)]
    pub input: String,
    /// JSON path expression to extract work items (for JSON files)
    #[serde(default)]
    pub json_path: String,
    /// Maximum number of parallel agents
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,
    /// Maximum number of items to process
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    /// Number of items to skip
    #[serde(skip_serializing_if = "Option::is_none")]
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
            max_items: None,
            offset: None,
        }
    }
}

/// Setup phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupPhase {
    /// Commands to execute during setup
    pub commands: Vec<WorkflowStep>,
    /// Timeout for the entire setup phase (in seconds)
    pub timeout: u64,
    /// Variables to capture from setup commands
    /// Key is variable name, value is the command index to capture from
    #[serde(default)]
    pub capture_outputs: HashMap<String, usize>,
}

/// Map phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPhase {
    /// Input configuration
    #[serde(flatten)]
    pub config: MapReduceConfig,
    /// Agent template commands
    pub agent_template: Vec<WorkflowStep>,
    /// Optional filter expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Optional sort field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,
    /// Optional distinct field for deduplication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct: Option<String>,
}

/// Reduce phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducePhase {
    /// Commands to execute in reduce phase
    pub commands: Vec<WorkflowStep>,
}

// AgentStatus and AgentResult are now imported from the agent module

/// Options for resuming a MapReduce job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeOptions {
    /// Force resume even if job appears complete
    pub force: bool,
    /// Maximum additional retries for failed items
    pub max_additional_retries: u32,
    /// Skip validation of checkpoint integrity
    pub skip_validation: bool,
    /// Specific checkpoint version to resume from (None for latest)
    #[serde(default)]
    pub from_checkpoint: Option<u32>,
}

impl Default for ResumeOptions {
    fn default() -> Self {
        Self {
            force: false,
            max_additional_retries: 2,
            skip_validation: false,
            from_checkpoint: None,
        }
    }
}

/// Result of resuming a MapReduce job
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
    pub final_results: Vec<AgentResult>,
}

// Note: ProgressTracker and AgentOperation are now imported from the progress module

/// Context for agent-specific command execution
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
            self.shell_output = Some(out.clone());
            self.variables
                .insert("shell.output".to_string(), out.clone());
            self.variables.insert("shell.last_output".to_string(), out);
        }
    }

    /// Convert to InterpolationContext
    pub fn to_interpolation_context(&self) -> InterpolationContext {
        let mut context = InterpolationContext::new();

        // Add all variables
        for (key, value) in &self.variables {
            context.set(key.clone(), Value::String(value.clone()));
        }

        // Add shell output
        if let Some(ref output) = self.shell_output {
            context.set(
                "shell",
                json!({
                    "output": output,
                    "last_output": output
                }),
            );
        }

        // Add captured outputs
        for (key, value) in &self.captured_outputs {
            context.set(key.clone(), Value::String(value.clone()));
        }

        // Add iteration variables
        for (key, value) in &self.iteration_vars {
            context.set(key.clone(), Value::String(value.clone()));
        }

        context
    }

    /// Convert to enhanced variable context
    pub async fn to_variable_context(&self) -> VariableContext {
        let mut context = VariableContext::new();

        // Add all string variables as phase-level variables
        for (key, value) in &self.variables {
            // Handle nested map variables specially
            if key.starts_with("map.") {
                // Try to parse as a number first
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

        // Add structured variables from variable_store
        let store_vars = self.variable_store.get_all().await;
        for (key, captured_value) in store_vars {
            // Convert CapturedValue to Value and add to context
            let value = captured_value.to_json();

            // map.results and other structured data should be at phase level for reduce phase
            if key.starts_with("map.") {
                context.set_phase(key.clone(), Variable::Static(value));
            } else {
                context.set_local(key.clone(), Variable::Static(value));
            }
        }

        // Add shell output as structured data
        if let Some(ref output) = self.shell_output {
            context.set_phase(
                "shell",
                Variable::Static(json!({
                    "output": output,
                    "last_output": output
                })),
            );
        }

        // Add captured outputs
        for (key, value) in &self.captured_outputs {
            context.set_local(key.clone(), Variable::Static(Value::String(value.clone())));
        }

        // Add iteration variables
        for (key, value) in &self.iteration_vars {
            context.set_local(key.clone(), Variable::Static(Value::String(value.clone())));
        }

        // Add environment access
        context.set_local(
            "workflow",
            Variable::Static(json!({
                "id": self.item_id.clone(),
                "worktree": self.worktree_name.clone(),
                "path": self.worktree_path.to_string_lossy()
            })),
        );

        context
    }
}

/// MapReduce executor for parallel workflow execution
pub struct MapReduceExecutor {
    claude_executor: Arc<dyn ClaudeExecutor>,
    session_manager: Arc<dyn SessionManager>,
    user_interaction: Arc<dyn UserInteraction>,
    worktree_manager: Arc<WorktreeManager>,
    worktree_pool: Option<Arc<WorktreePool>>,
    project_root: PathBuf,
    interpolation_engine: Arc<Mutex<InterpolationEngine>>,
    command_registry: Arc<CommandRegistry>,
    command_router: Arc<command::CommandRouter>,
    step_executor: Arc<command::StepExecutor>,
    subprocess: Arc<SubprocessManager>,
    state_manager: Arc<dyn JobStateManager>,
    enhanced_state_manager: Arc<StateManager>,
    event_logger: Arc<EventLogger>,
    dlq: Option<Arc<DeadLetterQueue>>,
    correlation_id: String,
    enhanced_progress_tracker: Option<Arc<EnhancedProgressTracker>>,
    new_progress_tracker: Option<Arc<NewProgressTracker>>,
    enable_web_dashboard: bool,
    setup_variables: HashMap<String, String>,
    retry_state_manager: Arc<crate::cook::retry_state::RetryStateManager>,
    error_policy_executor: Option<ErrorPolicyExecutor>,
    agent_lifecycle_manager: Arc<dyn AgentLifecycleManager>,
    agent_result_aggregator: Arc<dyn AgentResultAggregator>,
    resource_manager: Arc<resources::ResourceManager>,
}

#[cfg(test)]
mod pure_function_tests {
    use super::utils::{
        add_individual_result_variables, classify_agent_status, truncate_output, AgentEventType,
        MapResultSummary,
    };
    use super::*;
    use serde_json::Value;
    use std::time::Duration;

    /// Helper function to create test AgentResult
    fn create_test_agent_result(
        item_id: &str,
        status: AgentStatus,
        output: Option<String>,
        commits: Vec<String>,
    ) -> AgentResult {
        AgentResult {
            item_id: item_id.to_string(),
            status,
            output,
            commits,
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        }
    }

    /// Test calculate_map_result_summary with mixed results
    #[test]
    fn test_calculate_map_result_summary_mixed_results() {
        let map_results = vec![
            create_test_agent_result(
                "item1",
                AgentStatus::Success,
                Some("success output".to_string()),
                vec!["commit1".to_string()],
            ),
            create_test_agent_result(
                "item2",
                AgentStatus::Failed("error".to_string()),
                Some("error output".to_string()),
                vec![],
            ),
            create_test_agent_result(
                "item3",
                AgentStatus::Success,
                Some("success output 2".to_string()),
                vec!["commit2".to_string(), "commit3".to_string()],
            ),
        ];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.total, 3);
    }

    /// Test calculate_map_result_summary with all successful results
    #[test]
    fn test_calculate_map_result_summary_all_successful() {
        let map_results = vec![
            create_test_agent_result(
                "item1",
                AgentStatus::Success,
                Some("success".to_string()),
                vec!["commit1".to_string()],
            ),
            create_test_agent_result(
                "item2",
                AgentStatus::Success,
                Some("success".to_string()),
                vec!["commit2".to_string()],
            ),
        ];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 2);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.total, 2);
    }

    /// Test calculate_map_result_summary with all failed results
    #[test]
    fn test_calculate_map_result_summary_all_failed() {
        let map_results = vec![
            AgentResult {
                item_id: "item1".to_string(),
                status: AgentStatus::Failed("error1".to_string()),
                output: None,
                commits: vec![],
                duration: Duration::from_secs(1),
                error: Some("error1".to_string()),
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
            AgentResult {
                item_id: "item2".to_string(),
                status: AgentStatus::Timeout,
                output: None,
                commits: vec![],
                duration: Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
        ];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 0);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.total, 2);
    }

    /// Test calculate_map_result_summary with empty results
    #[test]
    fn test_calculate_map_result_summary_empty_results() {
        let map_results = vec![];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.total, 0);
    }

    /// Test generate_agent_id pure function
    #[test]
    fn test_generate_agent_id() {
        assert_eq!(generate_agent_id(0, "item-1"), "agent-0-item-1");
        assert_eq!(generate_agent_id(5, "test-item"), "agent-5-test-item");
        assert_eq!(generate_agent_id(100, "special"), "agent-100-special");
    }

    /// Test generate_agent_branch_name pure function
    #[test]
    fn test_generate_agent_branch_name() {
        assert_eq!(
            generate_agent_branch_name("session-123", "item-1"),
            "prodigy-agent-session-123-item-1"
        );
        assert_eq!(
            generate_agent_branch_name("test-session", "special-item"),
            "prodigy-agent-test-session-special-item"
        );
    }

    /// Test classify_agent_status pure function
    #[test]
    fn test_classify_agent_status() {
        assert_eq!(
            classify_agent_status(&AgentStatus::Success),
            AgentEventType::Completed
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Failed("error".to_string())),
            AgentEventType::Failed
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Timeout),
            AgentEventType::TimedOut
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Retrying(1)),
            AgentEventType::Retrying
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Pending),
            AgentEventType::InProgress
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Running),
            AgentEventType::InProgress
        );
    }

    /// Test build_map_results_interpolation_context
    #[test]
    fn test_build_map_results_interpolation_context() {
        let map_results = vec![
            AgentResult {
                item_id: "item1".to_string(),
                status: AgentStatus::Success,
                output: Some("success".to_string()),
                commits: vec!["commit1".to_string()],
                duration: Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
            AgentResult {
                item_id: "item2".to_string(),
                status: AgentStatus::Failed("error".to_string()),
                output: None,
                commits: vec![],
                duration: Duration::from_secs(1),
                error: Some("error".to_string()),
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
        ];

        let summary = MapResultSummary {
            successful: 1,
            failed: 1,
            total: 2,
        };

        let context =
            utils::build_map_results_interpolation_context(&map_results, &summary).unwrap();

        // Test that map object is properly structured
        let map_value = context.resolve_path(&["map".to_string()]).unwrap();

        if let Value::Object(map_obj) = map_value {
            assert_eq!(map_obj.get("successful").unwrap().as_u64().unwrap(), 1);
            assert_eq!(map_obj.get("failed").unwrap().as_u64().unwrap(), 1);
            assert_eq!(map_obj.get("total").unwrap().as_u64().unwrap(), 2);
        } else {
            panic!("Expected map to be an object");
        }

        // Test that individual paths resolve correctly
        assert_eq!(
            context
                .resolve_path(&["map".to_string(), "successful".to_string()])
                .unwrap(),
            Value::Number(1.into())
        );
        assert_eq!(
            context
                .resolve_path(&["map".to_string(), "failed".to_string()])
                .unwrap(),
            Value::Number(1.into())
        );
        assert_eq!(
            context
                .resolve_path(&["map".to_string(), "total".to_string()])
                .unwrap(),
            Value::Number(2.into())
        );

        // Test that map.results contains the full results
        let results_value = context.resolve_path(&["map.results".to_string()]).unwrap();
        assert!(results_value.is_array());
    }

    /// Test build_agent_context_variables
    #[test]
    fn test_build_agent_context_variables() {
        let map_results = vec![AgentResult {
            item_id: "test_item".to_string(),
            status: AgentStatus::Success,
            output: Some("output data".to_string()),
            commits: vec!["abc123".to_string()],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        }];

        let summary = MapResultSummary {
            successful: 1,
            failed: 0,
            total: 1,
        };

        let variables = utils::build_agent_context_variables(&map_results, &summary).unwrap();

        // Test summary statistics
        assert_eq!(variables.get("map.successful").unwrap(), "1");
        assert_eq!(variables.get("map.failed").unwrap(), "0");
        assert_eq!(variables.get("map.total").unwrap(), "1");

        // Test that results are present as JSON
        assert!(variables.contains_key("map.results"));
        assert!(variables.contains_key("map.results_json"));

        // Test individual result variables
        assert_eq!(variables.get("result.0.item_id").unwrap(), "test_item");
        assert_eq!(variables.get("result.0.status").unwrap(), "success");
        assert_eq!(variables.get("result.0.output").unwrap(), "output data");
        assert_eq!(variables.get("result.0.commits").unwrap(), "1");
    }

    /// Test add_individual_result_variables with different statuses
    #[test]
    fn test_add_individual_result_variables_various_statuses() {
        let mut variables = HashMap::new();

        // Test success result
        let success_result = AgentResult {
            item_id: "success_item".to_string(),
            status: AgentStatus::Success,
            output: Some("success output".to_string()),
            commits: vec!["commit1".to_string(), "commit2".to_string()],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        };

        add_individual_result_variables(&mut variables, 0, &success_result);
        assert_eq!(variables.get("result.0.item_id").unwrap(), "success_item");
        assert_eq!(variables.get("result.0.status").unwrap(), "success");
        assert_eq!(variables.get("result.0.output").unwrap(), "success output");
        assert_eq!(variables.get("result.0.commits").unwrap(), "2");

        // Test failed result
        let failed_result = AgentResult {
            item_id: "failed_item".to_string(),
            status: AgentStatus::Failed("test error".to_string()),
            output: None,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: Some("test error".to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        };

        add_individual_result_variables(&mut variables, 1, &failed_result);
        assert_eq!(variables.get("result.1.item_id").unwrap(), "failed_item");
        assert_eq!(
            variables.get("result.1.status").unwrap(),
            "failed: test error"
        );
        assert!(!variables.contains_key("result.1.output")); // No output for failed
        assert_eq!(variables.get("result.1.commits").unwrap(), "0");

        // Test timeout result
        let timeout_result = AgentResult {
            item_id: "timeout_item".to_string(),
            status: AgentStatus::Timeout,
            output: None,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        };

        add_individual_result_variables(&mut variables, 2, &timeout_result);
        assert_eq!(variables.get("result.2.status").unwrap(), "timeout");
    }

    /// Test truncate_output function
    #[test]
    fn test_truncate_output() {
        // Test short output - should not be truncated
        let short_output = "short output";
        assert_eq!(truncate_output(short_output, 100), "short output");

        // Test long output - should be truncated
        let long_output = "a".repeat(600);
        let truncated = truncate_output(&long_output, 500);
        assert!(truncated.len() <= 500 + "...[truncated]".len());
        assert!(truncated.ends_with("...[truncated]"));
        assert!(truncated.starts_with("aaa"));

        // Test exact length - should not be truncated
        let exact_output = "a".repeat(500);
        assert_eq!(truncate_output(&exact_output, 500), exact_output);
    }

    /// Test that the bug scenario we fixed is now properly handled
    #[test]
    fn test_mapreduce_variable_interpolation_bug_fix() {
        // This test simulates the exact scenario that was failing before our fix:
        // MapReduce variables (${map.successful}, ${map.failed}, ${map.total})
        // were showing as 0 instead of actual values in the reduce phase

        let map_results = vec![
            AgentResult {
                item_id: "item1".to_string(),
                status: AgentStatus::Success,
                output: Some("processed item 1".to_string()),
                commits: vec!["commit1".to_string()],
                duration: Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
            AgentResult {
                item_id: "item2".to_string(),
                status: AgentStatus::Success,
                output: Some("processed item 2".to_string()),
                commits: vec!["commit2".to_string()],
                duration: Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
            AgentResult {
                item_id: "item3".to_string(),
                status: AgentStatus::Failed("processing error".to_string()),
                output: None,
                commits: vec![],
                duration: Duration::from_secs(1),
                error: Some("processing error".to_string()),
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
        ];

        // Calculate summary - this is the core fix
        let summary = calculate_map_result_summary(&map_results);
        assert_eq!(summary.successful, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.total, 3);

        // Build interpolation context - this ensures variables are available
        let interp_context =
            utils::build_map_results_interpolation_context(&map_results, &summary).unwrap();

        // Test the exact variables that were failing
        let successful_value = interp_context
            .resolve_path(&["map".to_string(), "successful".to_string()])
            .unwrap();
        let failed_value = interp_context
            .resolve_path(&["map".to_string(), "failed".to_string()])
            .unwrap();
        let total_value = interp_context
            .resolve_path(&["map".to_string(), "total".to_string()])
            .unwrap();

        assert_eq!(successful_value, Value::Number(2.into()));
        assert_eq!(failed_value, Value::Number(1.into()));
        assert_eq!(total_value, Value::Number(3.into()));

        // Test shell command variables - this is what was causing substitution errors
        let shell_variables = utils::build_agent_context_variables(&map_results, &summary).unwrap();

        assert_eq!(shell_variables.get("map.successful").unwrap(), "2");
        assert_eq!(shell_variables.get("map.failed").unwrap(), "1");
        assert_eq!(shell_variables.get("map.total").unwrap(), "3");

        // Before the fix, these would have been "0", "0", "0"
        // After the fix, they correctly show "2", "1", "3"
    }
}

impl MapReduceExecutor {
    /// Log agent failure event asynchronously
    fn log_agent_failure_async(&self, job_id: String, agent_id: String, error_msg: String) {
        let event_logger = self.event_logger.clone();
        tokio::spawn(async move {
            event_logger
                .log(MapReduceEvent::AgentFailed {
                    job_id,
                    agent_id,
                    error: error_msg,
                    retry_eligible: true,
                })
                .await
                .unwrap_or_else(|e| log::warn!("Failed to log error event: {}", e));
        });
    }
    /// Create error context with correlation ID
    fn create_error_context(&self, span_name: &str) -> ErrorContext {
        ErrorContext {
            correlation_id: self.correlation_id.clone(),
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

    /// Create a new MapReduce executor
    pub async fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        worktree_manager: Arc<WorktreeManager>,
        project_root: PathBuf,
    ) -> Self {
        // Create state manager with global storage support
        let state_manager =
            match DefaultJobStateManager::new_with_global(project_root.clone()).await {
                Ok(manager) => Arc::new(manager),
                Err(e) => {
                    warn!(
                        "Failed to create global state manager: {}, falling back to local",
                        e
                    );
                    // Fallback to local storage
                    let state_dir = project_root.join(".prodigy").join("mapreduce");
                    Arc::new(DefaultJobStateManager::new(state_dir))
                }
            };

        // Use global storage for events
        let job_id = format!("mapreduce-{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
        let event_logger =
            match crate::storage::create_global_event_logger(&project_root, &job_id).await {
                Ok(logger) => {
                    info!("Using global event storage for job: {}", job_id);
                    Arc::new(logger)
                }
                Err(e) => {
                    warn!(
                        "Failed to create global event logger: {}, using temp directory fallback",
                        e
                    );
                    // Fallback to temp directory
                    let temp_path = std::env::temp_dir().join("prodigy_events.jsonl");
                    let writer: Box<dyn EventWriter> =
                        match JsonlEventWriter::new(temp_path.clone()).await {
                            Ok(w) => Box::new(w),
                            Err(e) => {
                                error!(
                                    "Failed to create fallback event logger at {:?}: {}",
                                    temp_path, e
                                );
                                error!("Using no-op event writer - events will not be persisted!");
                                Box::new(noop_writer::NoOpEventWriter::new())
                            }
                        };
                    let event_writers: Vec<Box<dyn EventWriter>> = vec![writer];
                    Arc::new(EventLogger::new(event_writers))
                }
            };

        // Create agent lifecycle manager and result aggregator
        let agent_lifecycle_manager =
            Arc::new(DefaultLifecycleManager::new(worktree_manager.clone()));
        let agent_result_aggregator = Arc::new(DefaultResultAggregator::new());

        // Create command registry and router
        let command_registry = Arc::new(CommandRegistry::with_defaults().await);

        // Initialize command router with executors
        let mut command_router = command::CommandRouter::new();
        command_router.register(
            "claude".to_string(),
            Arc::new(command::ClaudeCommandExecutor::new(claude_executor.clone())),
        );
        command_router.register(
            "shell".to_string(),
            Arc::new(command::ShellCommandExecutor::new()),
        );
        command_router.register(
            "handler".to_string(),
            Arc::new(command::HandlerCommandExecutor::new(
                command_registry.clone(),
            )),
        );

        // Create the command router Arc
        let command_router = Arc::new(command_router);

        // Create the interpolation engine Arc
        let interpolation_engine = Arc::new(Mutex::new(InterpolationEngine::new(false)));

        // Create the step interpolator
        let step_interpolator = Arc::new(command::StepInterpolator::new(Arc::new(Mutex::new(
            command::InterpolationEngine::new(false),
        ))));

        // Create the step executor
        let step_executor = Arc::new(command::StepExecutor::new(
            command_router.clone(),
            step_interpolator,
        ));

        // Create resource manager with worktree manager
        let resource_manager = Arc::new(resources::ResourceManager::with_worktree_manager(
            None,
            worktree_manager.clone(),
        ));

        // Create enhanced state manager using the existing state manager
        let enhanced_state_manager = Arc::new(StateManager::new(Arc::new(
            DefaultStateStore::from_manager(state_manager.clone()),
        )));

        Self {
            claude_executor,
            session_manager,
            user_interaction,
            worktree_manager,
            worktree_pool: None, // Will be initialized when needed with config
            project_root,
            interpolation_engine,
            command_registry,
            command_router,
            step_executor,
            subprocess: Arc::new(SubprocessManager::production()),
            state_manager,
            enhanced_state_manager,
            event_logger,
            dlq: None, // Will be initialized per job
            correlation_id: Uuid::new_v4().to_string(),
            enhanced_progress_tracker: None,
            new_progress_tracker: None,
            enable_web_dashboard: std::env::var("PRODIGY_WEB_DASHBOARD")
                .unwrap_or_else(|_| "false".to_string())
                .eq_ignore_ascii_case("true"),
            setup_variables: HashMap::new(),
            retry_state_manager: Arc::new(crate::cook::retry_state::RetryStateManager::new()),
            error_policy_executor: None,
            agent_lifecycle_manager,
            agent_result_aggregator,
            resource_manager,
        }
    }

    /// Set the error handling policy for this executor
    pub fn set_error_policy(&mut self, policy: WorkflowErrorPolicy) {
        self.error_policy_executor = Some(ErrorPolicyExecutor::new(policy));
    }

    /// Initialize worktree pool with given configuration
    fn initialize_pool(&mut self, config: WorktreePoolConfig) {
        if self.worktree_pool.is_none() {
            let pool = Arc::new(WorktreePool::new(config, self.worktree_manager.clone()));
            self.worktree_pool = Some(pool.clone());

            // Update resource manager with the new pool
            self.resource_manager = Arc::new(resources::ResourceManager::with_worktree_manager(
                Some(pool),
                self.worktree_manager.clone(),
            ));
        }
    }

    /// Initialize pool with default configuration if not already initialized
    fn ensure_pool_initialized(&mut self) {
        if self.worktree_pool.is_none() {
            let config = WorktreePoolConfig::default();
            self.initialize_pool(config);
        }
    }

    /// Execute a MapReduce workflow
    pub async fn execute(
        &mut self,
        map_phase: &MapPhase,
        reduce_phase: Option<&ReducePhase>,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        self.execute_with_context(map_phase, reduce_phase, env, HashMap::new())
            .await
    }

    /// Execute a MapReduce workflow with setup context variables
    pub async fn execute_with_context(
        &mut self,
        map_phase: &MapPhase,
        reduce_phase: Option<&ReducePhase>,
        env: &ExecutionEnvironment,
        setup_variables: HashMap<String, String>,
    ) -> MapReduceResult<Vec<AgentResult>> {
        let start_time = Instant::now();

        // Initialize worktree pool if needed
        self.ensure_pool_initialized();

        // Store setup variables for use in agent execution
        self.setup_variables = setup_variables;

        // Load and parse work items with filtering and sorting
        let work_items = self
            .load_work_items_with_pipeline(
                &map_phase.config,
                &map_phase.filter,
                &map_phase.sort_by,
                &map_phase.distinct,
            )
            .await?;

        self.user_interaction.display_info(&format!(
            "Starting MapReduce execution with {} items, max {} parallel agents",
            work_items.len(),
            map_phase.config.max_parallel
        ));

        // Create a new job with persistent state
        let job_id = self
            .state_manager
            .create_job(
                map_phase.config.clone(),
                work_items.clone(),
                map_phase.agent_template.clone(),
                reduce_phase.map(|r| r.commands.clone()),
            )
            .await?;

        debug!("Created MapReduce job with ID: {}", job_id);

        // Initialize Dead Letter Queue for this job using global storage
        self.dlq = Some(Arc::new(
            match crate::storage::create_global_dlq(
                &self.project_root,
                &job_id,
                Some(self.event_logger.clone()),
            )
            .await
            {
                Ok(dlq) => {
                    info!("Using global DLQ storage for job: {}", job_id);
                    dlq
                }
                Err(e) => {
                    warn!(
                        "Failed to create global DLQ: {}, using temp directory fallback",
                        e
                    );
                    // Fallback to temp directory
                    let dlq_path = std::env::temp_dir().join("prodigy_dlq");
                    DeadLetterQueue::new(
                        job_id.clone(),
                        dlq_path,
                        1000, // Max 1000 items in DLQ
                        30,   // 30 days retention
                        Some(self.event_logger.clone()),
                    )
                    .await
                    .map_err(|e| MapReduceError::JobInitializationFailed {
                        job_id: job_id.clone(),
                        reason: format!("Failed to create DLQ: {}", e),
                        source: None,
                    })?
                }
            },
        ));

        // Log job started event
        self.event_logger
            .log(MapReduceEvent::JobStarted {
                job_id: job_id.clone(),
                config: map_phase.config.clone(),
                total_items: work_items.len(),
                timestamp: Utc::now(),
            })
            .await
            .unwrap_or_else(|e| warn!("Failed to log job started event: {}", e));

        // Initialize enhanced progress tracker if enabled
        if self.enable_web_dashboard {
            let mut tracker = EnhancedProgressTracker::new(job_id.clone(), work_items.len());

            // Start web dashboard on port 8080
            if let Err(e) = tracker.start_web_server(8080).await {
                warn!("Failed to start progress web server: {}", e);
            }

            self.enhanced_progress_tracker = Some(Arc::new(tracker));
        }

        // Initialize new progress tracker with rich display
        let display_mode = match std::env::var("PRODIGY_PROGRESS_MODE") {
            Ok(mode) => match mode.to_lowercase().as_str() {
                "rich" => DisplayMode::Rich,
                "simple" => DisplayMode::Simple,
                "json" => DisplayMode::Json,
                "none" => DisplayMode::None,
                _ => DisplayMode::Rich,
            },
            Err(_) => DisplayMode::Rich,
        };

        let progress_display = Box::new(MultiProgressDisplay::new(display_mode));
        let new_tracker = NewProgressTracker::new(
            job_id.clone(),
            "MapReduce Workflow".to_string(),
            progress_display,
        );

        // Start the workflow with total steps
        let total_steps = if reduce_phase.is_some() { 2 } else { 1 };
        new_tracker.start_workflow(total_steps).await.ok();

        self.new_progress_tracker = Some(Arc::new(new_tracker));

        // Execute map phase with state tracking
        let map_results = self
            .execute_map_phase_with_state(&job_id, map_phase, work_items, env)
            .await?;

        // Execute reduce phase if specified AND there were items to process
        // Skip reduce if no items were processed or all failed
        if let Some(reduce_phase) = reduce_phase {
            if map_results.is_empty() {
                self.user_interaction.display_warning(
                    "⚠️ Skipping reduce phase: no items were processed in map phase",
                );
            } else {
                let successful_count = map_results
                    .iter()
                    .filter(|r| matches!(r.status, AgentStatus::Success))
                    .count();

                if successful_count == 0 {
                    self.user_interaction
                        .display_warning("⚠️ Skipping reduce phase: all map agents failed");
                } else {
                    // Mark reduce phase as started
                    self.state_manager.start_reduce_phase(&job_id).await?;

                    self.execute_reduce_phase(reduce_phase, &map_results, env)
                        .await?;

                    // Mark reduce phase as completed
                    self.state_manager
                        .complete_reduce_phase(&job_id, None)
                        .await?;
                }
            }
        }

        // Mark job as complete
        self.state_manager.mark_job_complete(&job_id).await?;

        // Report summary
        let duration = start_time.elapsed();
        self.report_summary(&map_results, duration);

        // Log job completion event
        let success_count = map_results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .count();
        let failure_count = map_results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Failed(_)))
            .count();

        self.event_logger
            .log(MapReduceEvent::JobCompleted {
                job_id: job_id.clone(),
                duration: chrono::Duration::from_std(duration)
                    .unwrap_or(chrono::Duration::seconds(0)),
                success_count,
                failure_count,
            })
            .await
            .unwrap_or_else(|e| warn!("Failed to log job completed event: {}", e));

        // Report DLQ statistics if any items were added
        if let Some(dlq) = &self.dlq {
            if let Ok(stats) = dlq.get_stats().await {
                if stats.total_items > 0 {
                    self.user_interaction.display_warning(&format!(
                        "Dead Letter Queue: {} items failed permanently (run 'prodigy dlq list' to view)",
                        stats.total_items
                    ));
                }
            }
        }

        Ok(map_results)
    }

    /// Execute map phase with state tracking
    async fn execute_map_phase_with_state(
        &self,
        job_id: &str,
        map_phase: &MapPhase,
        work_items: Vec<Value>,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        // Execute the normal map phase with enhanced progress tracking
        let results = if let Some(ref tracker) = self.enhanced_progress_tracker {
            self.execute_map_phase_with_enhanced_progress(
                map_phase,
                work_items,
                env,
                tracker.clone(),
            )
            .await?
        } else {
            self.execute_map_phase(map_phase, work_items, env).await?
        };

        // Update state for each result
        for result in &results {
            self.state_manager
                .update_agent_result(job_id, result.clone())
                .await?;
        }

        Ok(results)
    }

    /// Resume a MapReduce job from checkpoint with options
    pub async fn resume_job_with_options(
        &self,
        job_id: &str,
        options: ResumeOptions,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<ResumeResult> {
        // Load job state from checkpoint (specific version if requested)
        let state = if let Some(version) = options.from_checkpoint {
            self.state_manager
                .get_job_state_from_checkpoint(job_id, Some(version))
                .await?
        } else {
            self.state_manager.get_job_state(job_id).await?
        };

        // Validate checkpoint integrity unless skipped
        if !options.skip_validation {
            self.validate_checkpoint(&state)?;
        }

        // Clean up orphaned worktrees from failed agents
        let worktree_names: Vec<String> = state
            .failed_agents
            .values()
            .filter_map(|failure| failure.worktree_info.as_ref().map(|info| info.name.clone()))
            .collect();
        self.resource_manager
            .cleanup_orphaned_resources(&worktree_names)
            .await;

        // Check if job is already complete and not forcing
        if state.is_complete && !options.force {
            return Ok(ResumeResult {
                job_id: job_id.to_string(),
                resumed_from_version: state.checkpoint_version,
                total_items: state.total_items,
                already_completed: state.completed_agents.len(),
                remaining_items: 0,
                final_results: state.agent_results.into_values().collect(),
            });
        }

        self.user_interaction.display_info(&format!(
            "Resuming MapReduce job {} from checkpoint v{}",
            job_id, state.checkpoint_version
        ));

        // Display progress information
        self.user_interaction.display_info(&format!(
            "Progress: {} completed, {} failed, {} pending",
            state.successful_count,
            state.failed_count,
            state.pending_items.len()
        ));

        let already_completed = state.completed_agents.len();
        let mut final_results: Vec<AgentResult>;
        let mut remaining_count = 0;

        // Log job resumed event
        self.event_logger
            .log(MapReduceEvent::JobResumed {
                job_id: job_id.to_string(),
                checkpoint_version: state.checkpoint_version,
                pending_items: state.pending_items.len(),
            })
            .await
            .unwrap_or_else(|e| log::warn!("Failed to log job resumed event: {}", e));

        // Check if map phase is complete
        if !state.is_map_phase_complete() {
            // Calculate pending items
            let pending_items =
                self.calculate_pending_items(&state, options.max_additional_retries)?;
            remaining_count = pending_items.len();

            if !pending_items.is_empty() {
                self.user_interaction.display_info(&format!(
                    "Resuming map phase with {} remaining items",
                    pending_items.len()
                ));

                // Create a map phase config from the stored state
                let map_phase = MapPhase {
                    config: state.config.clone(),
                    agent_template: state.agent_template.clone(),
                    filter: None,
                    sort_by: None,
                    distinct: None,
                };

                // Execute remaining items
                let new_results = self
                    .execute_map_phase(&map_phase, pending_items, env)
                    .await?;

                // Update state with new results
                for result in &new_results {
                    self.state_manager
                        .update_agent_result(job_id, result.clone())
                        .await?;
                }

                // Combine with existing results
                final_results = state.agent_results.into_values().collect();
                final_results.extend(new_results);
            } else {
                final_results = state.agent_results.into_values().collect();
            }
        } else {
            // Map phase is complete
            final_results = state.agent_results.into_values().collect();

            // Check if reduce phase needs to be executed
            if let Some(reduce_commands) = &state.reduce_commands {
                if state.reduce_phase_state.is_none()
                    || (state
                        .reduce_phase_state
                        .as_ref()
                        .is_some_and(|s| !s.started))
                {
                    self.user_interaction
                        .display_info("Map phase complete, executing pending reduce phase");

                    // Create reduce phase from stored commands
                    let reduce_phase = ReducePhase {
                        commands: reduce_commands.clone(),
                    };

                    // Mark reduce phase as started
                    self.state_manager.start_reduce_phase(job_id).await?;

                    // Execute reduce phase
                    self.execute_reduce_phase(&reduce_phase, &final_results, env)
                        .await?;

                    // Mark reduce phase as completed
                    self.state_manager
                        .complete_reduce_phase(job_id, None)
                        .await?;

                    self.user_interaction
                        .display_success("Reduce phase completed successfully");
                }
            } else if state.reduce_phase_state.is_none() {
                self.user_interaction
                    .display_info("Map phase complete, no reduce phase configured");
            }
        }

        Ok(ResumeResult {
            job_id: job_id.to_string(),
            resumed_from_version: state.checkpoint_version,
            total_items: state.total_items,
            already_completed,
            remaining_items: remaining_count,
            final_results,
        })
    }

    /// Resume a MapReduce job from checkpoint (backward compatibility)
    pub async fn resume_job(
        &self,
        job_id: &str,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        // Use the new method with default options for backward compatibility
        let result = self
            .resume_job_with_options(job_id, ResumeOptions::default(), env)
            .await?;
        Ok(result.final_results)
    }

    /// Validate checkpoint integrity
    fn validate_checkpoint(&self, state: &MapReduceJobState) -> MapReduceResult<()> {
        // Convert MapReduceJobState to the new JobState format for validation
        let job_state = state::JobState {
            id: state.job_id.clone(),
            phase: if state.is_complete {
                state::PhaseType::Completed
            } else if state
                .reduce_phase_state
                .as_ref()
                .map(|r| r.started)
                .unwrap_or(false)
            {
                state::PhaseType::Reduce
            } else if state.setup_completed {
                state::PhaseType::Map
            } else {
                state::PhaseType::Setup
            },
            checkpoint: None,
            processed_items: state.completed_agents.clone(),
            failed_items: state.pending_items.clone(),
            variables: state.variables.clone(),
            created_at: state.started_at,
            updated_at: state.updated_at,
            config: state.config.clone(),
            agent_results: state.agent_results.clone(),
            is_complete: state.is_complete,
            total_items: state.total_items,
        };

        // Use the enhanced state manager's validation
        self.enhanced_state_manager
            .validate_checkpoint(&job_state)
            .map_err(|e| {
                let context = self.create_error_context("checkpoint_validation");
                MapReduceError::CheckpointCorrupted {
                    job_id: state.job_id.clone(),
                    version: state.checkpoint_version,
                    details: e.to_string(),
                }
                .with_context(context)
                .error
            })
    }

    /// Calculate pending items for resumption
    fn calculate_pending_items(
        &self,
        state: &MapReduceJobState,
        max_additional_retries: u32,
    ) -> MapReduceResult<Vec<Value>> {
        // Convert MapReduceJobState to the new JobState format for calculation
        let mut job_state = state::JobState {
            id: state.job_id.clone(),
            phase: if state.is_complete {
                state::PhaseType::Completed
            } else if state
                .reduce_phase_state
                .as_ref()
                .map(|r| r.started)
                .unwrap_or(false)
            {
                state::PhaseType::Reduce
            } else if state.setup_completed {
                state::PhaseType::Map
            } else {
                state::PhaseType::Setup
            },
            checkpoint: None,
            processed_items: state.completed_agents.clone(),
            failed_items: Vec::new(),
            variables: state.variables.clone(),
            created_at: state.started_at,
            updated_at: state.updated_at,
            config: state.config.clone(),
            agent_results: state.agent_results.clone(),
            is_complete: state.is_complete,
            total_items: state.total_items,
        };

        // Add failed items to the state
        for item_id in state.failed_agents.keys() {
            job_state.failed_items.push(item_id.clone());
        }

        // Use the enhanced state manager's calculation
        // We need to provide work items, so let's use the original implementation for now
        let mut pending_items = Vec::new();

        // Add never-attempted items
        for (i, item) in state.work_items.iter().enumerate() {
            let item_id = format!("item_{}", i);
            if !state.completed_agents.contains(&item_id)
                && !state.failed_agents.contains_key(&item_id)
            {
                pending_items.push(item.clone());
            }
        }

        // Add retriable failed items
        let max_retries = max_additional_retries;
        for (item_id, failure) in &state.failed_agents {
            if failure.attempts < max_retries {
                if let Some(idx) = item_id
                    .strip_prefix("item_")
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    if idx < state.work_items.len() {
                        pending_items.push(state.work_items[idx].clone());
                    }
                }
            }
        }

        Ok(pending_items)
    }

    /// Check if a job can be resumed
    pub async fn can_resume_job(&self, job_id: &str) -> bool {
        match self.state_manager.get_job_state(job_id).await {
            Ok(state) => !state.is_complete,
            Err(_) => false,
        }
    }

    /// List resumable jobs
    pub async fn list_resumable_jobs(&self) -> MapReduceResult<Vec<String>> {
        let jobs = self
            .state_manager
            .list_resumable_jobs()
            .await
            .map_err(|e| MapReduceError::General {
                message: format!("Failed to list resumable jobs: {}", e),
                source: None,
            })?;
        Ok(jobs.into_iter().map(|job| job.job_id).collect())
    }

    /// Load work items from input source (command or JSON file) with pipeline processing
    async fn load_work_items_with_pipeline(
        &self,
        config: &MapReduceConfig,
        filter: &Option<String>,
        sort_by: &Option<String>,
        distinct: &Option<String>,
    ) -> MapReduceResult<Vec<Value>> {
        // Detect input source type using the project root as base
        let input_source = InputSource::detect_with_base(&config.input, &self.project_root);

        let items = match input_source {
            InputSource::Command(ref cmd) => {
                // Execute command to get work items
                info!("Executing command for work items: {}", cmd);

                // Use subprocess manager with timeout
                let timeout = Duration::from_secs(600); // Default 10 minute timeout
                InputSource::execute_command(cmd, timeout, &self.subprocess).await?
            }
            InputSource::JsonFile(ref path) => {
                // Load JSON file and process with pipeline
                let json = InputSource::load_json_file(path, &self.project_root).await?;

                debug!("Loaded JSON file: {}", path);

                // Debug: Show the top-level structure
                if let Value::Object(ref map) = json {
                    let keys: Vec<_> = map.keys().cloned().collect();
                    debug!("JSON top-level keys: {:?}", keys);
                }

                // Use data pipeline for extraction, filtering, and sorting
                let json_path = if config.json_path.is_empty() {
                    None
                } else {
                    Some(config.json_path.clone())
                };

                // Create pipeline with all configuration options
                let pipeline = DataPipeline::from_full_config(
                    json_path.clone(),
                    filter.clone(),
                    sort_by.clone(),
                    config.max_items,
                    config.offset,
                    distinct.clone(),
                )?;

                // Debug: Show what JSON path will be used
                if let Some(ref path) = json_path {
                    debug!("Using JSON path expression: {}", path);
                } else {
                    debug!("No JSON path specified, treating input as array or single item");
                }

                pipeline.process(&json)?
            }
        };

        debug!(
            "Loaded {} work items after pipeline processing",
            items.len()
        );

        Ok(items)
    }

    /// Execute the map phase with parallel agents
    async fn execute_map_phase(
        &self,
        map_phase: &MapPhase,
        work_items: Vec<Value>,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        let total_items = work_items.len();

        // If there are no items to process, return empty results
        if total_items == 0 {
            self.user_interaction
                .display_warning("No items to process in map phase");
            return Ok(Vec::new());
        }

        let max_parallel = map_phase.config.max_parallel.min(total_items);

        // Create progress tracker and start timer
        let mut progress_tracker = ProgressTracker::new(total_items, max_parallel);
        progress_tracker.start_timer();
        let progress = Arc::new(progress_tracker);

        // Create channels for work distribution (ensure buffer is at least 1)
        let (work_tx, work_rx) = mpsc::channel::<(usize, Value)>(total_items.max(1));
        let work_rx = Arc::new(RwLock::new(work_rx));

        // Send all work items to the queue
        for (index, item) in work_items.into_iter().enumerate() {
            work_tx.send((index, item)).await.map_err(|e| {
                let context = self.create_error_context("map_phase_execution");
                MapReduceError::General {
                    message: format!("Failed to send work item to queue: {}", e),
                    source: None,
                }
                .with_context(context)
                .error
            })?;
        }
        drop(work_tx); // Close the sender

        // Results collection
        let results = Arc::new(RwLock::new(Vec::new()));

        // Spawn worker tasks
        let mut workers = Vec::new();
        for agent_index in 0..max_parallel {
            let work_rx = work_rx.clone();
            let results = results.clone();
            let progress = progress.clone();
            let map_phase = map_phase.clone();
            let env = env.clone();
            let executor = self.clone_executor();

            let handle: JoinHandle<MapReduceResult<()>> = tokio::spawn(async move {
                executor
                    .run_agent(agent_index, work_rx, results, progress, map_phase, env)
                    .await
            });

            workers.push(handle);
        }

        // Wait for all workers to complete
        for worker in workers {
            match worker.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    self.user_interaction
                        .display_warning(&format!("Worker error: {}", e));
                }
                Err(join_err) => {
                    let context = self.create_error_context("map_phase_execution");
                    return Err(MapReduceError::General {
                        message: format!("Worker task panicked: {}", join_err),
                        source: None,
                    }
                    .with_context(context)
                    .error);
                }
            }
        }

        // Finish progress tracking
        progress.finish("Map phase completed");

        // Return collected results
        let results = results.read().await;
        Ok(results.clone())
    }

    /// Execute map phase with enhanced progress tracking
    async fn execute_map_phase_with_enhanced_progress(
        &self,
        map_phase: &MapPhase,
        work_items: Vec<Value>,
        env: &ExecutionEnvironment,
        tracker: Arc<EnhancedProgressTracker>,
    ) -> MapReduceResult<Vec<AgentResult>> {
        let total_items = work_items.len();

        // If there are no items to process, return empty results
        if total_items == 0 {
            self.user_interaction
                .display_warning("No items to process in map phase");
            return Ok(Vec::new());
        }

        let max_parallel = map_phase.config.max_parallel.min(total_items);

        // Create channels for work distribution (ensure buffer is at least 1)
        let (work_tx, work_rx) = mpsc::channel::<(usize, Value)>(total_items.max(1));
        let work_rx = Arc::new(RwLock::new(work_rx));

        // Send all work items to the queue
        for (index, item) in work_items.into_iter().enumerate() {
            work_tx.send((index, item)).await.map_err(|e| {
                let context = self.create_error_context("map_phase_execution");
                MapReduceError::General {
                    message: format!("Failed to send work item to queue: {}", e),
                    source: None,
                }
                .with_context(context)
                .error
            })?;
        }
        drop(work_tx); // Close the sender

        // Results collection
        let results = Arc::new(RwLock::new(Vec::new()));

        // Spawn worker tasks with enhanced progress tracking
        let mut workers = Vec::new();
        for agent_index in 0..max_parallel {
            let work_rx = work_rx.clone();
            let results = results.clone();
            let tracker = tracker.clone();
            let map_phase = map_phase.clone();
            let env = env.clone();
            let executor = self.clone_executor();

            let handle: JoinHandle<MapReduceResult<()>> = tokio::spawn(async move {
                executor
                    .run_agent_with_enhanced_progress(
                        agent_index,
                        work_rx,
                        results,
                        tracker,
                        map_phase,
                        env,
                    )
                    .await
            });

            workers.push(handle);
        }

        // Optional: Start CLI progress viewer in separate task
        if !self.enable_web_dashboard {
            let tracker_clone = tracker.clone();
            tokio::spawn(async move {
                let viewer = CLIProgressViewer::new(tracker_clone);
                let _ = viewer.display().await;
            });
        }

        // Wait for all workers to complete
        for worker in workers {
            match worker.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    self.user_interaction
                        .display_warning(&format!("Worker error: {}", e));
                }
                Err(join_err) => {
                    let context = self.create_error_context("map_phase_execution");
                    return Err(MapReduceError::General {
                        message: format!("Worker task panicked: {}", join_err),
                        source: None,
                    }
                    .with_context(context)
                    .error);
                }
            }
        }

        // Mark job as complete in tracker
        let _ = tracker.event_sender.send(ProgressUpdate {
            update_type: UpdateType::JobCompleted,
            timestamp: Utc::now(),
            data: json!({"job_id": tracker.job_id}),
        });

        // Return collected results
        let results = results.read().await;
        Ok(results.clone())
    }

    /// Run a single agent worker
    async fn run_agent(
        &self,
        agent_index: usize,
        work_rx: Arc<RwLock<mpsc::Receiver<(usize, Value)>>>,
        results: Arc<RwLock<Vec<AgentResult>>>,
        progress: Arc<ProgressTracker>,
        map_phase: MapPhase,
        env: ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        loop {
            // Get next work item
            let work_item = {
                let mut rx = work_rx.write().await;
                rx.recv().await
            };

            let Some((item_index, item)) = work_item else {
                // No more work
                progress
                    .update_agent_operation(agent_index, AgentOperation::Complete)
                    .await;
                break;
            };

            let item_id = Self::extract_item_identifier(&item, item_index);
            progress.update_agent(agent_index, &format!("Processing {}", &item_id));

            // Execute work item with retries
            let mut attempt = 0;
            let mut previous_error: Option<String> = None;
            let agent_start_time = Instant::now();
            let agent_result = loop {
                attempt += 1;

                if attempt > 1 {
                    progress
                        .update_agent_operation(
                            agent_index,
                            AgentOperation::Retrying(item_id.clone(), attempt),
                        )
                        .await;
                }

                let result = self
                    .execute_agent_commands_with_retry_info(
                        &item_id,
                        &item,
                        &map_phase.agent_template,
                        &env,
                        agent_index,
                        progress.clone(),
                        attempt,
                        previous_error.clone(),
                    )
                    .await;

                match result {
                    Ok(res) => break res,
                    Err(e) if attempt == 0 => {
                        // No automatic retries
                        // Save error for next attempt
                        previous_error = Some(e.to_string());
                        // Retry on failure
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                    Err(e) => {
                        // Final failure - add to DLQ
                        let agent_result = AgentResult {
                            item_id: item_id.clone(),
                            status: AgentStatus::Failed(e.to_string()),
                            output: None,
                            commits: vec![],
                            duration: agent_start_time.elapsed(),
                            error: Some(e.to_string()),
                            worktree_path: None,
                            branch_name: None,
                            worktree_session_id: None,
                            files_modified: vec![],
                        };

                        // Log error event with correlation ID
                        self.event_logger
                            .log(MapReduceEvent::AgentFailed {
                                job_id: env.session_id.clone(),
                                agent_id: format!("agent_{}", agent_index),
                                error: e.to_string(),
                                retry_eligible: false,
                            })
                            .await
                            .unwrap_or_else(|log_err| {
                                log::warn!("Failed to log agent error event: {}", log_err);
                            });

                        // Add to Dead Letter Queue
                        if let Some(dlq) = &self.dlq {
                            let failure_detail = FailureDetail {
                                attempt_number: attempt,
                                timestamp: Utc::now(),
                                error_type: ErrorType::Unknown,
                                error_message: e.to_string(),
                                stack_trace: None,
                                agent_id: format!("agent_{}", agent_index),
                                step_failed: "execute_agent_commands".to_string(),
                                duration_ms: 0,
                            };

                            let dlq_item = DeadLetteredItem {
                                item_id: item_id.clone(),
                                item_data: item.clone(),
                                first_attempt: Utc::now(),
                                last_attempt: Utc::now(),
                                failure_count: attempt,
                                failure_history: vec![failure_detail],
                                error_signature: DeadLetterQueue::create_error_signature(
                                    &ErrorType::Unknown,
                                    &e.to_string(),
                                ),
                                worktree_artifacts: None,
                                reprocess_eligible: true,
                                manual_review_required: false,
                            };

                            if let Err(dlq_err) = dlq.add(dlq_item).await {
                                error!("Failed to add item to DLQ: {}", dlq_err);
                            } else {
                                info!("Added failed item {} to Dead Letter Queue", item_id);
                            }
                        }

                        break agent_result;
                    }
                }
            };

            // Store result
            {
                let mut res = results.write().await;
                res.push(agent_result);
            }

            // Update progress
            progress.complete_item();
        }

        Ok(())
    }

    /// Run a single agent worker with enhanced progress tracking
    async fn run_agent_with_enhanced_progress(
        &self,
        agent_index: usize,
        work_rx: Arc<RwLock<mpsc::Receiver<(usize, Value)>>>,
        results: Arc<RwLock<Vec<AgentResult>>>,
        tracker: Arc<EnhancedProgressTracker>,
        map_phase: MapPhase,
        env: ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        loop {
            // Get next work item
            let work_item = {
                let mut rx = work_rx.write().await;
                rx.recv().await
            };

            let Some((item_index, item)) = work_item else {
                // No more work
                let agent_id = format!("agent_{}", agent_index);
                tracker
                    .update_agent_state(&agent_id, ProgressAgentState::Completed)
                    .await?;
                break;
            };

            let item_id = Self::extract_item_identifier(&item, item_index);
            let agent_id = format!("agent_{}", agent_index);

            // Initialize agent progress
            let agent_progress = AgentProgress {
                agent_id: agent_id.clone(),
                item_id: item_id.clone(),
                state: ProgressAgentState::Initializing,
                current_step: "Starting".to_string(),
                steps_completed: 0,
                total_steps: map_phase.agent_template.len(),
                progress_percentage: 0.0,
                started_at: Utc::now(),
                last_update: Utc::now(),
                estimated_completion: None,
                error_count: 0,
                retry_count: 0,
            };
            tracker
                .update_agent_progress(&agent_id, agent_progress)
                .await?;

            // Execute work item with retries
            let mut attempt = 0;
            let mut previous_error: Option<String> = None;
            let agent_result = loop {
                attempt += 1;

                if attempt > 1 {
                    tracker
                        .update_agent_state(&agent_id, ProgressAgentState::Retrying { attempt })
                        .await?;
                }

                let start_time = Instant::now();
                let result = self
                    .execute_agent_commands_with_progress_and_retry(
                        &item_id,
                        &item,
                        &map_phase.agent_template,
                        &env,
                        agent_index,
                        tracker.clone(),
                        attempt,
                        previous_error.clone(),
                    )
                    .await;

                match result {
                    Ok(mut res) => {
                        res.duration = start_time.elapsed();
                        tracker.mark_item_completed(&agent_id).await?;
                        break res;
                    }
                    Err(e) if attempt == 0 => {
                        // No automatic retries
                        // Save error for next attempt
                        previous_error = Some(e.to_string());
                        // Retry on failure
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                    Err(e) => {
                        // Final failure
                        tracker.mark_item_failed(&agent_id, e.to_string()).await?;

                        let agent_result = AgentResult {
                            item_id: item_id.clone(),
                            status: AgentStatus::Failed(e.to_string()),
                            output: None,
                            commits: vec![],
                            duration: start_time.elapsed(),
                            error: Some(e.to_string()),
                            worktree_path: None,
                            branch_name: None,
                            worktree_session_id: None,
                            files_modified: vec![],
                        };

                        // Add to Dead Letter Queue
                        if let Some(dlq) = &self.dlq {
                            let failure_detail = FailureDetail {
                                attempt_number: attempt,
                                timestamp: Utc::now(),
                                error_type: ErrorType::Unknown,
                                error_message: e.to_string(),
                                stack_trace: None,
                                agent_id: agent_id.clone(),
                                step_failed: "execute_agent_commands".to_string(),
                                duration_ms: start_time.elapsed().as_millis() as u64,
                            };

                            let dlq_item = DeadLetteredItem {
                                item_id: item_id.clone(),
                                item_data: item.clone(),
                                first_attempt: Utc::now(),
                                last_attempt: Utc::now(),
                                failure_count: attempt,
                                failure_history: vec![failure_detail],
                                error_signature: DeadLetterQueue::create_error_signature(
                                    &ErrorType::Unknown,
                                    &e.to_string(),
                                ),
                                worktree_artifacts: None,
                                reprocess_eligible: true,
                                manual_review_required: false,
                            };

                            if let Err(dlq_err) = dlq.add(dlq_item).await {
                                error!("Failed to add item to DLQ: {}", dlq_err);
                            }
                        }

                        break agent_result;
                    }
                }
            };

            // Store result
            {
                let mut res = results.write().await;
                res.push(agent_result);
            }
        }

        Ok(())
    }

    /// Execute commands for a single agent
    /// Extract variables from item data for context
    fn extract_item_variables(item: &Value) -> HashMap<String, String> {
        let mut variables = HashMap::new();
        if let Value::Object(obj) = item {
            for (key, value) in obj {
                let str_value = match value {
                    Value::String(s) => s.clone(),
                    _ => value.to_string(),
                };
                variables.insert(key.clone(), str_value);
            }
        }
        variables
    }

    /// Create standard variables for agent context
    fn create_standard_variables(
        worktree_name: &str,
        item_id: &str,
        session_id: &str,
    ) -> HashMap<String, String> {
        let mut variables = HashMap::new();
        variables.insert("worktree".to_string(), worktree_name.to_string());
        variables.insert("item_id".to_string(), item_id.to_string());
        variables.insert(
            "session_id".to_string(),
            format!("{}-{}", session_id, item_id),
        );
        variables
    }

    /// Initialize agent context with all necessary variables
    fn initialize_agent_context(
        &self,
        item_id: &str,
        item: &Value,
        worktree_path: PathBuf,
        worktree_name: String,
        env: &ExecutionEnvironment,
    ) -> AgentContext {
        let agent_env = ExecutionEnvironment {
            working_dir: worktree_path.clone(),
            project_dir: env.project_dir.clone(),
            worktree_name: Some(worktree_name.clone()),
            session_id: format!("{}-{}", env.session_id, item_id),
        };

        let mut context = AgentContext::new(
            item_id.to_string(),
            worktree_path,
            worktree_name.clone(),
            agent_env,
        );

        // Add item variables
        let item_vars = Self::extract_item_variables(item);
        context.variables.extend(item_vars);

        // Add standard variables
        let std_vars = Self::create_standard_variables(&worktree_name, item_id, &env.session_id);
        context.variables.extend(std_vars);

        // Add setup variables from setup phase
        context.variables.extend(self.setup_variables.clone());

        context
    }

    /// Initialize agent context with retry information
    #[allow(clippy::too_many_arguments)]
    fn initialize_agent_context_with_retry(
        &self,
        item_id: &str,
        item: &Value,
        worktree_path: PathBuf,
        worktree_name: String,
        env: &ExecutionEnvironment,
        attempt: u32,
        previous_error: Option<String>,
    ) -> AgentContext {
        let mut context =
            self.initialize_agent_context(item_id, item, worktree_path, worktree_name, env);

        // Set retry count for internal use
        context.retry_count = attempt - 1; // Convert attempt number to retry count (0-based)

        // Add retry-related variables for interpolation
        context
            .variables
            .insert("item.attempt".to_string(), attempt.to_string());
        if let Some(error) = previous_error {
            context
                .variables
                .insert("item.previous_error".to_string(), error);
        }

        context
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_agent_commands_with_retry_info(
        &self,
        item_id: &str,
        item: &Value,
        template_steps: &[WorkflowStep],
        env: &ExecutionEnvironment,
        agent_index: usize,
        progress: Arc<ProgressTracker>,
        attempt: u32,
        previous_error: Option<String>,
    ) -> MapReduceResult<AgentResult> {
        let start_time = Instant::now();
        let agent_id = generate_agent_id(agent_index, item_id);

        // Log that agent is starting
        info!(
            "Agent {} starting to process item: {} (attempt: {})",
            agent_index, item_id, attempt
        );
        self.user_interaction.display_progress(&format!(
            "Agent {} processing item: {} (attempt: {})",
            agent_index, item_id, attempt
        ));

        // Acquire worktree session with error handling
        let worktree_session = match self
            .resource_manager
            .acquire_worktree_session(&agent_id, env)
            .await
        {
            Ok(session) => session,
            Err(e) => {
                // Log failure asynchronously
                self.log_agent_failure_async(
                    env.session_id.clone(),
                    agent_id.clone(),
                    e.to_string(),
                );
                return Err(e);
            }
        };
        let worktree_name = worktree_session.name.clone();
        let worktree_path = worktree_session.path.clone();
        let worktree_session_id = worktree_name.clone();

        // Log agent started event
        self.event_logger
            .log(MapReduceEvent::AgentStarted {
                job_id: env.session_id.clone(),
                agent_id: agent_id.clone(),
                item_id: item_id.to_string(),
                worktree: worktree_name.clone(),
                attempt,
            })
            .await
            .unwrap_or_else(|e| log::warn!("Failed to log agent started event: {}", e));

        // Create branch name for this agent
        let branch_name = generate_agent_branch_name(&env.session_id, item_id);

        // Initialize agent context with retry information
        let mut context = self.initialize_agent_context_with_retry(
            item_id,
            item,
            worktree_path.clone(),
            worktree_name.clone(),
            env,
            attempt,
            previous_error,
        );

        // Execute template steps with real command execution
        let execution_result = self
            .execute_all_steps(
                template_steps,
                &mut context,
                item_id,
                agent_index,
                progress.clone(),
                &agent_id,
                env,
            )
            .await;

        let (total_output, execution_error) = execution_result;

        // Finalize and create result
        let result = self
            .finalize_agent_result(
                item_id,
                &worktree_path,
                &worktree_name,
                &branch_name,
                worktree_session_id,
                env,
                template_steps,
                execution_error,
                total_output,
                start_time,
            )
            .await?;

        // Log agent completed or failed event
        match &result.status {
            AgentStatus::Success => {
                // Convert commits to include agent_id
                let agent_commits: Vec<String> = result
                    .commits
                    .iter()
                    .map(|c| format!("[{}] {}", agent_id, c))
                    .collect();

                self.event_logger
                    .log(MapReduceEvent::AgentCompleted {
                        job_id: env.session_id.clone(),
                        agent_id: agent_id.clone(),
                        commits: agent_commits,
                        duration: chrono::Duration::from_std(result.duration)
                            .unwrap_or_else(|_| chrono::Duration::seconds(0)),
                    })
                    .await
                    .unwrap_or_else(|e| log::warn!("Failed to log agent completed event: {}", e));
            }
            AgentStatus::Failed(_) => {
                if let Some(err) = &result.error {
                    self.event_logger
                        .log(MapReduceEvent::AgentFailed {
                            job_id: env.session_id.clone(),
                            agent_id: agent_id.clone(),
                            error: err.clone(),
                            retry_eligible: attempt < 3, // Usually max 3 retries
                        })
                        .await
                        .unwrap_or_else(|e| log::warn!("Failed to log agent failed event: {}", e));
                }
            }
            _ => {
                // Other statuses (Pending, Running, Timeout, Retrying) don't need special logging
            }
        }

        Ok(result)
    }

    #[allow(dead_code)]
    async fn execute_agent_commands(
        &self,
        item_id: &str,
        item: &Value,
        template_steps: &[WorkflowStep],
        env: &ExecutionEnvironment,
        agent_index: usize,
        progress: Arc<ProgressTracker>,
    ) -> MapReduceResult<AgentResult> {
        let start_time = Instant::now();
        let agent_id = generate_agent_id(agent_index, item_id);

        // Log that agent is starting
        info!(
            "Agent {} starting to process item: {}",
            agent_index, item_id
        );
        self.user_interaction.display_progress(&format!(
            "Agent {} processing item: {}",
            agent_index, item_id
        ));

        // Acquire worktree session with error handling
        let worktree_session = match self
            .resource_manager
            .acquire_worktree_session(&agent_id, env)
            .await
        {
            Ok(session) => session,
            Err(e) => {
                // Log failure asynchronously
                self.log_agent_failure_async(
                    env.session_id.clone(),
                    agent_id.clone(),
                    e.to_string(),
                );
                return Err(e);
            }
        };
        let worktree_name = worktree_session.name.clone();
        let worktree_path = worktree_session.path.clone();
        let worktree_session_id = worktree_name.clone();

        // Log agent started event
        self.event_logger
            .log(MapReduceEvent::AgentStarted {
                job_id: env.session_id.clone(),
                agent_id: agent_id.clone(),
                item_id: item_id.to_string(),
                worktree: worktree_name.clone(),
                attempt: 1,
            })
            .await
            .unwrap_or_else(|e| log::warn!("Failed to log agent started event: {}", e));

        // Create branch name for this agent
        let branch_name = generate_agent_branch_name(&env.session_id, item_id);

        // Initialize agent context with all variables
        let mut context = self.initialize_agent_context(
            item_id,
            item,
            worktree_path.clone(),
            worktree_name.clone(),
            env,
        );

        // Execute template steps with real command execution
        let execution_result = self
            .execute_all_steps(
                template_steps,
                &mut context,
                item_id,
                agent_index,
                progress.clone(),
                &agent_id,
                env,
            )
            .await;

        let (total_output, execution_error) = execution_result;

        // Finalize and create result
        let result = self
            .finalize_agent_result(
                item_id,
                &worktree_path,
                &worktree_name,
                &branch_name,
                worktree_session_id,
                env,
                template_steps,
                execution_error,
                total_output,
                start_time,
            )
            .await?;

        // Log agent completed or failed event
        match &result.status {
            AgentStatus::Success => {
                // Convert commits to include agent_id
                let agent_commits: Vec<String> = result
                    .commits
                    .iter()
                    .map(|c| format!("{} (agent: {})", c, agent_id))
                    .collect();

                self.event_logger
                    .log(MapReduceEvent::AgentCompleted {
                        job_id: env.session_id.clone(),
                        agent_id: agent_id.clone(),
                        duration: chrono::Duration::from_std(start_time.elapsed())
                            .unwrap_or(chrono::Duration::seconds(0)),
                        commits: agent_commits,
                    })
                    .await
                    .unwrap_or_else(|e| log::warn!("Failed to log agent completed event: {}", e));
            }
            AgentStatus::Failed(error) => {
                self.event_logger
                    .log(MapReduceEvent::AgentFailed {
                        job_id: env.session_id.clone(),
                        agent_id: agent_id.clone(),
                        error: error.clone(),
                        retry_eligible: true,
                    })
                    .await
                    .unwrap_or_else(|e| log::warn!("Failed to log agent failed event: {}", e));
            }
            _ => {
                // For other statuses (Pending, Running, Timeout, Retrying), no specific event needed
                log::debug!("Agent {} status: {:?}", agent_id, result.status);
            }
        }

        Ok(result)
    }

    /// Execute agent commands with enhanced progress tracking and retry info
    #[allow(clippy::too_many_arguments)]
    async fn execute_agent_commands_with_progress_and_retry(
        &self,
        item_id: &str,
        item: &Value,
        template_steps: &[WorkflowStep],
        env: &ExecutionEnvironment,
        agent_index: usize,
        tracker: Arc<EnhancedProgressTracker>,
        attempt: u32,
        previous_error: Option<String>,
    ) -> MapReduceResult<AgentResult> {
        let start_time = Instant::now();
        let agent_id = format!("agent_{}", agent_index);

        // Create isolated worktree session for this agent
        let worktree_session = self.worktree_manager.create_session().await.map_err(|e| {
            let context = self.create_error_context("worktree_creation");
            MapReduceError::WorktreeCreationFailed {
                agent_id: agent_id.clone(),
                reason: e.to_string(),
                source: std::io::Error::other(e.to_string()),
            }
            .with_context(context)
            .error
        })?;

        let worktree_name = worktree_session.name.clone();
        let worktree_path = worktree_session.path.clone();
        let worktree_session_id = worktree_name.clone();

        // Create branch name for this agent
        let branch_name = generate_agent_branch_name(&env.session_id, item_id);

        // Initialize agent context with retry information
        let mut context = self.initialize_agent_context_with_retry(
            item_id,
            item,
            worktree_path.clone(),
            worktree_name.clone(),
            env,
            attempt,
            previous_error,
        );

        // Execute template steps with enhanced progress tracking
        let mut total_output = String::new();
        let mut execution_error = None;

        for (step_index, step) in template_steps.iter().enumerate() {
            // Update progress for current step
            let progress_percentage =
                ((step_index as f32 + 1.0) / template_steps.len() as f32) * 100.0;
            let step_name = step
                .name
                .clone()
                .unwrap_or_else(|| format!("Step {}", step_index + 1));

            tracker
                .update_agent_state(
                    &agent_id,
                    ProgressAgentState::Running {
                        step: step_name.clone(),
                        progress: progress_percentage,
                    },
                )
                .await?;

            // Execute the step (interpolation is handled internally)
            let result = self.execute_single_step(step, &mut context).await;

            match result {
                Ok(step_result) => {
                    if !step_result.stdout.is_empty() {
                        total_output.push_str(&step_result.stdout);
                        total_output.push('\n');
                    }
                }
                Err(e) => {
                    execution_error = Some(e.to_string());
                    break;
                }
            }
        }

        // Create result
        let result = self
            .finalize_agent_result(
                item_id,
                &worktree_path,
                &worktree_name,
                &branch_name,
                worktree_session_id,
                env,
                template_steps,
                execution_error,
                total_output,
                start_time,
            )
            .await?;

        Ok(result)
    }

    /// Execute agent commands with enhanced progress tracking
    #[allow(dead_code)]
    async fn execute_agent_commands_with_progress(
        &self,
        item_id: &str,
        item: &Value,
        template_steps: &[WorkflowStep],
        env: &ExecutionEnvironment,
        agent_index: usize,
        tracker: Arc<EnhancedProgressTracker>,
    ) -> MapReduceResult<AgentResult> {
        let start_time = Instant::now();
        let agent_id = format!("agent_{}", agent_index);

        // Create isolated worktree session for this agent
        let worktree_session = self.worktree_manager.create_session().await.map_err(|e| {
            let context = self.create_error_context("worktree_creation");
            MapReduceError::WorktreeCreationFailed {
                agent_id: agent_id.clone(),
                reason: e.to_string(),
                source: std::io::Error::other(e.to_string()),
            }
            .with_context(context)
            .error
        })?;

        let worktree_name = worktree_session.name.clone();
        let worktree_path = worktree_session.path.clone();
        let worktree_session_id = worktree_name.clone();

        // Create branch name for this agent
        let branch_name = generate_agent_branch_name(&env.session_id, item_id);

        // Initialize agent context with all variables
        let mut context = self.initialize_agent_context(
            item_id,
            item,
            worktree_path.clone(),
            worktree_name.clone(),
            env,
        );

        // Execute template steps with enhanced progress tracking
        let mut total_output = String::new();
        let mut execution_error = None;

        for (step_index, step) in template_steps.iter().enumerate() {
            // Update progress for current step
            let progress_percentage =
                ((step_index as f32 + 1.0) / template_steps.len() as f32) * 100.0;
            let step_name = step
                .name
                .clone()
                .unwrap_or_else(|| format!("Step {}", step_index + 1));

            let agent_progress = AgentProgress {
                agent_id: agent_id.clone(),
                item_id: item_id.to_string(),
                state: ProgressAgentState::Running {
                    step: step_name.clone(),
                    progress: progress_percentage,
                },
                current_step: step_name.clone(),
                steps_completed: step_index,
                total_steps: template_steps.len(),
                progress_percentage,
                started_at: Utc::now(),
                last_update: Utc::now(),
                estimated_completion: None,
                error_count: 0,
                retry_count: 0,
            };

            tracker
                .update_agent_progress(&agent_id, agent_progress)
                .await?;

            // Log the step being executed
            let step_display = if let Some(claude_cmd) = &step.claude {
                format!("claude: {}", claude_cmd)
            } else if let Some(shell_cmd) = &step.shell {
                format!("shell: {}", shell_cmd)
            } else {
                step_name.clone()
            };
            info!(
                "Agent {} executing step {}/{}: {}",
                agent_id,
                step_index + 1,
                template_steps.len(),
                step_display
            );

            // Execute the step
            let result = self.execute_single_step(step, &mut context).await;

            match result {
                Ok(step_result) => {
                    if !step_result.stdout.is_empty() {
                        total_output.push_str(&step_result.stdout);
                        total_output.push('\n');
                    }
                    context.update_with_output(Some(step_result.stdout));
                }
                Err(e) => {
                    execution_error = Some(e.to_string());
                    break;
                }
            }
        }

        // Finalize and create result
        let result = self
            .finalize_agent_result(
                item_id,
                &worktree_path,
                &worktree_name,
                &branch_name,
                worktree_session_id,
                env,
                template_steps,
                execution_error,
                total_output,
                start_time,
            )
            .await?;

        Ok(result)
    }

    /// Classify the operation type of a step for progress tracking
    fn classify_step_operation(step: &WorkflowStep) -> AgentOperation {
        if let Some(claude_cmd) = &step.claude {
            AgentOperation::Claude(claude_cmd.clone())
        } else if let Some(shell_cmd) = &step.shell {
            AgentOperation::Shell(shell_cmd.clone())
        } else if let Some(test) = &step.test {
            AgentOperation::Test(test.command.clone())
        } else if let Some(handler) = &step.handler {
            AgentOperation::Handler(handler.name.clone())
        } else {
            AgentOperation::Setup(step.name.clone().unwrap_or_else(|| "step".to_string()))
        }
    }

    /// Execute all steps for an agent
    #[allow(clippy::too_many_arguments)]
    async fn execute_all_steps(
        &self,
        template_steps: &[WorkflowStep],
        context: &mut AgentContext,
        item_id: &str,
        agent_index: usize,
        progress: Arc<ProgressTracker>,
        agent_id: &str,
        env: &ExecutionEnvironment,
    ) -> (String, Option<String>) {
        let mut total_output = String::new();
        let mut execution_error: Option<String> = None;

        for (step_index, step) in template_steps.iter().enumerate() {
            debug!(
                "Executing step {} for agent {}: {:?}",
                step_index + 1,
                item_id,
                step.name
            );

            // Update agent operation
            let operation = Self::classify_step_operation(step);
            progress
                .update_agent_operation(agent_index, operation)
                .await;

            // Log agent progress event
            let step_name = step
                .name
                .clone()
                .unwrap_or_else(|| format!("step_{}", step_index + 1));
            let progress_pct = ((step_index as f32 + 0.5) / template_steps.len() as f32) * 100.0;
            self.event_logger
                .log(MapReduceEvent::AgentProgress {
                    job_id: env.session_id.clone(),
                    agent_id: agent_id.to_string(),
                    step: step_name.clone(),
                    progress_pct,
                })
                .await
                .unwrap_or_else(|e| log::warn!("Failed to log agent progress event: {}", e));

            // Execute the step and handle result
            let step_result = self
                .execute_step_with_handlers(step, context, item_id, step_index)
                .await;

            match step_result {
                Ok((result, should_continue)) => {
                    // Update context and accumulate output
                    self.update_context_from_step(context, &result, step_index);
                    total_output.push_str(&self.format_step_output(&result, step, step_index));

                    // Handle success case
                    if result.success {
                        if let Some(on_success) = &step.on_success {
                            self.execute_success_handler(on_success, context, item_id, step_index)
                                .await;
                        }
                    }

                    if !should_continue {
                        execution_error = Some(format!(
                            "Step {} failed and workflow should stop",
                            step_index + 1
                        ));
                        break;
                    }
                }
                Err(error) => {
                    execution_error = Some(error.to_string());
                    break;
                }
            }
        }

        (total_output, execution_error)
    }

    /// Execute a single step with error handlers
    async fn execute_step_with_handlers(
        &self,
        step: &WorkflowStep,
        context: &mut AgentContext,
        item_id: &str,
        step_index: usize,
    ) -> MapReduceResult<(StepResult, bool)> {
        match self.execute_single_step(step, context).await {
            Ok(result) => Ok((result, true)),
            Err(e) => {
                let error_msg = format!("Step {} failed: {}", step_index + 1, e);
                error!("Agent {} error: {}", item_id, error_msg);

                if let Some(on_failure) = &step.on_failure {
                    info!("Executing on_failure handler for agent {}", item_id);
                    let handled = self
                        .handle_on_failure(on_failure, step, context, error_msg.clone())
                        .await?;

                    let failed_result = StepResult {
                        success: false,
                        exit_code: Some(1),
                        stdout: String::new(),
                        stderr: e.to_string(),
                    };

                    Ok((failed_result, handled))
                } else {
                    let context = self.create_error_context("execute_all_steps");
                    Err(MapReduceError::General {
                        message: error_msg,
                        source: None,
                    }
                    .with_context(context)
                    .error)
                }
            }
        }
    }

    /// Update context from step result
    fn update_context_from_step(
        &self,
        context: &mut AgentContext,
        result: &StepResult,
        step_index: usize,
    ) {
        if !result.stdout.is_empty() {
            context.update_with_output(Some(result.stdout.clone()));
            context.variables.insert(
                format!("step{}.output", step_index + 1),
                result.stdout.clone(),
            );
        }
    }

    /// Format step output for display
    fn format_step_output(
        &self,
        result: &StepResult,
        step: &WorkflowStep,
        step_index: usize,
    ) -> String {
        format!(
            "=== Step {} ({}) ===\n{}\n",
            step_index + 1,
            step.name.as_deref().unwrap_or("unnamed"),
            result.stdout
        )
    }

    /// Execute success handler for a step
    async fn execute_success_handler(
        &self,
        on_success: &WorkflowStep,
        context: &mut AgentContext,
        item_id: &str,
        step_index: usize,
    ) {
        debug!(
            "Executing on_success handler for agent {} step {}",
            item_id,
            step_index + 1
        );

        // Store output for handler
        if let Some(output) = context.shell_output.clone() {
            context
                .captured_outputs
                .insert("shell.output".to_string(), output.clone());
            context.variables.insert("shell.output".to_string(), output);
        }

        match self.execute_single_step(on_success, context).await {
            Ok(result) if !result.success => {
                warn!(
                    "on_success handler failed for agent {} step {}: {}",
                    item_id,
                    step_index + 1,
                    result.stderr
                );
            }
            Err(e) => {
                warn!(
                    "Failed to execute on_success handler for agent {} step {}: {}",
                    item_id,
                    step_index + 1,
                    e
                );
            }
            _ => {}
        }
    }

    /// Finalize agent result and handle merging/cleanup
    #[allow(clippy::too_many_arguments)]
    async fn finalize_agent_result(
        &self,
        item_id: &str,
        worktree_path: &Path,
        worktree_name: &str,
        branch_name: &str,
        worktree_session_id: String,
        env: &ExecutionEnvironment,
        template_steps: &[WorkflowStep],
        execution_error: Option<String>,
        total_output: String,
        start_time: Instant,
    ) -> MapReduceResult<AgentResult> {
        // Initialize CommitTracker for agent commit tracking
        let git_ops = Arc::new(crate::abstractions::RealGitOperations::new());
        let mut commit_tracker =
            crate::cook::commit_tracker::CommitTracker::new(git_ops, worktree_path.to_path_buf());
        commit_tracker.initialize().await.map_err(|e| {
            let context = self.create_error_context("commit_tracker_init");
            MapReduceError::General {
                message: format!("Failed to initialize commit tracker: {}", e),
                source: None,
            }
            .with_context(context)
            .error
        })?;

        // Get commits and modified files using lifecycle manager
        let commits = self
            .agent_lifecycle_manager
            .get_worktree_commits(worktree_path)
            .await
            .map_err(|e| {
                let context = self.create_error_context("get_worktree_commits");
                MapReduceError::General {
                    message: format!("Failed to get worktree commits: {}", e),
                    source: None,
                }
                .with_context(context)
                .error
            })?;

        let files_modified = self
            .agent_lifecycle_manager
            .get_modified_files(worktree_path)
            .await
            .map_err(|e| {
                let context = self.create_error_context("get_modified_files");
                MapReduceError::General {
                    message: format!("Failed to get modified files: {}", e),
                    source: None,
                }
                .with_context(context)
                .error
            })?;

        // Determine status
        let status = execution_error
            .clone()
            .map(AgentStatus::Failed)
            .unwrap_or(AgentStatus::Success);

        // Handle merge and cleanup using lifecycle manager
        let merge_result = self
            .agent_lifecycle_manager
            .handle_merge_and_cleanup(
                execution_error.is_none(),
                env,
                worktree_path,
                worktree_name,
                branch_name,
                template_steps,
                item_id,
            )
            .await
            .map_err(|e| {
                let context = self.create_error_context("handle_merge_and_cleanup");
                MapReduceError::General {
                    message: format!("Failed to handle merge and cleanup: {}", e),
                    source: None,
                }
                .with_context(context)
                .error
            })?;

        Ok(AgentResult {
            item_id: item_id.to_string(),
            status,
            output: Some(total_output),
            commits,
            files_modified,
            duration: start_time.elapsed(),
            error: execution_error,
            worktree_path: if merge_result {
                None
            } else {
                Some(worktree_path.to_path_buf())
            },
            branch_name: Some(branch_name.to_string()),
            worktree_session_id: if merge_result {
                None
            } else {
                Some(worktree_session_id)
            },
        })
    }

    // Git operations and branch management methods have been moved to the agent::lifecycle module

    /// Validate the parent worktree state after a merge
    #[allow(dead_code)]
    async fn validate_parent_state(&self, parent_path: &Path) -> MapReduceResult<()> {
        // Check that there are no merge conflicts
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(parent_path)
            .output()?;

        let status = String::from_utf8_lossy(&output.stdout);
        if status.contains("UU ") || status.contains("AA ") || status.contains("DD ") {
            let context = self.create_error_context("validate_parent_state");
            return Err(MapReduceError::General {
                message: "Unresolved merge conflicts detected in parent worktree".to_string(),
                source: None,
            }
            .with_context(context)
            .error);
        }

        // Run basic syntax check if it's a Rust project
        if parent_path.join("Cargo.toml").exists() {
            let check_output = Command::new("cargo")
                .args(["check", "--quiet"])
                .current_dir(parent_path)
                .output()?;

            if !check_output.status.success() {
                warn!("Parent worktree fails cargo check after merge, but continuing");
            }
        }

        Ok(())
    }

    /// Execute the reduce phase
    async fn execute_reduce_phase(
        &self,
        reduce_phase: &ReducePhase,
        map_results: &[AgentResult],
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        // Display start message and summary
        self.display_reduce_start(map_results);

        // Create and configure phase context
        let mut phase_context = self.create_reduce_phase_context(env, map_results);

        // Execute the reduce phase
        let result = self
            .run_reduce_executor(reduce_phase, &mut phase_context)
            .await?;

        // Process the result
        self.process_reduce_result(result)?;

        // Handle post-reduce worktree instructions
        self.display_worktree_instructions(env);

        Ok(())
    }

    /// Display reduce phase start message and summary
    fn display_reduce_start(&self, map_results: &[AgentResult]) {
        self.user_interaction
            .display_progress("Starting reduce phase...");
        let summary_stats = calculate_map_result_summary(map_results);
        self.display_reduce_summary(&summary_stats);
    }

    /// Create phase context for reduce execution
    fn create_reduce_phase_context(
        &self,
        env: &ExecutionEnvironment,
        map_results: &[AgentResult],
    ) -> phases::PhaseContext {
        let mut phase_context = phases::PhaseContext::new(env.clone(), self.subprocess.clone());
        phase_context.map_results = Some(map_results.to_vec());
        phase_context
    }

    /// Run the reduce phase executor
    async fn run_reduce_executor(
        &self,
        reduce_phase: &ReducePhase,
        phase_context: &mut phases::PhaseContext,
    ) -> MapReduceResult<phases::PhaseResult> {
        let reduce_executor = phases::ReducePhaseExecutor::new(reduce_phase.clone());
        reduce_executor.execute(phase_context).await.map_err(|e| {
            let context = self.create_error_context("reduce_phase_execution");
            MapReduceError::General {
                message: format!("Reduce phase failed: {}", e),
                source: None,
            }
            .with_context(context)
            .error
        })
    }

    /// Process the reduce phase result
    fn process_reduce_result(&self, result: phases::PhaseResult) -> MapReduceResult<()> {
        if result.success {
            self.user_interaction
                .display_success("Reduce phase completed successfully");
            Ok(())
        } else {
            Err(MapReduceError::General {
                message: result
                    .error_message
                    .unwrap_or_else(|| "Reduce phase failed".to_string()),
                source: None,
            })
        }
    }

    /// Display reduce phase summary statistics
    fn display_reduce_summary(&self, summary_stats: &utils::MapResultSummary) {
        self.user_interaction.display_info(&format!(
            "All {} successful agents merged to parent worktree",
            summary_stats.successful
        ));

        if summary_stats.failed > 0 {
            self.user_interaction.display_warning(&format!(
                "{} agents failed and were not merged",
                summary_stats.failed
            ));
        }

        self.user_interaction
            .display_progress("Starting reduce phase in parent worktree...");
    }

    /// Display worktree instructions after reduce phase
    fn display_worktree_instructions(&self, env: &ExecutionEnvironment) {
        // Don't merge here - let the orchestrator's cleanup handle it
        // This prevents double-merge attempts
        if let Some(worktree_name) = &env.worktree_name {
            if !self.should_auto_merge(env) {
                // Only show manual instructions if NOT auto-merging
                // (If auto-merging, orchestrator cleanup will handle it)
                self.user_interaction.display_info(&format!(
                    "\nParent worktree ready for review: {}\n",
                    worktree_name
                ));
                self.user_interaction
                    .display_info("To create a PR: git push origin <branch> && gh pr create");
            }
        }
    }

    /// Check if auto-merge is enabled
    fn should_auto_merge(&self, _env: &ExecutionEnvironment) -> bool {
        // Check for -y flag via environment variable
        std::env::var("PRODIGY_AUTO_MERGE").unwrap_or_default() == "true"
            || std::env::var("PRODIGY_AUTO_CONFIRM").unwrap_or_default() == "true"
    }

    /// Execute a single workflow step with agent context
    async fn execute_single_step(
        &self,
        step: &WorkflowStep,
        context: &mut AgentContext,
    ) -> MapReduceResult<StepResult> {
        // Use the step executor to handle interpolation, execution, and capture
        self.step_executor.execute(step, context).await
    }

    /// Format a legacy command name with leading slash if needed
    #[cfg(test)]
    pub(crate) fn format_legacy_command(name: &str) -> String {
        if name.starts_with('/') {
            name.to_string()
        } else {
            format!("/{name}")
        }
    }

    /// Handle on_failure logic for a failed step
    /// Returns Ok(true) if the failure was handled and execution should continue,
    /// Ok(false) if the workflow should fail, or an error if the handler itself failed
    async fn handle_on_failure(
        &self,
        on_failure: &crate::cook::workflow::OnFailureConfig,
        original_step: &WorkflowStep,
        context: &mut AgentContext,
        error: String,
    ) -> MapReduceResult<bool> {
        // Add error to context for interpolation
        context.variables.insert("error".to_string(), error.clone());
        context.variables.insert("last_error".to_string(), error);

        // Check if there's a handler to execute
        if let Some(handler_step) = on_failure.handler() {
            info!("Executing on_failure handler for agent {}", context.item_id);

            // Execute the on_failure handler step
            let handler_result = self.execute_single_step(&handler_step, context).await?;

            if !handler_result.success {
                warn!(
                    "on_failure handler failed for agent {}: {}",
                    context.item_id, handler_result.stderr
                );
                // If handler fails and fail_workflow is true, propagate failure
                if on_failure.should_fail_workflow() {
                    return Ok(false);
                }
            }

            // Check if we should retry the original command
            // Retry is determined by max_retries > 0 (consistent with regular workflows)
            if on_failure.should_retry() {
                let max_retries = on_failure.max_retries();
                info!(
                    "🔄 Will retry original command for agent {} (max_retries/max_attempts: {})",
                    context.item_id, max_retries
                );

                for retry in 1..=max_retries {
                    self.user_interaction.display_info(&format!(
                        "🔄 Retry attempt {}/{} for agent {}",
                        retry, max_retries, context.item_id
                    ));

                    // Create a copy of the step without on_failure to avoid recursion
                    let mut retry_step = original_step.clone();
                    retry_step.on_failure = None;

                    let retry_result = self.execute_single_step(&retry_step, context).await?;
                    if retry_result.success {
                        self.user_interaction.display_success(&format!(
                            "✅ Retry succeeded for agent {} on attempt {}/{}",
                            context.item_id, retry, max_retries
                        ));
                        return Ok(true); // Successfully handled
                    } else {
                        self.user_interaction.display_warning(&format!(
                            "❌ Retry attempt {}/{} failed for agent {}: {}",
                            retry,
                            max_retries,
                            context.item_id,
                            retry_result
                                .stderr
                                .lines()
                                .next()
                                .unwrap_or("unknown error")
                        ));
                    }
                }
                self.user_interaction.display_error(&format!(
                    "All {} retry attempts failed for agent {}",
                    max_retries, context.item_id
                ));
            } else {
                debug!(
                    "Not retrying original command (max_retries: {})",
                    on_failure.max_retries()
                );
            }
        }

        // Return whether we should continue based on fail_workflow setting
        Ok(!on_failure.should_fail_workflow())
    }

    /// Report execution summary
    fn report_summary(&self, results: &[AgentResult], duration: Duration) {
        let successful = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .count();
        let failed = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Failed(_)))
            .count();
        let timeout = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Timeout))
            .count();

        let total_commits: usize = results.iter().map(|r| r.commits.len()).sum();

        self.user_interaction.display_info(&format!(
            "\n📊 MapReduce Execution Summary:
            Total items: {}
            Successful: {} ({:.1}%)
            Failed: {} ({:.1}%)
            Timeouts: {} ({:.1}%)
            Total commits: {}
            Total duration: {:.2}s
            Average time per item: {:.2}s",
            results.len(),
            successful,
            (successful as f64 / results.len() as f64) * 100.0,
            failed,
            (failed as f64 / results.len() as f64) * 100.0,
            timeout,
            (timeout as f64 / results.len() as f64) * 100.0,
            total_commits,
            duration.as_secs_f64(),
            duration.as_secs_f64() / results.len() as f64,
        ));
    }

    /// Clone the executor for use in spawned tasks
    fn clone_executor(&self) -> MapReduceExecutor {
        MapReduceExecutor {
            claude_executor: self.claude_executor.clone(),
            session_manager: self.session_manager.clone(),
            user_interaction: self.user_interaction.clone(),
            worktree_manager: self.worktree_manager.clone(),
            worktree_pool: self.worktree_pool.clone(),
            project_root: self.project_root.clone(),
            interpolation_engine: self.interpolation_engine.clone(),
            command_registry: self.command_registry.clone(),
            command_router: self.command_router.clone(),
            step_executor: self.step_executor.clone(),
            subprocess: self.subprocess.clone(),
            state_manager: self.state_manager.clone(),
            enhanced_state_manager: self.enhanced_state_manager.clone(),
            event_logger: self.event_logger.clone(),
            dlq: self.dlq.clone(),
            correlation_id: self.correlation_id.clone(),
            enhanced_progress_tracker: self.enhanced_progress_tracker.clone(),
            new_progress_tracker: self.new_progress_tracker.clone(),
            enable_web_dashboard: self.enable_web_dashboard,
            setup_variables: self.setup_variables.clone(),
            retry_state_manager: self.retry_state_manager.clone(),
            error_policy_executor: None, // Don't clone error policy executor - it's per-job
            agent_lifecycle_manager: self.agent_lifecycle_manager.clone(),
            agent_result_aggregator: self.agent_result_aggregator.clone(),
            resource_manager: self.resource_manager.clone(),
        }
    }

    /// Extract a meaningful identifier from a JSON work item
    fn extract_item_identifier(item: &Value, index: usize) -> String {
        // Priority order for identifier fields
        let id_fields = [
            "id",
            "name",
            "title",
            "path",
            "file",
            "key",
            "label",
            "identifier",
        ];

        if let Value::Object(obj) = item {
            for field in &id_fields {
                if let Some(value) = obj.get(*field) {
                    match value {
                        Value::String(s) => {
                            return Self::truncate_identifier(s, 30);
                        }
                        Value::Number(n) => {
                            return n.to_string();
                        }
                        _ => continue,
                    }
                }
            }
        }

        // Fallback to index
        format!("item_{}", index)
    }

    fn truncate_identifier(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::command::TestCommand;
    use crate::cook::workflow::{CaptureOutput, HandlerStep};

    #[test]
    fn test_collect_command_types_claude() {
        let step = WorkflowStep {
            claude: Some("test command".to_string()),
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
        };

        let commands = command::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        matches!(&commands[0], crate::cook::workflow::CommandType::Claude(cmd) if cmd == "test command");
    }

    #[test]
    fn test_collect_command_types_shell() {
        let step = WorkflowStep {
            claude: None,
            shell: Some("echo test".to_string()),
            test: None,
            goal_seek: None,
            foreach: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
        };

        let commands = command::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        matches!(&commands[0], crate::cook::workflow::CommandType::Shell(cmd) if cmd == "echo test");
    }

    #[test]
    fn test_collect_command_types_test() {
        let step = WorkflowStep {
            claude: None,
            shell: None,
            test: Some(TestCommand {
                command: "cargo test".to_string(),
                on_failure: None,
            }),
            goal_seek: None,
            foreach: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
        };

        let commands = command::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        matches!(&commands[0], crate::cook::workflow::CommandType::Test(cmd) if cmd.command == "cargo test");
    }

    #[test]
    fn test_collect_command_types_handler() {
        let step = WorkflowStep {
            claude: None,
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            handler: Some(HandlerStep {
                name: "test_handler".to_string(),
                attributes: HashMap::new(),
            }),
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
        };

        let commands = command::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        matches!(&commands[0], crate::cook::workflow::CommandType::Handler { handler_name, .. } if handler_name == "test_handler");
    }

    #[test]
    fn test_collect_command_types_legacy_name() {
        let step = WorkflowStep {
            claude: None,
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            handler: None,
            name: Some("test_command".to_string()),
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
        };

        let commands = command::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        matches!(&commands[0], crate::cook::workflow::CommandType::Legacy(cmd) if cmd == "/test_command");
    }

    #[test]
    fn test_collect_command_types_legacy_name_with_slash() {
        let step = WorkflowStep {
            claude: None,
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            handler: None,
            name: Some("/test_command".to_string()),
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
        };

        let commands = command::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        if let crate::cook::workflow::CommandType::Legacy(cmd) = &commands[0] {
            assert_eq!(cmd, "/test_command");
        } else {
            panic!("Expected Legacy command type");
        }
    }

    #[test]
    fn test_collect_command_types_empty() {
        let step = WorkflowStep {
            claude: None,
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
        };

        let commands = command::collect_command_types(&step);

        assert_eq!(commands.len(), 0);
    }

    #[test]
    fn test_collect_command_types_multiple() {
        // This tests that the collection function returns all specified commands
        // The validation happens in validate_command_count
        let step = WorkflowStep {
            claude: Some("claude cmd".to_string()),
            shell: Some("shell cmd".to_string()),
            test: None,
            goal_seek: None,
            foreach: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
        };

        let commands = command::collect_command_types(&step);

        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_format_legacy_command() {
        assert_eq!(MapReduceExecutor::format_legacy_command("test"), "/test");
        assert_eq!(MapReduceExecutor::format_legacy_command("/test"), "/test");
        assert_eq!(
            MapReduceExecutor::format_legacy_command("/already/slash"),
            "/already/slash"
        );
    }

    #[test]
    fn test_truncate_identifier() {
        assert_eq!(MapReduceExecutor::truncate_identifier("short", 10), "short");
        assert_eq!(
            MapReduceExecutor::truncate_identifier("this is a very long identifier", 10),
            "this is..."
        );
        assert_eq!(
            MapReduceExecutor::truncate_identifier("exactly_ten", 11),
            "exactly_ten"
        );
        assert_eq!(
            MapReduceExecutor::truncate_identifier("exactly_eleven_", 11),
            "exactly_..."
        );
    }

    // Tests for checkpoint validation and pending item calculations
    // These tests verify the logic without needing full MapReduceExecutor setup

    #[test]
    fn test_checkpoint_state_validation() {
        use crate::cook::execution::state::MapReduceJobState;

        let config = MapReduceConfig {
            input: "test.json".to_string(),
            json_path: String::new(),
            max_parallel: 5,
            max_items: None,
            offset: None,
        };

        // Test empty job ID case
        let mut state = MapReduceJobState::new(
            String::new(),
            config.clone(),
            vec![serde_json::json!({"id": 1})],
        );
        state.job_id = String::new();
        assert!(state.job_id.is_empty());

        // Test no work items case
        let state2 = MapReduceJobState::new("test-job".to_string(), config.clone(), vec![]);
        assert!(state2.work_items.is_empty());

        // Test valid state
        let state3 = MapReduceJobState::new(
            "test-job".to_string(),
            config,
            vec![serde_json::json!({"id": 1})],
        );
        assert!(!state3.job_id.is_empty());
        assert!(!state3.work_items.is_empty());
    }

    #[test]
    fn test_pending_items_logic() {
        use crate::cook::execution::state::{FailureRecord, MapReduceJobState};

        let config = MapReduceConfig {
            input: "test.json".to_string(),
            json_path: String::new(),
            max_parallel: 5,
            max_items: None,
            offset: None,
        };

        let work_items = vec![
            serde_json::json!({"id": 1}),
            serde_json::json!({"id": 2}),
            serde_json::json!({"id": 3}),
        ];

        // Test all items pending initially
        let state =
            MapReduceJobState::new("test-job".to_string(), config.clone(), work_items.clone());
        assert_eq!(state.pending_items.len(), 3);
        assert!(state.completed_agents.is_empty());

        // Test with completed items
        let mut state2 =
            MapReduceJobState::new("test-job".to_string(), config.clone(), work_items.clone());
        state2.completed_agents.insert("item_0".to_string());
        state2.pending_items.retain(|x| x != "item_0");
        assert_eq!(state2.pending_items.len(), 2);

        // Test with failed items
        let mut state3 = MapReduceJobState::new("test-job".to_string(), config, work_items);
        use chrono::Utc;
        state3.failed_agents.insert(
            "item_1".to_string(),
            FailureRecord {
                item_id: "item_1".to_string(),
                attempts: 1,
                last_error: "Test error".to_string(),
                last_attempt: Utc::now(),
                worktree_info: None,
            },
        );

        // Check if item is retriable (attempts < retry_on_failure)
        let failed_record = state3.failed_agents.get("item_1").unwrap();
        assert!(failed_record.attempts < 2); // Default retry limit
    }
}
