//! Expression optimizer for improving performance

use super::ast::Expression;
use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;

/// Optimizer configuration
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    pub enable_constant_folding: bool,
    pub enable_cse: bool,
    pub enable_dce: bool,
    pub enable_algebraic: bool,
    pub enable_short_circuit: bool,
    pub cache_threshold: usize,
    pub max_passes: usize,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            enable_constant_folding: true,
            enable_cse: true,
            enable_dce: true,
            enable_algebraic: true,
            enable_short_circuit: true,
            cache_threshold: 3, // Cache expressions accessed 3+ times
            max_passes: 5,      // Max optimization passes
        }
    }
}

/// Optimization statistics
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    pub expressions_optimized: usize,
    pub constants_folded: usize,
    pub subexpressions_eliminated: usize,
    pub dead_code_removed: usize,
    pub algebraic_simplifications: usize,
    pub optimization_time: Duration,
}

/// Expression optimizer
pub struct ExpressionOptimizer {
    config: OptimizerConfig,
    stats: OptimizationStats,
    cache: SubExpressionCache,
    next_cache_id: usize,
}

impl ExpressionOptimizer {
    /// Create a new optimizer
    pub fn new() -> Self {
        Self::with_config(OptimizerConfig::default())
    }

    /// Create optimizer with custom configuration
    pub fn with_config(config: OptimizerConfig) -> Self {
        Self {
            config,
            stats: OptimizationStats::default(),
            cache: SubExpressionCache::new(),
            next_cache_id: 0,
        }
    }

    /// Set configuration
    pub fn set_config(&mut self, config: OptimizerConfig) {
        self.config = config;
    }

