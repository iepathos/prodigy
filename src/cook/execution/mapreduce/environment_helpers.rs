//! Reader pattern helpers for environment access
//!
//! This module provides helper functions for accessing environment values using
//! Stillwater's Effect pattern. These helpers implement the Reader pattern for
//! clean dependency injection without manual parameter threading.
//!
//! # Architecture
//!
//! Instead of passing configuration and dependencies through function parameters:
//!
//! ```ignore
//! // Before: Manual threading
//! async fn execute_agent(
//!     item: &Value,
//!     config: &MapConfig,
//!     worktree_manager: &WorktreeManager,
//!     executor: &CommandExecutor,
//!     storage: &Storage,
//! ) -> Result<AgentResult> { ... }
//! ```
//!
//! Use Effect::asks to extract them from the environment:
//!
//! ```ignore
//! // After: Reader pattern
//! fn execute_agent(item: Value) -> Effect<AgentResult, AgentError, MapEnv> {
//!     get_worktree_manager()
//!         .and_then(|wt_mgr| create_worktree_effect(&item.id))
//!         .and_then(|worktree| execute_commands_effect(&item, &worktree))
//! }
//! ```
//!
//! # Local Overrides
//!
//! Use the `with_*` functions to temporarily modify environment values:
//!
//! ```ignore
//! // Increase timeout for long-running operation
//! let effect = with_timeout(
//!     Duration::from_secs(600),
//!     execute_setup_commands(commands),
//! );
//! ```
//!
//! # Testing
//!
//! Use `MockMapEnvBuilder` for unit testing:
//!
//! ```ignore
//! let env = MockMapEnvBuilder::new()
//!     .with_max_parallel(4)
//!     .build();
//!
//! let effect = get_max_parallel();
//! assert_eq!(effect.run(&env).unwrap(), 4);
//! ```

use crate::cook::execution::errors::MapReduceError;
use crate::cook::execution::mapreduce::agent_command_executor::AgentCommandExecutor;
use crate::cook::execution::mapreduce::checkpoint::storage::CheckpointStorage;
use crate::cook::execution::mapreduce::environment::{MapEnv, PhaseEnv};
use crate::cook::workflow::WorkflowStep;
use crate::worktree::WorktreeManager;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use stillwater::{asks, local, Effect};

// =============================================================================
// MapEnv Reader Helpers
// =============================================================================

/// Get the worktree manager from the map environment.
///
/// # Example
///
/// ```ignore
/// let effect = get_worktree_manager()
///     .and_then(|mgr| {
///         // Use worktree manager
///         Effect::pure(())
///     });
/// ```
pub fn get_worktree_manager(
) -> impl Effect<Output = Arc<WorktreeManager>, Error = MapReduceError, Env = MapEnv> {
    asks(|env: &MapEnv| env.worktree_manager.clone())
}

/// Get the command executor from the map environment.
///
/// # Example
///
/// ```ignore
/// let effect = get_command_executor()
///     .and_then(|executor| {
///         // Execute commands with executor
///         Effect::pure(())
///     });
/// ```
pub fn get_command_executor(
) -> impl Effect<Output = Arc<AgentCommandExecutor>, Error = MapReduceError, Env = MapEnv> {
    asks(|env: &MapEnv| env.command_executor.clone())
}

/// Get the checkpoint storage from the map environment.
///
/// # Example
///
/// ```ignore
/// let effect = get_storage()
///     .and_then(|storage| {
///         // Save checkpoint
///         Effect::pure(())
///     });
/// ```
pub fn get_storage(
) -> impl Effect<Output = Arc<dyn CheckpointStorage>, Error = MapReduceError, Env = MapEnv> {
    asks(|env: &MapEnv| env.storage.clone())
}

/// Get the agent template from the map environment.
///
/// # Example
///
/// ```ignore
/// let effect = get_agent_template()
///     .and_then(|template| {
///         // Use template for agent
///         Effect::pure(())
///     });
/// ```
pub fn get_agent_template(
) -> impl Effect<Output = Vec<WorkflowStep>, Error = MapReduceError, Env = MapEnv> {
    asks(|env: &MapEnv| env.agent_template.clone())
}

