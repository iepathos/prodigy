//! Pure interpolation functions for command execution

use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::execution::mapreduce::AgentContext;
use crate::cook::execution::variables::VariableContext;
use crate::cook::workflow::WorkflowStep;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Engine for interpolating variables in commands
pub struct InterpolationEngine {
    inner: crate::cook::execution::interpolation::InterpolationEngine,
}

impl InterpolationEngine {
    /// Create a new interpolation engine
    pub fn new(strict_mode: bool) -> Self {
        Self {
            inner: crate::cook::execution::interpolation::InterpolationEngine::new(strict_mode),
        }
    }

    /// Interpolate a string with the given context
    pub fn interpolate(
        &mut self,
        template: &str,
        context: &InterpolationContext,
    ) -> Result<String, String> {
        self.inner
            .interpolate(template, context)
            .map_err(|e| e.to_string())
    }

    /// Extract variables from a template string
    pub fn extract_variables(&self, template: &str) -> Vec<String> {
        let mut variables = Vec::new();
        let mut chars = template.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '$' && chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                let mut var_name = String::new();

                while let Some(ch) = chars.next() {
                    if ch == '}' {
                        if !var_name.is_empty() {
                            variables.push(var_name);
                        }
                        break;
                    }
                    var_name.push(ch);
                }
            }
        }

        variables
    }
}

/// Pure function to interpolate a workflow step
pub async fn interpolate_workflow_step(
    step: &WorkflowStep,
    context: &InterpolationContext,
    engine: &mut InterpolationEngine,
) -> Result<WorkflowStep, String> {
    let mut interpolated = step.clone();

    // Interpolate all string fields
    if let Some(name) = &step.name {
        interpolated.name = Some(engine.interpolate(name, context)?);
    }

    if let Some(claude) = &step.claude {
        interpolated.claude = Some(engine.interpolate(claude, context)?);
    }

    if let Some(shell) = &step.shell {
        interpolated.shell = Some(engine.interpolate(shell, context)?);
    }

    if let Some(command) = &step.command {
        interpolated.command = Some(engine.interpolate(command, context)?);
    }

    // Interpolate test command if present
    if let Some(test) = &step.test {
        let mut test_clone = test.clone();
        test_clone.command = engine.interpolate(&test.command, context)?;
        interpolated.test = Some(test_clone);
    }

    Ok(interpolated)
}

/// Pure function to interpolate using variable context
pub async fn interpolate_workflow_step_enhanced(
    step: &WorkflowStep,
    context: &VariableContext,
) -> Result<WorkflowStep, String> {
    let mut interpolated = step.clone();

    // Interpolate all string fields
    if let Some(name) = &step.name {
        interpolated.name = Some(context.interpolate(name).await.map_err(|e| e.to_string())?);
    }

    if let Some(claude) = &step.claude {
        interpolated.claude = Some(
            context
                .interpolate(claude)
                .await
                .map_err(|e| e.to_string())?,
        );
    }

    if let Some(shell) = &step.shell {
        interpolated.shell = Some(
            context
                .interpolate(shell)
                .await
                .map_err(|e| e.to_string())?,
        );
    }

    if let Some(command) = &step.command {
        interpolated.command = Some(
            context
                .interpolate(command)
                .await
                .map_err(|e| e.to_string())?,
        );
    }

    // Interpolate test command if present
    if let Some(test) = &step.test {
        let mut test_clone = test.clone();
        test_clone.command = context
            .interpolate(&test.command)
            .await
            .map_err(|e| e.to_string())?;
        interpolated.test = Some(test_clone);
    }

    Ok(interpolated)
}

/// Build interpolation context from variables
pub fn build_interpolation_context(variables: &HashMap<String, String>) -> InterpolationContext {
    let mut context = InterpolationContext::new();

    for (key, value) in variables {
        context.set(key.clone(), serde_json::Value::String(value.clone()));
    }

    context
}

/// Merge two interpolation contexts
pub fn merge_contexts(
    _base: &InterpolationContext,
    overlay: &InterpolationContext,
) -> InterpolationContext {
    // Note: This would need access to the internal structure of InterpolationContext
    // For now, we just return a clone of the overlay
    // In practice, we'd merge the internal HashMaps
    overlay.clone()
}

/// Step interpolator that handles both legacy and enhanced interpolation
pub struct StepInterpolator {
    engine: Arc<Mutex<InterpolationEngine>>,
}

impl StepInterpolator {
    /// Create a new step interpolator
    pub fn new(engine: Arc<Mutex<InterpolationEngine>>) -> Self {
        Self { engine }
    }

    /// Interpolate a workflow step using the appropriate context
    pub async fn interpolate(
        &self,
        step: &WorkflowStep,
        context: &AgentContext,
    ) -> Result<WorkflowStep, crate::cook::execution::errors::MapReduceError> {
        // Decide which interpolation approach to use
        if context.item_id == "reduce" || context.variables.contains_key("map.total") {
            // Use enhanced context for reduce phase or when map variables present
            let var_context = context.to_variable_context().await;
            interpolate_workflow_step_enhanced(step, &var_context)
                .await
                .map_err(|e| crate::cook::execution::errors::MapReduceError::General {
                    message: format!("Interpolation failed: {}", e),
                    source: None,
                })
        } else {
            // Use legacy interpolation for backward compatibility
            let interp_context = context.to_interpolation_context();
            let mut engine = self.engine.lock().await;
            interpolate_workflow_step(step, &interp_context, &mut engine)
                .await
                .map_err(|e| crate::cook::execution::errors::MapReduceError::General {
                    message: format!("Interpolation failed: {}", e),
                    source: None,
                })
        }
    }
}
