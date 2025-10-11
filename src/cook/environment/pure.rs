//! Pure functions for environment resolution
//!
//! This module provides pure, stateless functions for resolving working directories
//! and building command environments. All functions are side-effect free and fully
//! testable without mocks.
//!
//! # Functional Programming Principles
//!
//! - **Pure Functions**: All functions have no side effects
//! - **Referential Transparency**: Same inputs always produce same outputs
//! - **Explicit Inputs**: All data dependencies are function parameters
//! - **Testability**: No mocks needed - just test with concrete values
//!
//! # Why Pure Functions?
//!
//! Previous implementation used mutable `EnvironmentManager` with hidden state that
//! caused bugs in MapReduce workflows. These pure functions make all logic explicit
//! and eliminate hidden state mutations.

use super::context::ImmutableEnvironmentContext;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::WorkflowStep;
use std::collections::HashMap;
use std::path::PathBuf;

/// Resolve working directory for a step (PURE FUNCTION)
///
/// Determines which directory to use for command execution based on:
/// 1. Explicit step.working_dir (highest priority)
/// 2. Environment context base directory (from worktree or repo)
///
/// # Arguments
///
/// * `step` - Workflow step (may specify explicit working_dir)
/// * `_env` - Execution environment (reserved for future use)
/// * `context` - Environment context (from builder)
///
/// # Returns
///
/// PathBuf representing the resolved working directory
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use prodigy::cook::environment::pure::resolve_working_directory;
/// use prodigy::cook::environment::context::ImmutableEnvironmentContext;
/// use prodigy::cook::workflow::WorkflowStep;
/// use prodigy::cook::orchestrator::ExecutionEnvironment;
/// use std::sync::Arc;
///
/// let step = WorkflowStep {
///     working_dir: Some(PathBuf::from("/custom")),
///     ..Default::default()
/// };
/// let context = ImmutableEnvironmentContext::new(PathBuf::from("/base"));
/// let env = ExecutionEnvironment {
///     working_dir: Arc::new(PathBuf::from("/env")),
///     project_dir: Arc::new(PathBuf::from("/project")),
///     worktree_name: None,
///     session_id: Arc::from("test"),
/// };
///
/// let working_dir = resolve_working_directory(&step, &env, &context);
/// assert_eq!(working_dir, PathBuf::from("/custom"));
/// ```
///
/// # Why This Is Pure
///
/// - Takes all inputs as parameters
/// - Returns new value, doesn't mutate
/// - No I/O, no hidden state
/// - Same inputs → same output always
pub fn resolve_working_directory(
    step: &WorkflowStep,
    _env: &ExecutionEnvironment,
    context: &ImmutableEnvironmentContext,
) -> PathBuf {
    // 1. Explicit step working_dir takes highest precedence
    if let Some(ref dir) = step.working_dir {
        return dir.clone();
    }

    // 2. Use environment context base directory (set by caller)
    //    This allows MapReduce workflows to explicitly set worktree directory
    context.working_dir().to_path_buf()

    // Note: We intentionally do NOT fall back to env.working_dir here
    // because context.base_working_dir should always be correctly set
    // by the caller (either to repo dir or worktree dir)
}

