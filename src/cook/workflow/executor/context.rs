//! Variable interpolation and context management
//!
//! This module handles building interpolation contexts, managing variables,
//! and formatting variable values for display with masking support.

use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::executor::{
    VariableResolution, WorkflowExecutor, WorkflowStep, BRACED_VAR_REGEX, UNBRACED_VAR_REGEX,
};
use crate::cook::workflow::git_context::GitChangeTracker;
use crate::cook::workflow::validation::ValidationResult;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Workflow context for variable interpolation
///
/// Manages all variables, captured outputs, and state needed for variable
/// interpolation throughout workflow execution. Supports complex data types
/// through the variable store and git change tracking.
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    /// Simple string variables set explicitly
    pub variables: HashMap<String, String>,
    /// Deprecated: Captured command outputs (use variable_store instead)
    pub captured_outputs: HashMap<String, String>,
    /// Iteration-specific variables (for iterative workflows)
    pub iteration_vars: HashMap<String, String>,
    /// Validation results from validation steps
    pub validation_results: HashMap<String, ValidationResult>,
    /// Advanced variable store supporting complex types (arrays, objects, etc.)
    pub variable_store: Arc<crate::cook::workflow::variables::VariableStore>,
    /// Git change tracker for file and commit tracking variables
    pub git_tracker: Option<Arc<std::sync::Mutex<GitChangeTracker>>>,
}

impl Default for WorkflowContext {
    fn default() -> Self {
        Self {
            variables: HashMap::new(),
            captured_outputs: HashMap::new(),
            iteration_vars: HashMap::new(),
            validation_results: HashMap::new(),
            variable_store: Arc::new(crate::cook::workflow::variables::VariableStore::new()),
            git_tracker: None,
        }
    }
}

impl WorkflowContext {
    /// Build InterpolationContext from WorkflowContext variables (pure function)
    pub fn build_interpolation_context(&self) -> InterpolationContext {
        let mut context = InterpolationContext::new();

        // Add variables as strings
        for (key, value) in &self.variables {
            context.set(key.clone(), Value::String(value.clone()));
        }

        // Add captured outputs
        for (key, value) in &self.captured_outputs {
            context.set(key.clone(), Value::String(value.clone()));
        }

        // Add variables from variable store
        // Use get_all() to get CapturedValue objects, then convert to JSON values
        // This preserves complex types like arrays and objects instead of stringifying them
        let store_vars = futures::executor::block_on(self.variable_store.get_all());
        for (key, captured_value) in store_vars {
            // Convert CapturedValue to JSON Value using to_json()
            // This ensures arrays/objects are properly represented as JSON, not strings
            let json_value = captured_value.to_json();
            context.set(key, json_value);
        }

        // Add iteration variables
        for (key, value) in &self.iteration_vars {
            context.set(key.clone(), Value::String(value.clone()));
        }

        // Add validation results as structured data
        for (key, validation_result) in &self.validation_results {
            let validation_value = serde_json::json!({
                "completion": validation_result.completion_percentage,
                "missing": validation_result.missing,
                "missing_count": validation_result.missing.len(),
                "status": validation_result.status,
                "implemented": validation_result.implemented,
                "gaps": validation_result.gaps
            });
            context.set(key.clone(), validation_value);
        }

        // Add git context variables if tracker is available
        if let Some(ref git_tracker) = self.git_tracker {
            if let Ok(tracker) = git_tracker.lock() {
                // Add custom resolver for git variables
                // We'll need to handle this differently as InterpolationContext
                // doesn't support lazy evaluation. Instead, we'll add the git
                // variables directly to the context.

                // Get current step changes
                if let Some(step_id) = tracker.current_step_id.as_ref() {
                    if let Some(changes) = tracker.get_step_changes(step_id) {
                        // Add step.* variables
                        context.set(
                            "step.files_added",
                            Value::String(changes.files_added.join(" ")),
                        );
                        context.set(
                            "step.files_modified",
                            Value::String(changes.files_modified.join(" ")),
                        );
                        context.set(
                            "step.files_deleted",
                            Value::String(changes.files_deleted.join(" ")),
                        );
                        context.set(
                            "step.files_changed",
                            Value::String(changes.files_changed().join(" ")),
                        );
                        context.set("step.commits", Value::String(changes.commits.join(" ")));
                        context.set(
                            "step.commit_count",
                            Value::String(changes.commit_count().to_string()),
                        );
                        context.set(
                            "step.insertions",
                            Value::String(changes.insertions.to_string()),
                        );
                        context.set(
                            "step.deletions",
                            Value::String(changes.deletions.to_string()),
                        );
                    }
                }

                // Add workflow.* variables for cumulative changes
                let workflow_changes = tracker.get_workflow_changes();
                context.set(
                    "workflow.files_added",
                    Value::String(workflow_changes.files_added.join(" ")),
                );
                context.set(
                    "workflow.files_modified",
                    Value::String(workflow_changes.files_modified.join(" ")),
                );
                context.set(
                    "workflow.files_deleted",
                    Value::String(workflow_changes.files_deleted.join(" ")),
                );
                context.set(
                    "workflow.files_changed",
                    Value::String(workflow_changes.files_changed().join(" ")),
                );
                context.set(
                    "workflow.commits",
                    Value::String(workflow_changes.commits.join(" ")),
                );
                context.set(
                    "workflow.commit_count",
                    Value::String(workflow_changes.commit_count().to_string()),
                );
                context.set(
                    "workflow.insertions",
                    Value::String(workflow_changes.insertions.to_string()),
                );
                context.set(
                    "workflow.deletions",
                    Value::String(workflow_changes.deletions.to_string()),
                );
            }
        }

        context
    }