/// Get the job ID from the map environment.
///
/// # Example
///
/// ```ignore
/// let effect = get_job_id()
///     .and_then(|job_id| {
///         info!("Processing job: {}", job_id);
///         Effect::pure(())
///     });
/// ```
pub fn get_job_id() -> impl Effect<Output = String, Error = MapReduceError, Env = MapEnv> {
    asks(|env: &MapEnv| env.job_id.clone())
}

/// Get the maximum parallel agents setting from the map environment.
///
/// # Example
///
/// ```ignore
/// let effect = get_max_parallel()
///     .and_then(|max| {
///         // Limit concurrency
///         Effect::pure(max)
///     });
/// ```
pub fn get_max_parallel() -> impl Effect<Output = usize, Error = MapReduceError, Env = MapEnv> {
    asks(|env: &MapEnv| env.max_parallel)
}

/// Get workflow environment variables from the map environment.
///
/// # Example
///
/// ```ignore
/// let effect = get_workflow_env()
///     .and_then(|vars| {
///         // Access workflow variables
///         Effect::pure(())
///     });
/// ```
pub fn get_workflow_env(
) -> impl Effect<Output = HashMap<String, Value>, Error = MapReduceError, Env = MapEnv> {
    asks(|env: &MapEnv| env.workflow_env.clone())
}

/// Get additional configuration from the map environment.
///
/// # Example
///
/// ```ignore
/// let effect = get_config()
///     .and_then(|config| {
///         if let Some(debug) = config.get("debug") {
///             // Handle debug mode
///         }
///         Effect::pure(())
///     });
/// ```
pub fn get_config(
) -> impl Effect<Output = HashMap<String, Value>, Error = MapReduceError, Env = MapEnv> {
    asks(|env: &MapEnv| env.config.clone())
}

/// Get a specific configuration value from the map environment.
///
/// # Example
///
/// ```ignore
/// let effect = get_config_value("debug")
///     .and_then(|debug_value| {
///         // Use debug value
///         Effect::pure(())
///     });
/// ```
pub fn get_config_value(
    key: &str,
) -> impl Effect<Output = Option<Value>, Error = MapReduceError, Env = MapEnv> {
    let key = key.to_string();
    asks(move |env: &MapEnv| env.config.get(&key).cloned())
}

/// Compose multiple environment accesses into a single effect.
///
/// # Example
///
/// ```ignore
/// let effect = get_execution_context()
///     .and_then(|(job_id, max_parallel, storage)| {
///         // Use all three values together
///         Effect::pure(())
///     });
/// ```
pub fn get_execution_context() -> impl Effect<
    Output = (String, usize, Arc<dyn CheckpointStorage>),
    Error = MapReduceError,
    Env = MapEnv,
> {
    asks(|env: &MapEnv| (env.job_id.clone(), env.max_parallel, env.storage.clone()))
}

// =============================================================================
// PhaseEnv Reader Helpers
// =============================================================================

/// Get the command executor from the phase environment.
pub fn get_phase_command_executor(
) -> impl Effect<Output = Arc<AgentCommandExecutor>, Error = MapReduceError, Env = PhaseEnv> {
    asks(|env: &PhaseEnv| env.command_executor.clone())
}

/// Get the checkpoint storage from the phase environment.
pub fn get_phase_storage(
) -> impl Effect<Output = Arc<dyn CheckpointStorage>, Error = MapReduceError, Env = PhaseEnv> {
    asks(|env: &PhaseEnv| env.storage.clone())
}

/// Get variables from the phase environment.
pub fn get_variables(
) -> impl Effect<Output = HashMap<String, Value>, Error = MapReduceError, Env = PhaseEnv> {
    asks(|env: &PhaseEnv| env.variables.clone())
}

/// Get a specific variable from the phase environment.
pub fn get_variable(
    name: &str,
) -> impl Effect<Output = Option<Value>, Error = MapReduceError, Env = PhaseEnv> {
    let name = name.to_string();
    asks(move |env: &PhaseEnv| env.variables.get(&name).cloned())
}

/// Get workflow environment variables from the phase environment.
pub fn get_phase_workflow_env(
) -> impl Effect<Output = HashMap<String, Value>, Error = MapReduceError, Env = PhaseEnv> {
    asks(|env: &PhaseEnv| env.workflow_env.clone())
}

// =============================================================================
// Local Override Utilities for MapEnv
// =============================================================================