/// Build complete environment variables for command execution (PURE FUNCTION)
///
/// Combines global environment config, step-specific env vars, and
/// workflow variables to produce the final environment for a command.
///
/// # Arguments
///
/// * `step` - Workflow step with step-specific env vars
/// * `context` - Environment context with base env vars
/// * `workflow_vars` - Variables from workflow context (for interpolation)
///
/// # Returns
///
/// HashMap of all environment variables for command
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use std::path::PathBuf;
/// use prodigy::cook::environment::pure::build_command_env;
/// use prodigy::cook::environment::context::ImmutableEnvironmentContext;
/// use prodigy::cook::workflow::WorkflowStep;
///
/// let step = WorkflowStep {
///     env: vec![("CUSTOM".to_string(), "value".to_string())]
///         .into_iter()
///         .collect(),
///     ..Default::default()
/// };
/// let context = ImmutableEnvironmentContext::new(PathBuf::from("/test"));
/// let workflow_vars = HashMap::new();
///
/// let env_vars = build_command_env(&step, &context, &workflow_vars);
/// assert_eq!(env_vars.get("CUSTOM"), Some(&"value".to_string()));
/// assert_eq!(env_vars.get("PRODIGY_AUTOMATION"), Some(&"true".to_string()));
/// ```
///
/// # Why This Is Pure
///
/// - All inputs passed as parameters
/// - Returns new HashMap, doesn't mutate
/// - No I/O or side effects
/// - Deterministic: same inputs → same output
pub fn build_command_env(
    step: &WorkflowStep,
    context: &ImmutableEnvironmentContext,
    workflow_vars: &HashMap<String, String>,
) -> HashMap<String, String> {
    // Start with context env vars (inherited from system + global config)
    let mut env = context.env_vars().clone();

    // Add step-specific environment variables with interpolation
    for (key, value) in &step.env {
        let interpolated = interpolate_value(value, workflow_vars);
        env.insert(key.clone(), interpolated);
    }

    // Add Prodigy-specific variables
    env.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

    env
}

