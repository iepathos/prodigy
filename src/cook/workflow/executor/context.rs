//! Variable interpolation and context management
//!
//! This module handles building interpolation contexts, managing variables,
//! and formatting variable values for display with masking support.

use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::executor::{
    VariableResolution, WorkflowContext, WorkflowExecutor, WorkflowStep, BRACED_VAR_REGEX,
    UNBRACED_VAR_REGEX,
};
use crate::cook::workflow::git_context::GitChangeTracker;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

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
        let store_vars = futures::executor::block_on(self.variable_store.to_hashmap());
        for (key, value) in store_vars {
            context.set(key, Value::String(value));
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

        // Add any command-line arguments or environment variables
        if let Ok(arg) = std::env::var("PRODIGY_ARG") {
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

        // Enable Claude streaming if set in environment or by default for better observability
        if std::env::var("PRODIGY_CLAUDE_STREAMING").unwrap_or_else(|_| "true".to_string())
            == "true"
        {
            env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
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
