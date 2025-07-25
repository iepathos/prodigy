use anyhow::{Context, Result};
use pest_derive::Parser;
use serde_json::Value;

use super::WorkflowContext;

#[derive(Parser)]
#[grammar = "workflow/condition.pest"]
struct ConditionParser;

pub struct ConditionEvaluator {}

impl Default for ConditionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ConditionEvaluator {
    pub fn new() -> Self {
        Self {}
    }

    pub fn evaluate(&self, condition: &str, context: &WorkflowContext) -> Result<bool> {
        let expanded = self.expand_variables(condition, context)?;

        if expanded.trim() == "true" {
            return Ok(true);
        } else if expanded.trim() == "false" {
            return Ok(false);
        }

        self.evaluate_expression(&expanded)
    }

    fn expand_variables(&self, condition: &str, context: &WorkflowContext) -> Result<String> {
        let mut tera_context = tera::Context::new();

        tera_context.insert("parameters", &context.parameters);
        tera_context.insert("variables", &context.variables);
        tera_context.insert("outputs", &context.outputs);
        tera_context.insert("project", &context.project);
        tera_context.insert("spec", &context.spec_id);

        if let Some(current_stage) = context.variables.get("current_stage") {
            tera_context.insert(
                "stages",
                &serde_json::json!({
                    current_stage.as_str().unwrap_or(""): {
                        "status": context.variables.get("stage_status").unwrap_or(&Value::Null)
                    }
                }),
            );
        }

        let mut engine = tera::Tera::default();
        engine
            .render_str(condition, &tera_context)
            .context("Failed to expand condition variables")
    }

    fn evaluate_expression(&self, expression: &str) -> Result<bool> {
        let parts: Vec<&str> = expression.split_whitespace().collect();

        if parts.len() == 3 {
            let left = parts[0];
            let operator = parts[1];
            let right = parts[2];

            match operator {
                "==" => Ok(left == right),
                "!=" => Ok(left != right),
                ">" => {
                    if let (Ok(l), Ok(r)) = (left.parse::<f64>(), right.parse::<f64>()) {
                        Ok(l > r)
                    } else {
                        Ok(left > right)
                    }
                }
                "<" => {
                    if let (Ok(l), Ok(r)) = (left.parse::<f64>(), right.parse::<f64>()) {
                        Ok(l < r)
                    } else {
                        Ok(left < right)
                    }
                }
                ">=" => {
                    if let (Ok(l), Ok(r)) = (left.parse::<f64>(), right.parse::<f64>()) {
                        Ok(l >= r)
                    } else {
                        Ok(left >= right)
                    }
                }
                "<=" => {
                    if let (Ok(l), Ok(r)) = (left.parse::<f64>(), right.parse::<f64>()) {
                        Ok(l <= r)
                    } else {
                        Ok(left <= right)
                    }
                }
                _ => anyhow::bail!("Unsupported operator: {}", operator),
            }
        } else if parts.len() == 1 {
            match parts[0] {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => anyhow::bail!("Invalid boolean expression: {}", expression),
            }
        } else if expression.contains("&&") {
            let parts: Vec<&str> = expression.split("&&").collect();
            for part in parts {
                if !self.evaluate_expression(part.trim())? {
                    return Ok(false);
                }
            }
            Ok(true)
        } else if expression.contains("||") {
            let parts: Vec<&str> = expression.split("||").collect();
            for part in parts {
                if self.evaluate_expression(part.trim())? {
                    return Ok(true);
                }
            }
            Ok(false)
        } else {
            anyhow::bail!("Invalid expression format: {}", expression)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_context() -> WorkflowContext {
        WorkflowContext {
            workflow: super::super::Workflow {
                name: "test".to_string(),
                description: None,
                version: "1.0.0".to_string(),
                triggers: vec![],
                parameters: HashMap::new(),
                stages: vec![],
                on_success: vec![],
                on_failure: vec![],
                extends: None,
            },
            spec_id: Some("test-spec".to_string()),
            parameters: HashMap::from([
                ("review_required".to_string(), json!(true)),
                ("parallel_implementation".to_string(), json!(false)),
            ]),
            variables: HashMap::from([
                ("current_stage".to_string(), json!("implementation")),
                ("stage_status".to_string(), json!("success")),
            ]),
            outputs: HashMap::from([("complexity_score".to_string(), json!(8))]),
            project: HashMap::new(),
        }
    }

    #[test]
    fn test_simple_boolean() {
        let evaluator = ConditionEvaluator::new();
        let context = create_test_context();

        assert!(evaluator.evaluate("true", &context).unwrap());
        assert!(!evaluator.evaluate("false", &context).unwrap());
    }

    #[test]
    fn test_variable_expansion() {
        let evaluator = ConditionEvaluator::new();
        let context = create_test_context();

        assert!(evaluator
            .evaluate("{{ parameters.review_required }}", &context)
            .unwrap());
        assert!(!evaluator
            .evaluate("{{ parameters.parallel_implementation }}", &context)
            .unwrap());
    }

    #[test]
    fn test_comparison_operators() {
        let evaluator = ConditionEvaluator::new();
        let context = create_test_context();

        assert!(evaluator
            .evaluate("{{ outputs.complexity_score }} > 5", &context)
            .unwrap());
        assert!(!evaluator
            .evaluate("{{ outputs.complexity_score }} < 5", &context)
            .unwrap());
    }
}