/// Configuration for local environment overrides.
#[derive(Clone, Debug, Default)]
pub struct MapEnvOverrides {
    /// Override max parallel agents
    pub max_parallel: Option<usize>,
    /// Override or merge additional config values
    pub config_overrides: Option<HashMap<String, Value>>,
    /// Override or merge workflow environment
    pub workflow_env_overrides: Option<HashMap<String, Value>>,
}

impl MapEnvOverrides {
    /// Create a new empty overrides instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set max parallel override.
    pub fn with_max_parallel(mut self, max_parallel: usize) -> Self {
        self.max_parallel = Some(max_parallel);
        self
    }

    /// Set config override.
    pub fn with_config(mut self, key: impl Into<String>, value: Value) -> Self {
        self.config_overrides
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value);
        self
    }

    /// Set workflow env override.
    pub fn with_workflow_env(mut self, key: impl Into<String>, value: Value) -> Self {
        self.workflow_env_overrides
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value);
        self
    }
}

/// Run an effect with a modified max_parallel setting.
///
/// This allows temporarily increasing or decreasing concurrency for
/// specific operations without affecting the rest of the workflow.
///
/// # Example
///
/// ```ignore
/// // Run with reduced concurrency for risky operations
/// let effect = with_max_parallel(
///     2,
///     execute_agents(work_items),
/// );
/// ```
pub fn with_max_parallel<E>(
    max_parallel: usize,
    effect: E,
) -> impl Effect<Output = E::Output, Error = MapReduceError, Env = MapEnv>
where
    E: Effect<Error = MapReduceError, Env = MapEnv>,
{
    local(
        move |env: &MapEnv| MapEnv {
            max_parallel,
            ..env.clone()
        },
        effect,
    )
}

/// Run an effect with additional config values.
///
/// Merges the provided config values with the existing config,
/// with new values taking precedence.
///
/// # Example
///
/// ```ignore
/// let effect = with_config(
///     [("debug".to_string(), json!(true))].into_iter().collect(),
///     execute_setup(commands),
/// );
/// ```
pub fn with_config<E>(
    config_overrides: HashMap<String, Value>,
    effect: E,
) -> impl Effect<Output = E::Output, Error = MapReduceError, Env = MapEnv>
where
    E: Effect<Error = MapReduceError, Env = MapEnv>,
{
    local(
        move |env: &MapEnv| {
            let mut config = env.config.clone();
            config.extend(config_overrides.clone());
            MapEnv {
                config,
                ..env.clone()
            }
        },
        effect,
    )
}

/// Run an effect with debug mode enabled.
///
/// Sets the "debug" config value to true for the duration of the effect.
///
/// # Example
///
/// ```ignore
/// let effect = with_debug(execute_commands(commands));
/// ```
pub fn with_debug<E>(
    effect: E,
) -> impl Effect<Output = E::Output, Error = MapReduceError, Env = MapEnv>
where
    E: Effect<Error = MapReduceError, Env = MapEnv>,
{
    local(
        |env: &MapEnv| {
            let mut config = env.config.clone();
            config.insert("debug".to_string(), serde_json::json!(true));
            MapEnv {
                config,
                ..env.clone()
            }
        },
        effect,
    )
}

/// Run an effect with verbose mode enabled.
///
/// Sets the "verbose" config value to true for the duration of the effect.
///
/// # Example
///
/// ```ignore
/// let effect = with_verbose(execute_agent(item));
/// ```
pub fn with_verbose<E>(
    effect: E,
) -> impl Effect<Output = E::Output, Error = MapReduceError, Env = MapEnv>
where
    E: Effect<Error = MapReduceError, Env = MapEnv>,
{
    local(
        |env: &MapEnv| {
            let mut config = env.config.clone();
            config.insert("verbose".to_string(), serde_json::json!(true));
            MapEnv {
                config,
                ..env.clone()
            }
        },
        effect,
    )
}