    /// Track variable resolutions from interpolation (pure function)
    pub fn extract_variable_resolutions(
        template: &str,
        _result: &str,
        context: &InterpolationContext,
    ) -> Vec<VariableResolution> {
        let mut resolutions = Vec::new();

        // Find ${...} patterns in original template
        for captures in BRACED_VAR_REGEX.captures_iter(template) {
            if let Some(var_match) = captures.get(0) {
                let full_expression = var_match.as_str();
                let var_expression = match captures.get(1) {
                    Some(m) => m.as_str(),
                    None => continue,
                };

                // Parse path segments (handle dotted paths like "map.successful")
                let path_segments: Vec<String> =
                    var_expression.split('.').map(|s| s.to_string()).collect();

                // Check if this variable was resolved by looking in the context
                if let Ok(value) = context.resolve_path(&path_segments) {
                    let resolved_value = Self::value_to_string(&value, var_expression);

                    resolutions.push(VariableResolution {
                        name: var_expression.to_string(),
                        raw_expression: full_expression.to_string(),
                        resolved_value,
                    });
                }
            }
        }

        // Find $VAR patterns (unbraced variables)
        for captures in UNBRACED_VAR_REGEX.captures_iter(template) {
            if let Some(var_match) = captures.get(0) {
                let full_expression = var_match.as_str();
                let var_name = match captures.get(1) {
                    Some(m) => m.as_str(),
                    None => continue,
                };

                // Check if this variable was resolved by looking in the context
                let path_segments = vec![var_name.to_string()];
                if let Ok(value) = context.resolve_path(&path_segments) {
                    let resolved_value = Self::value_to_string(&value, var_name);

                    resolutions.push(VariableResolution {
                        name: var_name.to_string(),
                        raw_expression: full_expression.to_string(),
                        resolved_value,
                    });
                }
            }
        }

        resolutions
    }

