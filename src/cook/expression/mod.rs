//! Expression evaluation for conditional workflow execution

use anyhow::{anyhow, Result};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

mod parser;
mod value;

pub use parser::{parse_expression, ComparisonOp, Expression, LogicalOp};
pub use value::Value;

/// Evaluates expressions for conditional workflow execution
#[derive(Debug)]
pub struct ExpressionEvaluator {
    parser: parser::ExpressionParser,
}

impl Default for ExpressionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ExpressionEvaluator {
    /// Create a new expression evaluator
    pub fn new() -> Self {
        Self {
            parser: parser::ExpressionParser::new(),
        }
    }

    /// Evaluate an expression with the given variable context
    pub fn evaluate(&self, expression: &str, context: &VariableContext) -> Result<bool> {
        // Parse the expression
        let expr = self.parser.parse(expression)?;

        // Evaluate and convert to boolean
        let value = self.evaluate_expression(&expr, context)?;
        self.to_boolean(value)
    }

    /// Evaluate an expression node
    fn evaluate_expression(&self, expr: &Expression, context: &VariableContext) -> Result<Value> {
        match expr {
            Expression::Variable(name) => context.get(name),
            Expression::Literal(value) => Ok(value.clone()),
            Expression::Comparison { left, op, right } => {
                let left_val = self.evaluate_expression(left, context)?;
                let right_val = self.evaluate_expression(right, context)?;
                self.compare(left_val, op, right_val)
            }
            Expression::Logical { left, op, right } => match op {
                LogicalOp::And => {
                    let left_bool = self.to_boolean(self.evaluate_expression(left, context)?)?;
                    if !left_bool {
                        return Ok(Value::Bool(false));
                    }
                    let right_val = self.evaluate_expression(right, context)?;
                    Ok(Value::Bool(self.to_boolean(right_val)?))
                }
                LogicalOp::Or => {
                    let left_bool = self.to_boolean(self.evaluate_expression(left, context)?)?;
                    if left_bool {
                        return Ok(Value::Bool(true));
                    }
                    let right_val = self.evaluate_expression(right, context)?;
                    Ok(Value::Bool(self.to_boolean(right_val)?))
                }
            },
            Expression::Not(inner) => {
                let val = self.evaluate_expression(inner, context)?;
                let bool_val = self.to_boolean(val)?;
                Ok(Value::Bool(!bool_val))
            }
            Expression::Exists(var_name) => Ok(Value::Bool(context.exists(var_name))),
        }
    }

    /// Compare two values
    fn compare(&self, left: Value, op: &ComparisonOp, right: Value) -> Result<Value> {
        // Try to coerce values to compatible types for comparison
        let (left_coerced, right_coerced) = self.coerce_for_comparison(left.clone(), right.clone());

        let result = match op {
            ComparisonOp::Equal => left_coerced == right_coerced,
            ComparisonOp::NotEqual => left_coerced != right_coerced,
            ComparisonOp::GreaterThan => match (&left_coerced, &right_coerced) {
                (Value::Number(l), Value::Number(r)) => l > r,
                (Value::String(l), Value::String(r)) => l > r,
                _ => return Err(anyhow!("Cannot compare {:?} and {:?} with >", left, right)),
            },
            ComparisonOp::LessThan => match (&left_coerced, &right_coerced) {
                (Value::Number(l), Value::Number(r)) => l < r,
                (Value::String(l), Value::String(r)) => l < r,
                _ => return Err(anyhow!("Cannot compare {:?} and {:?} with <", left, right)),
            },
            ComparisonOp::GreaterThanOrEqual => match (&left_coerced, &right_coerced) {
                (Value::Number(l), Value::Number(r)) => l >= r,
                (Value::String(l), Value::String(r)) => l >= r,
                _ => return Err(anyhow!("Cannot compare {:?} and {:?} with >=", left, right)),
            },
            ComparisonOp::LessThanOrEqual => match (&left_coerced, &right_coerced) {
                (Value::Number(l), Value::Number(r)) => l <= r,
                (Value::String(l), Value::String(r)) => l <= r,
                _ => return Err(anyhow!("Cannot compare {:?} and {:?} with <=", left, right)),
            },
        };
        Ok(Value::Bool(result))
    }

    /// Coerce values to compatible types for comparison
    fn coerce_for_comparison(&self, left: Value, right: Value) -> (Value, Value) {
        match (&left, &right) {
            // If one is a string that looks like a number and the other is a number, convert the string
            (Value::String(s), Value::Number(_)) => {
                if let Ok(n) = s.parse::<f64>() {
                    (Value::Number(n), right)
                } else {
                    (left, right)
                }
            }
            (Value::Number(_), Value::String(s)) => {
                if let Ok(n) = s.parse::<f64>() {
                    (left, Value::Number(n))
                } else {
                    (left, right)
                }
            }
            // Otherwise keep original types
            _ => (left, right),
        }
    }

    /// Convert a value to boolean
    fn to_boolean(&self, value: Value) -> Result<bool> {
        match value {
            Value::Bool(b) => Ok(b),
            Value::Number(n) => Ok(n != 0.0),
            Value::String(s) => Ok(!s.is_empty() && s != "false" && s != "0"),
            Value::Null => Ok(false),
        }
    }
}

/// Variable context for expression evaluation
#[derive(Debug, Clone)]
pub struct VariableContext {
    variables: HashMap<String, Value>,
}