/// Run an effect with custom overrides.
///
/// Applies all overrides from MapEnvOverrides to the environment.
///
/// # Example
///
/// ```ignore
/// let overrides = MapEnvOverrides::new()
///     .with_max_parallel(2)
///     .with_config("debug", json!(true));
///
/// let effect = with_overrides(overrides, execute_agents(items));
/// ```
pub fn with_overrides<E>(
    overrides: MapEnvOverrides,
    effect: E,
) -> impl Effect<Output = E::Output, Error = MapReduceError, Env = MapEnv>
where
    E: Effect<Error = MapReduceError, Env = MapEnv>,
{
    local(
        move |env: &MapEnv| {
            let mut new_env = env.clone();

            if let Some(max_parallel) = overrides.max_parallel {
                new_env.max_parallel = max_parallel;
            }

            if let Some(ref config_overrides) = overrides.config_overrides {
                new_env.config.extend(config_overrides.clone());
            }

            if let Some(ref workflow_env_overrides) = overrides.workflow_env_overrides {
                new_env.workflow_env.extend(workflow_env_overrides.clone());
            }

            new_env
        },
        effect,
    )
}

// =============================================================================
// Local Override Utilities for PhaseEnv
// =============================================================================

/// Configuration for phase environment overrides.
#[derive(Clone, Debug, Default)]
pub struct PhaseEnvOverrides {
    /// Override or merge variables
    pub variable_overrides: Option<HashMap<String, Value>>,
    /// Override or merge workflow environment
    pub workflow_env_overrides: Option<HashMap<String, Value>>,
}

impl PhaseEnvOverrides {
    /// Create a new empty overrides instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set variable override.
    pub fn with_variable(mut self, name: impl Into<String>, value: Value) -> Self {
        self.variable_overrides
            .get_or_insert_with(HashMap::new)
            .insert(name.into(), value);
        self
    }

    /// Set workflow env override.
    pub fn with_workflow_env(mut self, key: impl Into<String>, value: Value) -> Self {
        self.workflow_env_overrides
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value);
        self
    }
}

/// Run a phase effect with additional variables.
///
/// # Example
///
/// ```ignore
/// let effect = with_variables(
///     [("result".to_string(), json!({"count": 10}))].into_iter().collect(),
///     execute_reduce(commands),
/// );
/// ```
pub fn with_variables<E>(
    variable_overrides: HashMap<String, Value>,
    effect: E,
) -> impl Effect<Output = E::Output, Error = MapReduceError, Env = PhaseEnv>
where
    E: Effect<Error = MapReduceError, Env = PhaseEnv>,
{
    local(
        move |env: &PhaseEnv| {
            let mut variables = env.variables.clone();
            variables.extend(variable_overrides.clone());
            PhaseEnv {
                variables,
                ..env.clone()
            }
        },
        effect,
    )
}