    /// Get optimization statistics
    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }

    /// Reset cache and statistics
    pub fn reset(&mut self) {
        self.cache = SubExpressionCache::new();
        self.stats = OptimizationStats::default();
        self.next_cache_id = 0;
    }

    /// Optimize an expression tree
    pub fn optimize(&mut self, expr: Expression) -> Result<Expression> {
        let start = std::time::Instant::now();
        let mut optimized = expr;
        let mut pass = 0;
        let mut changed = true;

        // Apply optimization passes until no more changes or max passes reached
        while changed && pass < self.config.max_passes {
            let before = optimized.clone();

            if self.config.enable_constant_folding {
                optimized = self.constant_folding(optimized)?;
            }

            if self.config.enable_algebraic {
                optimized = self.algebraic_simplification(optimized)?;
            }

            if self.config.enable_dce {
                optimized = self.dead_code_elimination(optimized)?;
            }

            if self.config.enable_cse {
                optimized = self.common_subexpression_elimination(optimized)?;
            }

            if self.config.enable_short_circuit {
                optimized = self.short_circuit_optimization(optimized)?;
            }

            changed = !expressions_equal(&before, &optimized);
            pass += 1;
        }

        self.stats.expressions_optimized += 1;
        self.stats.optimization_time = start.elapsed();

        Ok(optimized)
    }

    /// Constant folding - evaluate constant expressions at compile time
    fn constant_folding(&mut self, expr: Expression) -> Result<Expression> {
        match expr {
            // Delegate logical operators to specialized function
            Expression::And(_, _) | Expression::Or(_, _) | Expression::Not(_) => {
                fold_logical_operators(self, expr)
            }
            // Fold constant comparisons
            Expression::Equal(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;
                fold_equal_comparison(&mut self.stats, left, right)
            }

            Expression::NotEqual(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;
                fold_not_equal_comparison(&mut self.stats, left, right)
            }

            // Fold comparison operators
            Expression::GreaterThan(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;
                fold_numeric_comparison(
                    &mut self.stats,
                    NumericComparisonOp::GreaterThan,
                    left,
                    right,
                )
            }
            Expression::LessThan(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;
                fold_numeric_comparison(&mut self.stats, NumericComparisonOp::LessThan, left, right)
            }
            Expression::GreaterEqual(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;
                fold_numeric_comparison(
                    &mut self.stats,
                    NumericComparisonOp::GreaterEqual,
                    left,
                    right,
                )
            }
            Expression::LessEqual(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;
                fold_numeric_comparison(
                    &mut self.stats,
                    NumericComparisonOp::LessEqual,
                    left,
                    right,
                )
            }

            // Delegate type checks to specialized function
            Expression::IsNull(_)
            | Expression::IsNotNull(_)
            | Expression::IsNumber(_)
            | Expression::IsString(_)
            | Expression::IsBool(_)
            | Expression::IsArray(_)
            | Expression::IsObject(_) => fold_type_checks(self, expr),

            // Delegate string operators to specialized function
            Expression::Contains(_, _)
            | Expression::StartsWith(_, _)
            | Expression::EndsWith(_, _)
            | Expression::Matches(_, _) => fold_string_operators(self, expr),
            Expression::Index(base, idx) => Ok(Expression::Index(
                Box::new(self.constant_folding(*base)?),
                Box::new(self.constant_folding(*idx)?),
            )),
            Expression::ArrayWildcard(base, path) => Ok(Expression::ArrayWildcard(
                Box::new(self.constant_folding(*base)?),
                path,
            )),

            // Aggregate functions
            Expression::Length(inner) => {
                Ok(Expression::Length(Box::new(self.constant_folding(*inner)?)))
            }
            Expression::Sum(inner) => Ok(Expression::Sum(Box::new(self.constant_folding(*inner)?))),
            Expression::Count(inner) => {
                Ok(Expression::Count(Box::new(self.constant_folding(*inner)?)))
            }
            Expression::Min(inner) => Ok(Expression::Min(Box::new(self.constant_folding(*inner)?))),
            Expression::Max(inner) => Ok(Expression::Max(Box::new(self.constant_folding(*inner)?))),
            Expression::Avg(inner) => Ok(Expression::Avg(Box::new(self.constant_folding(*inner)?))),

            // Literals and simple expressions pass through
            _ => Ok(expr),
        }
    }

    /// Common subexpression elimination
    fn common_subexpression_elimination(&mut self, expr: Expression) -> Result<Expression> {
        self.cse_recursive(expr, &mut HashMap::new())
    }

    /// Recursive CSE helper
    fn cse_recursive(
        &mut self,
        expr: Expression,
        expr_map: &mut HashMap<u64, Expression>,
    ) -> Result<Expression> {
        // Hash the expression
        let hash = hash_expression(&expr);

        // Check if we've seen this expression before
        if let Some(cached) = expr_map.get(&hash) {
            self.stats.subexpressions_eliminated += 1;
            return Ok(cached.clone());
        }

        // Process the expression
        let result = match expr {
            Expression::And(left, right) => {
                let left = self.cse_recursive(*left, expr_map)?;
                let right = self.cse_recursive(*right, expr_map)?;
                Expression::And(Box::new(left), Box::new(right))
            }
            Expression::Or(left, right) => {
                let left = self.cse_recursive(*left, expr_map)?;
                let right = self.cse_recursive(*right, expr_map)?;
                Expression::Or(Box::new(left), Box::new(right))
            }
            Expression::Not(inner) => {
                let inner = self.cse_recursive(*inner, expr_map)?;
                Expression::Not(Box::new(inner))
            }
            Expression::Equal(left, right) => {
                let left = self.cse_recursive(*left, expr_map)?;
                let right = self.cse_recursive(*right, expr_map)?;
                Expression::Equal(Box::new(left), Box::new(right))
            }
            Expression::NotEqual(left, right) => {
                let left = self.cse_recursive(*left, expr_map)?;
                let right = self.cse_recursive(*right, expr_map)?;
                Expression::NotEqual(Box::new(left), Box::new(right))
            }
            Expression::GreaterThan(left, right) => {
                let left = self.cse_recursive(*left, expr_map)?;
                let right = self.cse_recursive(*right, expr_map)?;
                Expression::GreaterThan(Box::new(left), Box::new(right))
            }
            Expression::LessThan(left, right) => {
                let left = self.cse_recursive(*left, expr_map)?;
                let right = self.cse_recursive(*right, expr_map)?;
                Expression::LessThan(Box::new(left), Box::new(right))
            }
            Expression::GreaterEqual(left, right) => {
                let left = self.cse_recursive(*left, expr_map)?;
                let right = self.cse_recursive(*right, expr_map)?;
                Expression::GreaterEqual(Box::new(left), Box::new(right))
            }
            Expression::LessEqual(left, right) => {
                let left = self.cse_recursive(*left, expr_map)?;
                let right = self.cse_recursive(*right, expr_map)?;
                Expression::LessEqual(Box::new(left), Box::new(right))
            }
            _ => expr,
        };

        // Cache complex expressions
        let complexity = self.estimate_complexity(&result);
        if complexity > self.config.cache_threshold as u32 {
            expr_map.insert(hash, result.clone());
        }

        Ok(result)
    }

    /// Algebraic simplification
    fn algebraic_simplification(&mut self, expr: Expression) -> Result<Expression> {
        match expr {
            // Simplify x && x => x
            Expression::And(ref left, ref right) if expressions_equal(left, right) => {
                self.stats.algebraic_simplifications += 1;
                Ok(*left.clone())
            }
            // Simplify x || x => x
            Expression::Or(ref left, ref right) if expressions_equal(left, right) => {
                self.stats.algebraic_simplifications += 1;
                Ok(*left.clone())
            }
            // Simplify !(x == y) => x != y
            Expression::Not(inner) => match *inner {
                Expression::Equal(left, right) => {
                    self.stats.algebraic_simplifications += 1;
                    Ok(Expression::NotEqual(left, right))
                }
                Expression::NotEqual(left, right) => {
                    self.stats.algebraic_simplifications += 1;
                    Ok(Expression::Equal(left, right))
                }
                Expression::GreaterThan(left, right) => {
                    self.stats.algebraic_simplifications += 1;
                    Ok(Expression::LessEqual(left, right))
                }
                Expression::LessThan(left, right) => {
                    self.stats.algebraic_simplifications += 1;
                    Ok(Expression::GreaterEqual(left, right))
                }
                Expression::GreaterEqual(left, right) => {
                    self.stats.algebraic_simplifications += 1;
                    Ok(Expression::LessThan(left, right))
                }
                Expression::LessEqual(left, right) => {
                    self.stats.algebraic_simplifications += 1;
                    Ok(Expression::GreaterThan(left, right))
                }
                _ => Ok(Expression::Not(Box::new(*inner))),
            },
            // Recursively simplify nested expressions
            Expression::And(left, right) => {
                let left = self.algebraic_simplification(*left)?;
                let right = self.algebraic_simplification(*right)?;
                Ok(Expression::And(Box::new(left), Box::new(right)))
            }
            Expression::Or(left, right) => {
                let left = self.algebraic_simplification(*left)?;
                let right = self.algebraic_simplification(*right)?;
                Ok(Expression::Or(Box::new(left), Box::new(right)))
            }
            _ => Ok(expr),
        }
    }

    /// Dead code elimination
    fn dead_code_elimination(&mut self, expr: Expression) -> Result<Expression> {
        match expr {
            // Remove unreachable branches in conditionals
            Expression::And(left, right) => {
                let left = self.dead_code_elimination(*left)?;
                let right = self.dead_code_elimination(*right)?;

                // If left is always false, right is never evaluated
                if let Expression::Boolean(false) = left {
                    self.stats.dead_code_removed += 1;
                    return Ok(Expression::Boolean(false));
                }

                Ok(Expression::And(Box::new(left), Box::new(right)))
            }
            Expression::Or(left, right) => {
                let left = self.dead_code_elimination(*left)?;
                let right = self.dead_code_elimination(*right)?;

                // If left is always true, right is never evaluated
                if let Expression::Boolean(true) = left {
                    self.stats.dead_code_removed += 1;
                    return Ok(Expression::Boolean(true));
                }

                Ok(Expression::Or(Box::new(left), Box::new(right)))
            }
            // Recursively eliminate dead code
            Expression::Not(inner) => {
                let inner = self.dead_code_elimination(*inner)?;
                Ok(Expression::Not(Box::new(inner)))
            }
            _ => Ok(expr),
        }
    }

    /// Short-circuit optimization - reorder conditions for better performance
    fn short_circuit_optimization(&mut self, expr: Expression) -> Result<Expression> {
        match expr {
            Expression::And(left, right) => {
                // Put cheaper/more selective conditions first
                let left = self.short_circuit_optimization(*left)?;
                let right = self.short_circuit_optimization(*right)?;

                // Estimate complexity and reorder if beneficial
                let left_complexity = self.estimate_complexity(&left);
                let right_complexity = self.estimate_complexity(&right);

                if right_complexity < left_complexity {
                    Ok(Expression::And(Box::new(right), Box::new(left)))
                } else {
                    Ok(Expression::And(Box::new(left), Box::new(right)))
                }
            }
            Expression::Or(left, right) => {
                // Similar optimization for OR
                let left = self.short_circuit_optimization(*left)?;
                let right = self.short_circuit_optimization(*right)?;

                let left_complexity = self.estimate_complexity(&left);
                let right_complexity = self.estimate_complexity(&right);

                if right_complexity < left_complexity {
                    Ok(Expression::Or(Box::new(right), Box::new(left)))
                } else {
                    Ok(Expression::Or(Box::new(left), Box::new(right)))
                }
            }
            _ => Ok(expr),
        }
    }

    /// Estimate expression complexity for optimization decisions
    #[allow(clippy::only_used_in_recursion)]
    fn estimate_complexity(&self, expr: &Expression) -> u32 {
        match expr {
            Expression::Number(_)
            | Expression::String(_)
            | Expression::Boolean(_)
            | Expression::Null => 1,
            Expression::Field(_) => 2,
            Expression::Variable(_) => 2,
            Expression::Equal(_, _) | Expression::NotEqual(_, _) => 3,
            Expression::GreaterThan(_, _) | Expression::LessThan(_, _) => 3,
            Expression::GreaterEqual(_, _) | Expression::LessEqual(_, _) => 3,
            Expression::Contains(_, _)
            | Expression::StartsWith(_, _)
            | Expression::EndsWith(_, _) => 5,
            Expression::Matches(_, _) => 10, // Regex is expensive
            Expression::And(left, right) | Expression::Or(left, right) => {
                self.estimate_complexity(left) + self.estimate_complexity(right)
            }
            Expression::Not(inner) => self.estimate_complexity(inner) + 1,
            Expression::IsNull(_) | Expression::IsNotNull(_) => 2,
            Expression::IsNumber(_) | Expression::IsString(_) | Expression::IsBool(_) => 2,
            Expression::IsArray(_) | Expression::IsObject(_) => 2,
            Expression::Length(_) | Expression::Count(_) => 4,
            Expression::Sum(_) | Expression::Min(_) | Expression::Max(_) | Expression::Avg(_) => 10,
            Expression::In(_, values) => 3 + values.len() as u32,
            Expression::Index(_, _) => 3,
            Expression::ArrayWildcard(base, path) => {
                self.estimate_complexity(base) + 3 + path.len() as u32
            }
        }
    }
}

impl Default for ExpressionOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

// Internal modules
mod cache;
mod folding;
mod utils;

// Import from internal modules
use cache::SubExpressionCache;
use folding::{
    fold_equal_comparison, fold_logical_operators, fold_not_equal_comparison,
    fold_numeric_comparison, fold_string_operators, fold_type_checks, NumericComparisonOp,
};
use utils::{expressions_equal, hash_expression};

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
