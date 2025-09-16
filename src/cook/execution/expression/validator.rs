//! Expression validator for checking syntax and types

use super::parser::Expression;
use anyhow::{anyhow, Result};

/// Expression validator
pub struct ExpressionValidator {
    // Configuration options could go here
}

impl ExpressionValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self {}
    }

    /// Validate an expression
    pub fn validate(&self, expr: &Expression) -> Result<()> {
        self.validate_expression(expr, 0)
    }

    /// Recursively validate an expression with depth limit
    fn validate_expression(&self, expr: &Expression, depth: usize) -> Result<()> {
        // Prevent stack overflow from deeply nested expressions
        if depth > 100 {
            return Err(anyhow!("Expression too deeply nested (max depth: 100)"));
        }

        match expr {
            // Literals are always valid
            Expression::Number(_)
            | Expression::String(_)
            | Expression::Boolean(_)
            | Expression::Null => Ok(()),

            // Field access - validate path
            Expression::Field(path) => {
                if path.is_empty() {
                    return Err(anyhow!("Empty field path"));
                }
                for segment in path {
                    if segment.is_empty() {
                        return Err(anyhow!("Empty field segment in path"));
                    }
                }
                Ok(())
            }

            // Variables - validate known variables
            Expression::Variable(name) => {
                if !["_index", "_key", "_value"].contains(&name.as_str()) {
                    return Err(anyhow!("Unknown variable: {}", name));
                }
                Ok(())
            }

            // Binary operations - validate both sides
            Expression::Equal(left, right)
            | Expression::NotEqual(left, right)
            | Expression::GreaterThan(left, right)
            | Expression::LessThan(left, right)
            | Expression::GreaterEqual(left, right)
            | Expression::LessEqual(left, right)
            | Expression::And(left, right)
            | Expression::Or(left, right)
            | Expression::Contains(left, right)
            | Expression::StartsWith(left, right)
            | Expression::EndsWith(left, right)
            | Expression::Matches(left, right)
            | Expression::Index(left, right) => {
                self.validate_expression(left, depth + 1)?;
                self.validate_expression(right, depth + 1)?;
                Ok(())
            }

            // Unary operations
            Expression::Not(inner)
            | Expression::IsNull(inner)
            | Expression::IsNotNull(inner)
            | Expression::IsNumber(inner)
            | Expression::IsString(inner)
            | Expression::IsBool(inner)
            | Expression::IsArray(inner)
            | Expression::IsObject(inner)
            | Expression::Length(inner)
            | Expression::Sum(inner)
            | Expression::Count(inner)
            | Expression::Min(inner)
            | Expression::Max(inner)
            | Expression::Avg(inner) => {
                self.validate_expression(inner, depth + 1)?;
                Ok(())
            }

            // Array operations
            Expression::In(expr, values) => {
                self.validate_expression(expr, depth + 1)?;
                if values.is_empty() {
                    return Err(anyhow!("IN operator requires at least one value"));
                }
                Ok(())
            }
        }
    }

    /// Validate that an expression returns a boolean (for filters)
    pub fn validate_filter(&self, expr: &Expression) -> Result<()> {
        self.validate(expr)?;

        // Check that the expression can produce a boolean result
        if !self.can_produce_boolean(expr) {
            return Err(anyhow!("Filter expression must produce a boolean result"));
        }

        Ok(())
    }

    /// Check if an expression can produce a boolean result
    fn can_produce_boolean(&self, expr: &Expression) -> bool {
        match expr {
            Expression::Boolean(_) => true,
            Expression::Equal(_, _)
            | Expression::NotEqual(_, _)
            | Expression::GreaterThan(_, _)
            | Expression::LessThan(_, _)
            | Expression::GreaterEqual(_, _)
            | Expression::LessEqual(_, _)
            | Expression::And(_, _)
            | Expression::Or(_, _)
            | Expression::Not(_)
            | Expression::Contains(_, _)
            | Expression::StartsWith(_, _)
            | Expression::EndsWith(_, _)
            | Expression::Matches(_, _)
            | Expression::IsNull(_)
            | Expression::IsNotNull(_)
            | Expression::IsNumber(_)
            | Expression::IsString(_)
            | Expression::IsBool(_)
            | Expression::IsArray(_)
            | Expression::IsObject(_)
            | Expression::In(_, _) => true,
            _ => false,
        }
    }

    /// Get a list of fields accessed by an expression
    pub fn get_accessed_fields(&self, expr: &Expression) -> Vec<String> {
        let mut fields = Vec::new();
        self.collect_fields(expr, &mut fields);
        fields.sort();
        fields.dedup();
        fields
    }

    /// Recursively collect field names from an expression
    fn collect_fields(&self, expr: &Expression, fields: &mut Vec<String>) {
        match expr {
            Expression::Field(path) => {
                fields.push(path.join("."));
            }
            Expression::Equal(left, right)
            | Expression::NotEqual(left, right)
            | Expression::GreaterThan(left, right)
            | Expression::LessThan(left, right)
            | Expression::GreaterEqual(left, right)
            | Expression::LessEqual(left, right)
            | Expression::And(left, right)
            | Expression::Or(left, right)
            | Expression::Contains(left, right)
            | Expression::StartsWith(left, right)
            | Expression::EndsWith(left, right)
            | Expression::Matches(left, right)
            | Expression::Index(left, right) => {
                self.collect_fields(left, fields);
                self.collect_fields(right, fields);
            }
            Expression::Not(inner)
            | Expression::IsNull(inner)
            | Expression::IsNotNull(inner)
            | Expression::IsNumber(inner)
            | Expression::IsString(inner)
            | Expression::IsBool(inner)
            | Expression::IsArray(inner)
            | Expression::IsObject(inner)
            | Expression::Length(inner)
            | Expression::Sum(inner)
            | Expression::Count(inner)
            | Expression::Min(inner)
            | Expression::Max(inner)
            | Expression::Avg(inner) => {
                self.collect_fields(inner, fields);
            }
            Expression::In(expr, _) => {
                self.collect_fields(expr, fields);
            }
            _ => {}
        }
    }
}

impl Default for ExpressionValidator {
    fn default() -> Self {
        Self::new()
    }
}