/// Run a phase effect with custom overrides.
///
/// # Example
///
/// ```ignore
/// let overrides = PhaseEnvOverrides::new()
///     .with_variable("result", json!({"count": 10}));
///
/// let effect = with_phase_overrides(overrides, execute_reduce(commands));
/// ```
pub fn with_phase_overrides<E>(
    overrides: PhaseEnvOverrides,
    effect: E,
) -> impl Effect<Output = E::Output, Error = MapReduceError, Env = PhaseEnv>
where
    E: Effect<Error = MapReduceError, Env = PhaseEnv>,
{
    local(
        move |env: &PhaseEnv| {
            let mut new_env = env.clone();

            if let Some(ref variable_overrides) = overrides.variable_overrides {
                new_env.variables.extend(variable_overrides.clone());
            }

            if let Some(ref workflow_env_overrides) = overrides.workflow_env_overrides {
                new_env.workflow_env.extend(workflow_env_overrides.clone());
            }

            new_env
        },
        effect,
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::mock_environment::{
        MockMapEnvBuilder, MockPhaseEnvBuilder,
    };
    use stillwater::{Effect, EffectExt};

    #[tokio::test]
    async fn test_get_max_parallel() {
        let env = MockMapEnvBuilder::new().with_max_parallel(10).build();

        let effect = get_max_parallel();
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10);
    }

    #[tokio::test]
    async fn test_get_job_id() {
        let env = MockMapEnvBuilder::new().with_job_id("my-job-456").build();

        let effect = get_job_id();
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "my-job-456");
    }

    #[tokio::test]
    async fn test_get_config_value() {
        let env = MockMapEnvBuilder::new()
            .with_config("debug", serde_json::json!(true))
            .with_config("timeout", serde_json::json!(30))
            .build();

        // Test existing key
        let effect = get_config_value("debug");
        let result = effect.run(&env).await.unwrap();
        assert_eq!(result, Some(serde_json::json!(true)));

        // Test missing key
        let effect = get_config_value("nonexistent");
        let result = effect.run(&env).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_with_max_parallel_local_override() {
        let env = MockMapEnvBuilder::new().with_max_parallel(5).build();

        // Without override
        let effect = get_max_parallel();
        assert_eq!(effect.run(&env).await.unwrap(), 5);

        // With local override
        let effect = with_max_parallel(20, get_max_parallel());
        assert_eq!(effect.run(&env).await.unwrap(), 20);

        // Original environment unchanged
        let effect = get_max_parallel();
        assert_eq!(effect.run(&env).await.unwrap(), 5);
    }

    #[tokio::test]
    async fn test_with_debug_local_override() {
        let env = MockMapEnvBuilder::new().build();

        // Without debug
        let effect = get_config_value("debug");
        assert_eq!(effect.run(&env).await.unwrap(), None);

        // With debug enabled
        let effect = with_debug(get_config_value("debug"));
        assert_eq!(
            effect.run(&env).await.unwrap(),
            Some(serde_json::json!(true))
        );

        // Original environment unchanged
        let effect = get_config_value("debug");
        assert_eq!(effect.run(&env).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_local_changes_dont_leak() {
        let env = MockMapEnvBuilder::new().with_max_parallel(5).build();

        // Execute with local override
        let inner_effect = with_max_parallel(100, get_max_parallel());
        let inner_result = inner_effect.run(&env).await.unwrap();
        assert_eq!(inner_result, 100); // Override applied inside

        // Check environment unchanged outside
        let outer_effect = get_max_parallel();
        let outer_result = outer_effect.run(&env).await.unwrap();
        assert_eq!(outer_result, 5); // Original value preserved
    }

    #[tokio::test]
    async fn test_nested_local_overrides() {
        let env = MockMapEnvBuilder::new().with_max_parallel(5).build();

        // Nested local overrides
        let effect = with_debug(with_max_parallel(
            50,
            get_max_parallel()
                .and_then(|max| get_config_value("debug").map(move |debug| (max, debug))),
        ));

        let (max, debug) = effect.run(&env).await.unwrap();
        assert_eq!(max, 50);
        assert_eq!(debug, Some(serde_json::json!(true)));

        // Original environment still unchanged
        let effect = get_max_parallel();
        assert_eq!(effect.run(&env).await.unwrap(), 5);
    }

    #[tokio::test]
    async fn test_with_overrides() {
        let env = MockMapEnvBuilder::new().with_max_parallel(5).build();

        let overrides = MapEnvOverrides::new()
            .with_max_parallel(25)
            .with_config("verbose", serde_json::json!(true));

        let effect = with_overrides(
            overrides,
            get_max_parallel()
                .and_then(|max| get_config_value("verbose").map(move |verbose| (max, verbose))),
        );

        let (max, verbose) = effect.run(&env).await.unwrap();
        assert_eq!(max, 25);
        assert_eq!(verbose, Some(serde_json::json!(true)));
    }

    #[tokio::test]
    async fn test_phase_env_get_variables() {
        let env = MockPhaseEnvBuilder::new()
            .with_variable("count", serde_json::json!(42))
            .with_variable("name", serde_json::json!("test"))
            .build();

        let effect = get_variable("count");
        let result = effect.run(&env).await.unwrap();
        assert_eq!(result, Some(serde_json::json!(42)));

        let effect = get_variable("missing");
        let result = effect.run(&env).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_phase_with_variables_override() {
        let env = MockPhaseEnvBuilder::new()
            .with_variable("count", serde_json::json!(10))
            .build();

        // Without override
        let effect = get_variable("count");
        assert_eq!(effect.run(&env).await.unwrap(), Some(serde_json::json!(10)));

        // With override
        let new_vars = [("count".to_string(), serde_json::json!(100))]
            .into_iter()
            .collect();
        let effect = with_variables(new_vars, get_variable("count"));
        assert_eq!(
            effect.run(&env).await.unwrap(),
            Some(serde_json::json!(100))
        );

        // Original unchanged
        let effect = get_variable("count");
        assert_eq!(effect.run(&env).await.unwrap(), Some(serde_json::json!(10)));
    }
}