/// Interpolate variables in a value (PURE FUNCTION)
///
/// Replaces ${var} and $var patterns with values from the variables map.
///
/// # Arguments
///
/// * `value` - String potentially containing variable references
/// * `variables` - Map of variable names to values
///
/// # Returns
///
/// String with all variables interpolated
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use prodigy::cook::environment::pure::interpolate_value;
///
/// let mut vars = HashMap::new();
/// vars.insert("NAME".to_string(), "World".to_string());
/// vars.insert("COUNT".to_string(), "42".to_string());
///
/// assert_eq!(
///     interpolate_value("Hello ${NAME}!", &vars),
///     "Hello World!"
/// );
/// assert_eq!(
///     interpolate_value("Count: $COUNT", &vars),
///     "Count: 42"
/// );
/// ```
///
/// # Why This Is Pure
///
/// - Only string manipulation
/// - No I/O or mutation
/// - Deterministic output
pub fn interpolate_value(value: &str, variables: &HashMap<String, String>) -> String {
    let mut result = value.to_string();

    // Simple ${var} and $var interpolation
    for (key, val) in variables {
        result = result.replace(&format!("${{{}}}", key), val);
        result = result.replace(&format!("${}", key), val);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn create_test_env() -> ExecutionEnvironment {
        ExecutionEnvironment {
            working_dir: Arc::new(PathBuf::from("/env")),
            project_dir: Arc::new(PathBuf::from("/project")),
            worktree_name: None,
            session_id: Arc::from("test"),
        }
    }

    #[test]
    fn test_resolve_working_directory_explicit_step() {
        let step = WorkflowStep {
            working_dir: Some(PathBuf::from("/explicit")),
            ..Default::default()
        };
        let env = create_test_env();
        let context = ImmutableEnvironmentContext::new(PathBuf::from("/context"));

        let result = resolve_working_directory(&step, &env, &context);
        assert_eq!(result, PathBuf::from("/explicit"));
    }

    #[test]
    fn test_resolve_working_directory_from_context() {
        let step = WorkflowStep {
            working_dir: None,
            ..Default::default()
        };
        let env = create_test_env();
        let context = ImmutableEnvironmentContext::new(PathBuf::from("/worktree"));

        let result = resolve_working_directory(&step, &env, &context);
        assert_eq!(result, PathBuf::from("/worktree"));
    }

    #[test]
    fn test_resolve_working_directory_is_pure() {
        let step = WorkflowStep {
            working_dir: None,
            ..Default::default()
        };
        let env = create_test_env();
        let context = ImmutableEnvironmentContext::new(PathBuf::from("/test"));

        // Calling multiple times should always return same result
        let result1 = resolve_working_directory(&step, &env, &context);
        let result2 = resolve_working_directory(&step, &env, &context);
        let result3 = resolve_working_directory(&step, &env, &context);

        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
    }

    #[test]
    fn test_build_command_env_step_vars() {
        let step = WorkflowStep {
            env: vec![("CUSTOM".to_string(), "value".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let context = ImmutableEnvironmentContext::new(PathBuf::from("/test"));
        let workflow_vars = HashMap::new();

        let result = build_command_env(&step, &context, &workflow_vars);

        assert_eq!(result.get("CUSTOM"), Some(&"value".to_string()));
        assert_eq!(result.get("PRODIGY_AUTOMATION"), Some(&"true".to_string()));
    }

    #[test]
    fn test_build_command_env_interpolation() {
        let step = WorkflowStep {
            env: vec![("MESSAGE".to_string(), "Hello ${NAME}".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let context = ImmutableEnvironmentContext::new(PathBuf::from("/test"));
        let mut workflow_vars = HashMap::new();
        workflow_vars.insert("NAME".to_string(), "World".to_string());

        let result = build_command_env(&step, &context, &workflow_vars);

        assert_eq!(result.get("MESSAGE"), Some(&"Hello World".to_string()));
    }

    #[test]
    fn test_build_command_env_inherits_context_vars() {
        let step = WorkflowStep {
            env: HashMap::new(),
            ..Default::default()
        };

        // Create context with some env vars
        let mut env_vars = HashMap::new();
        env_vars.insert("FROM_CONTEXT".to_string(), "context_value".to_string());

        let context = ImmutableEnvironmentContext {
            base_working_dir: Arc::new(PathBuf::from("/test")),
            env_vars: Arc::new(env_vars),
            secret_keys: Arc::new(Vec::new()),
            profile: None,
        };

        let workflow_vars = HashMap::new();
        let result = build_command_env(&step, &context, &workflow_vars);

        assert_eq!(
            result.get("FROM_CONTEXT"),
            Some(&"context_value".to_string())
        );
        assert_eq!(result.get("PRODIGY_AUTOMATION"), Some(&"true".to_string()));
    }

    #[test]
    fn test_interpolate_value_bracketed() {
        let mut variables = HashMap::new();
        variables.insert("VAR".to_string(), "value".to_string());

        let result = interpolate_value("prefix-${VAR}-suffix", &variables);
        assert_eq!(result, "prefix-value-suffix");
    }

    #[test]
    fn test_interpolate_value_simple() {
        let mut variables = HashMap::new();
        variables.insert("VAR".to_string(), "value".to_string());

        let result = interpolate_value("prefix-$VAR-suffix", &variables);
        assert_eq!(result, "prefix-value-suffix");
    }

    #[test]
    fn test_interpolate_value_multiple() {
        let mut variables = HashMap::new();
        variables.insert("A".to_string(), "1".to_string());
        variables.insert("B".to_string(), "2".to_string());

        let result = interpolate_value("${A} and ${B} and $A and $B", &variables);
        assert_eq!(result, "1 and 2 and 1 and 2");
    }

    #[test]
    fn test_interpolate_value_no_variables() {
        let variables = HashMap::new();
        let result = interpolate_value("no variables here", &variables);
        assert_eq!(result, "no variables here");
    }

    #[test]
    fn test_interpolate_value_missing_variable() {
        let variables = HashMap::new();
        let result = interpolate_value("missing ${VAR} here", &variables);
        // Missing variables are left as-is
        assert_eq!(result, "missing ${VAR} here");
    }

    #[test]
    fn test_build_command_env_is_pure() {
        let step = WorkflowStep {
            env: vec![("KEY".to_string(), "value".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let context = ImmutableEnvironmentContext::new(PathBuf::from("/test"));
        let workflow_vars = HashMap::new();

        // Calling multiple times should produce identical results
        let result1 = build_command_env(&step, &context, &workflow_vars);
        let result2 = build_command_env(&step, &context, &workflow_vars);

        assert_eq!(result1, result2);
    }
}
