//! Expression evaluation engine for filtering and sorting MapReduce work items
//!
//! Provides a comprehensive expression system supporting:
//! - Comparison operators: ==, !=, >, <, >=, <=
//! - Logical operators: &&, ||, !
//! - String operations: contains, starts_with, ends_with, matches
//! - Type checking: is_null, is_number, is_string, is_bool, is_array, is_object
//! - Computed fields: length, sum, count, min, max, avg
//! - JSONPath field access with array support
//! - Multiple sort keys with custom null handling

pub mod ast;
pub mod evaluator;
pub mod optimizer;
pub mod parser;
pub mod validator;

use anyhow::Result;
use serde_json::Value;

pub use ast::{Expression, NullHandling, SortDirection, SortKey};
pub use evaluator::{Collation, CompiledFilter, CompiledSort, ExpressionEvaluator};
pub use optimizer::ExpressionOptimizer;
pub use parser::ExpressionParser;
pub use validator::ExpressionValidator;

/// Expression engine for compiling and executing filter/sort expressions
pub struct ExpressionEngine {
    parser: ExpressionParser,
    evaluator: ExpressionEvaluator,
    optimizer: ExpressionOptimizer,
    validator: ExpressionValidator,
}

impl ExpressionEngine {
    /// Create a new expression engine
    pub fn new() -> Self {
        Self {
            parser: ExpressionParser::new(),
            evaluator: ExpressionEvaluator::new(),
            optimizer: ExpressionOptimizer::new(),
            validator: ExpressionValidator::new(),
        }
    }

    /// Compile a filter expression
    pub fn compile_filter(&mut self, expr: &str) -> Result<CompiledFilter> {
        // Parse the expression
        let ast = self.parser.parse_filter(expr)?;

        // Validate the expression
        self.validator.validate(&ast)?;

        // Optimize the expression
        let optimized = self.optimizer.optimize(ast)?;

        // Create compiled filter
        Ok(CompiledFilter::new(optimized, self.evaluator.clone()))
    }

    /// Compile a sort expression
    pub fn compile_sort(&mut self, expr: &str) -> Result<CompiledSort> {
        // Parse the sort keys
        let sort_keys = self.parser.parse_sort(expr)?;

        // Validate sort keys
        for key in &sort_keys {
            self.validator.validate(&key.expression)?;
        }

        // Create compiled sort
        Ok(CompiledSort::new(sort_keys))
    }

    /// Compile a sort expression with custom collation
    pub fn compile_sort_with_collation(
        &mut self,
        expr: &str,
        collation: Collation,
    ) -> Result<CompiledSort> {
        // Parse the sort keys
        let sort_keys = self.parser.parse_sort(expr)?;

        // Validate sort keys
        for key in &sort_keys {
            self.validator.validate(&key.expression)?;
        }

        // Create compiled sort with collation
        Ok(CompiledSort::with_collation(sort_keys, collation))
    }

    /// Evaluate a filter expression directly without compilation
    pub fn evaluate_filter(&mut self, expr: &str, item: &Value) -> Result<bool> {
        let filter = self.compile_filter(expr)?;
        filter.evaluate(item)
    }

    /// Get expression metadata for debugging
    pub fn analyze(&self, expr: &str) -> Result<ExpressionMetadata> {
        let ast = self.parser.parse_filter(expr)?;
        let analyzer = ExpressionAnalyzer;
        Ok(analyzer.analyze(&ast))
    }
}

impl Default for ExpressionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata about an expression
#[derive(Debug, Clone)]
pub struct ExpressionMetadata {
    /// Fields accessed by the expression
    pub accessed_fields: Vec<String>,
    /// Functions used in the expression
    pub used_functions: Vec<String>,
    /// Estimated complexity score
    pub complexity: u32,
    /// Whether the expression can be indexed
    pub indexable: bool,
    /// Optimization hints
    pub hints: Vec<String>,
}

/// Analyzer for expression metadata
struct ExpressionAnalyzer;

impl ExpressionAnalyzer {
    fn analyze(&self, _expr: &Expression) -> ExpressionMetadata {
        // This would analyze the expression tree
        ExpressionMetadata {
            accessed_fields: vec![],
            used_functions: vec![],
            complexity: 1,
            indexable: false,
            hints: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_expression_engine_filter() {
        let mut engine = ExpressionEngine::new();
        let filter = engine
            .compile_filter("priority > 5 && status == 'active'")
            .unwrap();

        let item1 = json!({"priority": 7, "status": "active"});
        let item2 = json!({"priority": 3, "status": "active"});
        let item3 = json!({"priority": 7, "status": "inactive"});

        assert!(filter.evaluate(&item1).unwrap());
        assert!(!filter.evaluate(&item2).unwrap());
        assert!(!filter.evaluate(&item3).unwrap());
    }

    #[test]
    fn test_expression_engine_sort() {
        let mut engine = ExpressionEngine::new();
        let sort = engine.compile_sort("priority DESC, name ASC").unwrap();

        let mut items = vec![
            json!({"priority": 3, "name": "B"}),
            json!({"priority": 5, "name": "A"}),
            json!({"priority": 5, "name": "C"}),
        ];

        sort.apply(&mut items).unwrap();

        assert_eq!(items[0]["priority"], 5);
        assert_eq!(items[0]["name"], "A");
        assert_eq!(items[1]["priority"], 5);
        assert_eq!(items[1]["name"], "C");
        assert_eq!(items[2]["priority"], 3);
    }
}