    /// Helper to convert Value to String
    fn value_to_string(value: &Value, _var_name: &str) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Array(arr) => {
                // For arrays, if they contain strings, join them
                if arr.iter().all(|v| matches!(v, Value::String(_))) {
                    let strings: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    strings.join(", ")
                } else {
                    serde_json::to_string(arr).unwrap_or_else(|_| format!("{:?}", arr))
                }
            }
            other => {
                // For complex objects, try to serialize to JSON
                serde_json::to_string(other).unwrap_or_else(|_| format!("{:?}", other))
            }
        }
    }

    /// Interpolate variables in a template string
    ///
    /// This is the standard interpolation method that silently falls back to
    /// the original template if interpolation fails. For error handling, use
    /// `interpolate_strict` instead.
    pub fn interpolate(&self, template: &str) -> String {
        self.interpolate_with_tracking(template).0
    }

    /// Interpolate variables and track resolutions for verbose output
    ///
    /// Returns both the interpolated string and a list of variable resolutions
    /// that occurred during interpolation. Useful for debugging and verbose
    /// logging of variable substitutions.
    pub fn interpolate_with_tracking(&self, template: &str) -> (String, Vec<VariableResolution>) {
        // Build interpolation context using pure function
        let context = self.build_interpolation_context();

        // Use InterpolationEngine for proper template parsing and variable resolution
        let mut engine = crate::cook::execution::interpolation::InterpolationEngine::new(false); // non-strict mode for backward compatibility

        match engine.interpolate(template, &context) {
            Ok(result) => {
                // Extract variable resolutions for tracking
                let resolutions = Self::extract_variable_resolutions(template, &result, &context);
                (result, resolutions)
            }
            Err(error) => {
                // Log interpolation failure for debugging
                tracing::warn!(
                    "Variable interpolation failed for template '{}': {}",
                    template,
                    error
                );

                // Provide detailed error information
                let available_variables =
                    WorkflowExecutor::get_available_variable_summary(&context);
                tracing::debug!("Available variables: {}", available_variables);

                // Fallback to original template on error (non-strict mode behavior)
                (template.to_string(), Vec::new())
            }
        }
    }

    /// Enhanced interpolation with strict mode and detailed error reporting
    ///
    /// Unlike `interpolate()`, this method returns an error if interpolation fails
    /// instead of falling back to the original template. Use this when you need
    /// to ensure all variables are properly resolved.
    pub fn interpolate_strict(&self, template: &str) -> Result<String, String> {
        let context = self.build_interpolation_context();
        let mut engine = crate::cook::execution::interpolation::InterpolationEngine::new(true); // strict mode

        engine.interpolate(template, &context).map_err(|error| {
            let available_variables = WorkflowExecutor::get_available_variable_summary(&context);
            format!(
                "Variable interpolation failed for template '{}': {}. Available variables: {}",
                template, error, available_variables
            )
        })
    }

    /// Resolve a variable path from the store (async)
    ///
    /// Queries the variable store for a value at the given path. Paths can be
    /// dotted notation like "user.name" or "map.results\[0\]".
    pub async fn resolve_variable(&self, path: &str) -> Option<String> {
        if let Ok(value) = self.variable_store.resolve_path(path).await {
            Some(value.to_string())
        } else {
            None
        }
    }
}

impl WorkflowExecutor {
    /// Log variable resolutions in debug mode
    pub fn log_variable_resolutions(&self, resolutions: &[VariableResolution]) {
        if tracing::enabled!(tracing::Level::DEBUG) && !resolutions.is_empty() {
            for resolution in resolutions {
                // Format the value for display, applying masking if sensitive
                let display_value = self.format_variable_value_with_masking(
                    &resolution.name,
                    &resolution.resolved_value,
                );
                tracing::debug!(
                    "   Variable {} = {}",
                    resolution.raw_expression,
                    display_value
                );
            }
        }
    }