impl Default for VariableContext {
    fn default() -> Self {
        Self::new()
    }
}

impl VariableContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Set a variable value
    pub fn set(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }

    /// Set a variable from a string value
    pub fn set_string(&mut self, name: String, value: String) {
        self.variables.insert(name, Value::String(value));
    }

    /// Set a boolean variable
    pub fn set_bool(&mut self, name: String, value: bool) {
        self.variables.insert(name, Value::Bool(value));
    }

    /// Set a numeric variable
    pub fn set_number(&mut self, name: String, value: f64) {
        self.variables.insert(name, Value::Number(value));
    }

    /// Set step result variables
    pub fn set_step_result(
        &mut self,
        step_name: &str,
        success: bool,
        exit_code: i32,
        output: Option<String>,
    ) {
        self.set_bool(format!("{}.success", step_name), success);
        self.set_number(format!("{}.exit_code", step_name), exit_code as f64);

        if let Some(output) = output {
            self.set_string(format!("{}.output", step_name), output);
        }
    }

    /// Get a variable value, supporting nested access
    pub fn get(&self, name: &str) -> Result<Value> {
        // Handle nested access (e.g., "result.data.status")
        let parts: Vec<&str> = name.split('.').collect();

        if parts.is_empty() {
            return Ok(Value::Null);
        }

        // Get the base variable
        let base = self.variables.get(parts[0]).cloned().unwrap_or(Value::Null);

        if parts.len() == 1 {
            return Ok(base);
        }

        // Handle nested access for JSON values
        if let Value::String(json_str) = &base {
            // Try to parse as JSON for nested access
            if let Ok(json_val) = serde_json::from_str::<JsonValue>(json_str) {
                return self.get_nested_json(&json_val, &parts[1..]);
            }
        }

        // Check for composite keys (e.g., "step.success")
        self.variables
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow!("Variable '{}' not found", name))
    }

    /// Check if a variable exists
    pub fn exists(&self, name: &str) -> bool {
        self.variables.contains_key(name) || {
            // Check for nested variable
            if let Some(dot_pos) = name.find('.') {
                let base = &name[..dot_pos];
                self.variables.contains_key(base)
            } else {
                false
            }
        }
    }

    /// Get nested value from JSON
    fn get_nested_json(&self, json: &JsonValue, path: &[&str]) -> Result<Value> {
        let mut current = json;

        for part in path {
            current = current
                .get(part)
                .ok_or_else(|| anyhow!("Path '{}' not found in JSON", part))?;
        }

        Ok(match current {
            JsonValue::Bool(b) => Value::Bool(*b),
            JsonValue::Number(n) => {
                if let Some(f) = n.as_f64() {
                    Value::Number(f)
                } else {
                    Value::Null
                }
            }
            JsonValue::String(s) => Value::String(s.clone()),
            JsonValue::Null => Value::Null,
            _ => Value::String(current.to_string()),
        })
    }

    /// Create from a HashMap of string values
    pub fn from_strings(map: HashMap<String, String>) -> Self {
        let mut context = Self::new();
        for (k, v) in map {
            context.set_string(k, v);
        }
        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_boolean() {
        let evaluator = ExpressionEvaluator::new();
        let mut context = VariableContext::new();
        context.set_bool("test".to_string(), true);

        let result = evaluator.evaluate("${test}", &context).unwrap();
        assert!(result);
    }

    #[test]
    fn test_comparison() {
        let evaluator = ExpressionEvaluator::new();
        let mut context = VariableContext::new();
        context.set_number("score".to_string(), 85.0);

        let result = evaluator.evaluate("${score} >= 80", &context).unwrap();
        assert!(result);
    }

    #[test]
    fn test_logical_operators() {
        let evaluator = ExpressionEvaluator::new();
        let mut context = VariableContext::new();
        context.set_bool("a".to_string(), true);
        context.set_bool("b".to_string(), false);

        let result = evaluator.evaluate("${a} && ${b}", &context).unwrap();
        assert!(!result);

        let result = evaluator.evaluate("${a} || ${b}", &context).unwrap();
        assert!(result);
    }

    #[test]
    fn test_string_comparison() {
        let evaluator = ExpressionEvaluator::new();
        let mut context = VariableContext::new();
        context.set_string("env".to_string(), "production".to_string());

        let result = evaluator
            .evaluate("${env} == 'production'", &context)
            .unwrap();
        assert!(result);
    }

    #[test]
    fn test_step_result() {
        let evaluator = ExpressionEvaluator::new();
        let mut context = VariableContext::new();
        context.set_step_result("build", true, 0, Some("Build successful".to_string()));

        let result = evaluator.evaluate("${build.success}", &context).unwrap();
        assert!(result);

        let result = evaluator
            .evaluate("${build.exit_code} == 0", &context)
            .unwrap();
        assert!(result);
    }

    #[test]
    fn test_undefined_variable() {
        let evaluator = ExpressionEvaluator::new();
        let context = VariableContext::new();

        // Undefined variables should be treated as null/false
        let result = evaluator.evaluate("${undefined}", &context);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_variable_exists() {
        let evaluator = ExpressionEvaluator::new();
        let mut context = VariableContext::new();
        context.set_string("defined".to_string(), "value".to_string());

        let result = evaluator.evaluate("${defined.exists}", &context).unwrap();
        assert!(result);

        let result = evaluator.evaluate("${undefined.exists}", &context).unwrap();
        assert!(!result);
    }
}