    /// Format variable value with sensitive data masking
    pub fn format_variable_value_with_masking(&self, name: &str, value: &str) -> String {
        // Check if this variable should be masked based on name patterns
        let should_mask_by_name = self
            .sensitive_config
            .name_patterns
            .iter()
            .any(|pattern| pattern.is_match(name));

        // Check if this value should be masked based on value patterns
        let should_mask_by_value = self
            .sensitive_config
            .value_patterns
            .iter()
            .any(|pattern| pattern.is_match(value));

        if should_mask_by_name || should_mask_by_value {
            // Return masked value
            self.sensitive_config.mask_string.clone()
        } else {
            // Format normally if not sensitive
            WorkflowExecutor::format_variable_value_static(value)
        }
    }

    /// Format variable value for display (used by tests)
    #[cfg(test)]
    pub fn format_variable_value(&self, value: &str) -> String {
        WorkflowExecutor::format_variable_value_static(value)
    }

    /// Static helper for formatting variable values
    pub fn format_variable_value_static(value: &str) -> String {
        const MAX_LENGTH: usize = 200;

        // Try to parse as JSON for pretty printing
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(value) {
            // Handle arrays and objects specially
            match &json_val {
                serde_json::Value::Array(arr) => {
                    if arr.is_empty() {
                        return "[]".to_string();
                    }
                    // For arrays, show as JSON if small, otherwise show count
                    let json_str =
                        serde_json::to_string(&json_val).unwrap_or_else(|_| value.to_string());
                    if json_str.len() <= MAX_LENGTH {
                        return json_str;
                    } else {
                        return format!("[...{} items...]", arr.len());
                    }
                }
                serde_json::Value::Object(obj) => {
                    if obj.is_empty() {
                        return "{}".to_string();
                    }
                    // For objects, pretty print if small
                    if let Ok(pretty) = serde_json::to_string_pretty(&json_val) {
                        if pretty.len() <= MAX_LENGTH {
                            return pretty;
                        } else {
                            // Show abbreviated version
                            let keys: Vec<_> = obj.keys().take(3).cloned().collect();
                            let preview = if obj.len() > 3 {
                                format!(
                                    "{{ {}, ... ({} total fields) }}",
                                    keys.join(", "),
                                    obj.len()
                                )
                            } else {
                                format!("{{ {} }}", keys.join(", "))
                            };
                            return preview;
                        }
                    }
                }
                _ => {
                    // For simple values, use as-is
                    return value.to_string();
                }
            }
        }

        // Not JSON, handle as plain string
        if value.len() <= MAX_LENGTH {
            // Quote strings to make them clear
            format!("\"{}\"", value)
        } else {
            // Truncate long values
            format!(
                "\"{}...\" (showing first {} chars)",
                &value[..MAX_LENGTH],
                MAX_LENGTH
            )
        }
    }

    /// Initialize workflow context with environment variables
    pub fn init_workflow_context(&self, env: &ExecutionEnvironment) -> WorkflowContext {
        let mut workflow_context = WorkflowContext::default();

        // Initialize git change tracker
        if let Ok(tracker) = GitChangeTracker::new(&**env.working_dir) {
            if tracker.is_active() {
                workflow_context.git_tracker = Some(Arc::new(std::sync::Mutex::new(tracker)));
                tracing::debug!("Git change tracker initialized for workflow");
            }
        }

        // Add positional arguments if provided (SPEC 163)
        // This makes $ARG and $ARG_N variables available for interpolation
        if let Some(args) = &self.positional_args {
            // Add first positional arg as $ARG for backward compatibility
            if let Some(first_arg) = args.first() {
                workflow_context.variables.insert("ARG".to_string(), first_arg.clone());
            }
            // Also inject all positional args as ARG_1, ARG_2, etc.
            use crate::cook::environment::pure::inject_positional_args;
            inject_positional_args(&mut workflow_context.variables, args);
        }
        // Fall back to PRODIGY_ARG environment variable if no positional args
        else if let Ok(arg) = std::env::var("PRODIGY_ARG") {
            workflow_context.variables.insert("ARG".to_string(), arg);
        }

        // Add project root and working directory
        workflow_context.variables.insert(
            "PROJECT_ROOT".to_string(),
            env.working_dir.to_string_lossy().to_string(),
        );

        // Add worktree name if available
        if let Ok(worktree) = std::env::var("PRODIGY_WORKTREE") {
            workflow_context
                .variables
                .insert("WORKTREE".to_string(), worktree);
        }

        workflow_context
    }

    /// Prepare environment variables for step execution
    pub fn prepare_env_vars(
        &self,
        step: &WorkflowStep,
        _env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        // Add automation flag
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Propagate PRODIGY_CLAUDE_STREAMING environment variable if set (spec 129)
        if let Ok(streaming_val) = std::env::var("PRODIGY_CLAUDE_STREAMING") {
            env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), streaming_val);
        }

        // Add step-specific environment variables with interpolation
        for (key, value) in &step.env {
            let (interpolated_value, resolutions) = ctx.interpolate_with_tracking(value);
            if !resolutions.is_empty() {
                tracing::debug!("   Environment variable {} resolved:", key);
                self.log_variable_resolutions(&resolutions);
            }
            env_vars.insert(key.clone(), interpolated_value);
        }

        env_vars
    }
}

#[cfg(test)]
mod tests {
    use crate::cook::execution::interpolation::InterpolationEngine;
    use crate::cook::workflow::executor::WorkflowContext;
    use crate::cook::workflow::variables::{CapturedValue, VariableStore};
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_map_results_interpolation_produces_valid_json() {
        // This test reproduces the exact scenario from the bug report
        // where ${map.results} was producing invalid JSON during interpolation

        // Create a variable store and add map.results as an array
        let store = VariableStore::new();
        let map_results = vec![
            CapturedValue::Json(json!({
                "item_id": "item_0",
                "status": "Success",
                "commits": ["abc123"]
            })),
            CapturedValue::Json(json!({
                "item_id": "item_1",
                "status": "Success",
                "commits": ["def456"]
            })),
        ];
        store
            .set("map.results", CapturedValue::Array(map_results))
            .await;

        // Create a WorkflowContext with the variable store
        let ctx = WorkflowContext {
            variable_store: Arc::new(store),
            ..Default::default()
        };

        // Build interpolation context (this is where the bug was)
        let interpolation_context = ctx.build_interpolation_context();

        // Interpolate ${map.results}
        let mut engine = InterpolationEngine::new(false);
        let interpolated = engine
            .interpolate("${map.results}", &interpolation_context)
            .expect("Interpolation should succeed");

        // Verify that the interpolated string is valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&interpolated)
            .expect("Interpolated map.results should be valid JSON");

        // Verify it's an array with 2 items
        assert!(parsed.is_array(), "map.results should be an array");
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2, "map.results should have 2 items");

        // Verify the structure of the first item
        assert_eq!(arr[0]["item_id"], "item_0");
        assert_eq!(arr[0]["status"], "Success");
    }

    #[tokio::test]
    async fn test_map_summary_stats_interpolation() {
        // Test that map.successful, map.failed, map.total work correctly
        let store = VariableStore::new();
        store
            .set("map.successful", CapturedValue::Number(8.0))
            .await;
        store.set("map.failed", CapturedValue::Number(2.0)).await;
        store.set("map.total", CapturedValue::Number(10.0)).await;

        let ctx = WorkflowContext {
            variable_store: Arc::new(store),
            ..Default::default()
        };

        let interpolation_context = ctx.build_interpolation_context();
        let mut engine = InterpolationEngine::new(false);

        let template = "Processed ${map.total}: ${map.successful} ok, ${map.failed} failed";
        let result = engine
            .interpolate(template, &interpolation_context)
            .expect("Interpolation should succeed");

        assert_eq!(result, "Processed 10.0: 8.0 ok, 2.0 failed");
    }
}
