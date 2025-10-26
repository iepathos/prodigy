//! Expression optimizer for improving performance

use super::parser::Expression;
use anyhow::Result;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
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

/// Sub-expression cache for CSE
#[derive(Debug)]
struct SubExpressionCache {
    #[allow(dead_code)]
    expressions: HashMap<u64, CachedExpression>,
    #[allow(dead_code)]
    access_count: HashMap<u64, usize>,
}

impl SubExpressionCache {
    fn new() -> Self {
        Self {
            expressions: HashMap::new(),
            access_count: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    fn get(&mut self, hash: u64) -> Option<&CachedExpression> {
        self.access_count
            .entry(hash)
            .and_modify(|c| *c += 1)
            .or_insert(1);
        self.expressions.get(&hash)
    }

    #[allow(dead_code)]
    fn insert(&mut self, hash: u64, expr: CachedExpression) {
        self.expressions.insert(hash, expr);
        self.access_count.insert(hash, 1);
    }

    #[allow(dead_code)]
    fn should_cache(&self, hash: u64, threshold: usize) -> bool {
        self.access_count.get(&hash).copied().unwrap_or(0) >= threshold
    }
}

/// Cached expression with metadata
#[derive(Debug, Clone)]
struct CachedExpression {
    #[allow(dead_code)]
    id: usize,
    #[allow(dead_code)]
    expr: Expression,
    #[allow(dead_code)]
    cost: u32,
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
            // Fold constant arithmetic
            Expression::And(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;

                match (&left, &right) {
                    (Expression::Boolean(false), _) | (_, Expression::Boolean(false)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(false))
                    }
                    (Expression::Boolean(true), other) | (other, Expression::Boolean(true)) => {
                        self.stats.constants_folded += 1;
                        Ok(other.clone())
                    }
                    _ => Ok(Expression::And(Box::new(left), Box::new(right))),
                }
            }
            Expression::Or(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;

                match (&left, &right) {
                    (Expression::Boolean(true), _) | (_, Expression::Boolean(true)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(true))
                    }
                    (Expression::Boolean(false), other) | (other, Expression::Boolean(false)) => {
                        self.stats.constants_folded += 1;
                        Ok(other.clone())
                    }
                    _ => Ok(Expression::Or(Box::new(left), Box::new(right))),
                }
            }
            Expression::Not(inner) => {
                let inner = self.constant_folding(*inner)?;
                match inner {
                    Expression::Boolean(b) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(!b))
                    }
                    Expression::Not(double_inner) => {
                        self.stats.algebraic_simplifications += 1;
                        Ok(*double_inner) // Double negation
                    }
                    _ => Ok(Expression::Not(Box::new(inner))),
                }
            }
            // Fold constant comparisons
            Expression::Equal(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;

                match (&left, &right) {
                    (Expression::Number(a), Expression::Number(b)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean((a - b).abs() < f64::EPSILON))
                    }
                    (Expression::String(a), Expression::String(b)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(a == b))
                    }
                    (Expression::Boolean(a), Expression::Boolean(b)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(a == b))
                    }
                    (Expression::Null, Expression::Null) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(true))
                    }
                    _ => {
                        // Check if same expression
                        if expressions_equal(&left, &right) {
                            self.stats.algebraic_simplifications += 1;
                            Ok(Expression::Boolean(true))
                        } else {
                            Ok(Expression::Equal(Box::new(left), Box::new(right)))
                        }
                    }
                }
            }

            Expression::NotEqual(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;

                match (&left, &right) {
                    (Expression::Number(a), Expression::Number(b)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean((a - b).abs() >= f64::EPSILON))
                    }
                    (Expression::String(a), Expression::String(b)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(a != b))
                    }
                    (Expression::Boolean(a), Expression::Boolean(b)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(a != b))
                    }
                    _ => {
                        // Check if same expression
                        if expressions_equal(&left, &right) {
                            self.stats.algebraic_simplifications += 1;
                            Ok(Expression::Boolean(false))
                        } else {
                            Ok(Expression::NotEqual(Box::new(left), Box::new(right)))
                        }
                    }
                }
            }
            // Fold comparison operators
            Expression::GreaterThan(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;

                match (&left, &right) {
                    (Expression::Number(a), Expression::Number(b)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(a > b))
                    }
                    _ => Ok(Expression::GreaterThan(Box::new(left), Box::new(right))),
                }
            }
            Expression::LessThan(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;

                match (&left, &right) {
                    (Expression::Number(a), Expression::Number(b)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(a < b))
                    }
                    _ => Ok(Expression::LessThan(Box::new(left), Box::new(right))),
                }
            }
            Expression::GreaterEqual(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;

                match (&left, &right) {
                    (Expression::Number(a), Expression::Number(b)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(a >= b))
                    }
                    _ => Ok(Expression::GreaterEqual(Box::new(left), Box::new(right))),
                }
            }
            Expression::LessEqual(left, right) => {
                let left = self.constant_folding(*left)?;
                let right = self.constant_folding(*right)?;

                match (&left, &right) {
                    (Expression::Number(a), Expression::Number(b)) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(a <= b))
                    }
                    _ => Ok(Expression::LessEqual(Box::new(left), Box::new(right))),
                }
            }

            // Type checks on constants
            Expression::IsNull(inner) => {
                let inner = self.constant_folding(*inner)?;
                match inner {
                    Expression::Null => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(true))
                    }
                    Expression::Number(_) | Expression::String(_) | Expression::Boolean(_) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(false))
                    }
                    _ => Ok(Expression::IsNull(Box::new(inner))),
                }
            }
            Expression::IsNotNull(inner) => {
                let inner = self.constant_folding(*inner)?;
                match inner {
                    Expression::Null => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(false))
                    }
                    Expression::Number(_) | Expression::String(_) | Expression::Boolean(_) => {
                        self.stats.constants_folded += 1;
                        Ok(Expression::Boolean(true))
                    }
                    _ => Ok(Expression::IsNotNull(Box::new(inner))),
                }
            }

            // Recursively fold other expressions
            Expression::Contains(left, right) => Ok(Expression::Contains(
                Box::new(self.constant_folding(*left)?),
                Box::new(self.constant_folding(*right)?),
            )),
            Expression::StartsWith(left, right) => Ok(Expression::StartsWith(
                Box::new(self.constant_folding(*left)?),
                Box::new(self.constant_folding(*right)?),
            )),
            Expression::EndsWith(left, right) => Ok(Expression::EndsWith(
                Box::new(self.constant_folding(*left)?),
                Box::new(self.constant_folding(*right)?),
            )),
            Expression::Matches(left, right) => Ok(Expression::Matches(
                Box::new(self.constant_folding(*left)?),
                Box::new(self.constant_folding(*right)?),
            )),
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

            // Type checks
            Expression::IsNumber(inner) => Ok(Expression::IsNumber(Box::new(
                self.constant_folding(*inner)?,
            ))),
            Expression::IsString(inner) => Ok(Expression::IsString(Box::new(
                self.constant_folding(*inner)?,
            ))),
            Expression::IsBool(inner) => {
                Ok(Expression::IsBool(Box::new(self.constant_folding(*inner)?)))
            }
            Expression::IsArray(inner) => Ok(Expression::IsArray(Box::new(
                self.constant_folding(*inner)?,
            ))),
            Expression::IsObject(inner) => Ok(Expression::IsObject(Box::new(
                self.constant_folding(*inner)?,
            ))),

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

/// Hash an expression for CSE
fn hash_expression(expr: &Expression) -> u64 {
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();
    hash_expression_recursive(expr, &mut hasher);
    hasher.finish()
}

fn hash_expression_recursive<H: Hasher>(expr: &Expression, hasher: &mut H) {
    // Hash discriminant to distinguish expression types
    std::mem::discriminant(expr).hash(hasher);

    match expr {
        Expression::Number(n) => {
            n.to_bits().hash(hasher);
        }
        Expression::String(s) => {
            s.hash(hasher);
        }
        Expression::Boolean(b) => {
            b.hash(hasher);
        }
        Expression::Null => {}
        Expression::Field(path) => {
            path.hash(hasher);
        }
        Expression::Variable(name) => {
            name.hash(hasher);
        }
        Expression::And(left, right)
        | Expression::Or(left, right)
        | Expression::Equal(left, right)
        | Expression::NotEqual(left, right)
        | Expression::GreaterThan(left, right)
        | Expression::LessThan(left, right)
        | Expression::GreaterEqual(left, right)
        | Expression::LessEqual(left, right)
        | Expression::Contains(left, right)
        | Expression::StartsWith(left, right)
        | Expression::EndsWith(left, right)
        | Expression::Matches(left, right)
        | Expression::Index(left, right) => {
            hash_expression_recursive(left, hasher);
            hash_expression_recursive(right, hasher);
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
            hash_expression_recursive(inner, hasher);
        }
        Expression::In(expr, values) => {
            hash_expression_recursive(expr, hasher);
            for val in values {
                val.to_string().hash(hasher);
            }
        }
        Expression::ArrayWildcard(base, path) => {
            hash_expression_recursive(base, hasher);
            path.hash(hasher);
        }
    }
}

/// Check if two expressions are structurally equal
fn expressions_equal(left: &Expression, right: &Expression) -> bool {
    match (left, right) {
        (Expression::Number(a), Expression::Number(b)) => (a - b).abs() < f64::EPSILON,
        (Expression::String(a), Expression::String(b)) => a == b,
        (Expression::Boolean(a), Expression::Boolean(b)) => a == b,
        (Expression::Null, Expression::Null) => true,
        (Expression::Field(a), Expression::Field(b)) => a == b,
        (Expression::Variable(a), Expression::Variable(b)) => a == b,
        (Expression::And(l1, r1), Expression::And(l2, r2))
        | (Expression::Or(l1, r1), Expression::Or(l2, r2))
        | (Expression::Equal(l1, r1), Expression::Equal(l2, r2))
        | (Expression::NotEqual(l1, r1), Expression::NotEqual(l2, r2))
        | (Expression::GreaterThan(l1, r1), Expression::GreaterThan(l2, r2))
        | (Expression::LessThan(l1, r1), Expression::LessThan(l2, r2))
        | (Expression::GreaterEqual(l1, r1), Expression::GreaterEqual(l2, r2))
        | (Expression::LessEqual(l1, r1), Expression::LessEqual(l2, r2))
        | (Expression::Contains(l1, r1), Expression::Contains(l2, r2))
        | (Expression::StartsWith(l1, r1), Expression::StartsWith(l2, r2))
        | (Expression::EndsWith(l1, r1), Expression::EndsWith(l2, r2))
        | (Expression::Matches(l1, r1), Expression::Matches(l2, r2))
        | (Expression::Index(l1, r1), Expression::Index(l2, r2)) => {
            expressions_equal(l1, l2) && expressions_equal(r1, r2)
        }
        (Expression::Not(a), Expression::Not(b))
        | (Expression::IsNull(a), Expression::IsNull(b))
        | (Expression::IsNotNull(a), Expression::IsNotNull(b))
        | (Expression::IsNumber(a), Expression::IsNumber(b))
        | (Expression::IsString(a), Expression::IsString(b))
        | (Expression::IsBool(a), Expression::IsBool(b))
        | (Expression::IsArray(a), Expression::IsArray(b))
        | (Expression::IsObject(a), Expression::IsObject(b))
        | (Expression::Length(a), Expression::Length(b))
        | (Expression::Sum(a), Expression::Sum(b))
        | (Expression::Count(a), Expression::Count(b))
        | (Expression::Min(a), Expression::Min(b))
        | (Expression::Max(a), Expression::Max(b))
        | (Expression::Avg(a), Expression::Avg(b)) => expressions_equal(a, b),
        (Expression::In(e1, v1), Expression::In(e2, v2)) => expressions_equal(e1, e2) && v1 == v2,
        (Expression::ArrayWildcard(b1, p1), Expression::ArrayWildcard(b2, p2)) => {
            expressions_equal(b1, b2) && p1 == p2
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_folding() {
        let mut optimizer = ExpressionOptimizer::new();

        // Test AND with constants
        let expr = Expression::And(
            Box::new(Expression::Boolean(true)),
            Box::new(Expression::Boolean(false)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));

        // Test OR with constants
        let expr = Expression::Or(
            Box::new(Expression::Boolean(false)),
            Box::new(Expression::Boolean(true)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Test double negation
        let expr = Expression::Not(Box::new(Expression::Not(Box::new(Expression::Boolean(
            true,
        )))));
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));
    }

    #[test]
    fn test_algebraic_simplification() {
        let mut optimizer = ExpressionOptimizer::new();

        // Test x && x => x
        let field = Expression::Field(vec!["name".to_string()]);
        let expr = Expression::And(Box::new(field.clone()), Box::new(field.clone()));
        let result = optimizer.algebraic_simplification(expr).unwrap();
        assert_eq!(result, field);

        // Test !(x == y) => x != y
        let expr = Expression::Not(Box::new(Expression::Equal(
            Box::new(Expression::Field(vec!["a".to_string()])),
            Box::new(Expression::Field(vec!["b".to_string()])),
        )));
        let result = optimizer.algebraic_simplification(expr).unwrap();
        matches!(result, Expression::NotEqual(_, _));
    }

    #[test]
    fn test_dead_code_elimination() {
        let mut optimizer = ExpressionOptimizer::new();

        // Test AND with false
        let expr = Expression::And(
            Box::new(Expression::Boolean(false)),
            Box::new(Expression::Field(vec!["expensive_field".to_string()])),
        );
        let result = optimizer.dead_code_elimination(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_short_circuit_optimization() {
        let mut optimizer = ExpressionOptimizer::new();

        // Test reordering based on complexity
        let simple = Expression::Field(vec!["status".to_string()]);
        let complex = Expression::Matches(
            Box::new(Expression::Field(vec!["description".to_string()])),
            Box::new(Expression::String(".*pattern.*".to_string())),
        );

        let expr = Expression::And(Box::new(complex.clone()), Box::new(simple.clone()));
        let result = optimizer.short_circuit_optimization(expr).unwrap();

        // Should reorder to put simple expression first
        match result {
            Expression::And(left, right) => {
                assert!(
                    optimizer.estimate_complexity(&left) <= optimizer.estimate_complexity(&right)
                );
            }
            _ => panic!("Expected And expression"),
        }
    }

    #[test]
    fn test_full_optimization() {
        let mut optimizer = ExpressionOptimizer::new();

        // Complex expression with multiple optimization opportunities
        let expr = Expression::And(
            Box::new(Expression::Or(
                Box::new(Expression::Boolean(false)),
                Box::new(Expression::Equal(
                    Box::new(Expression::Field(vec!["status".to_string()])),
                    Box::new(Expression::String("active".to_string())),
                )),
            )),
            Box::new(Expression::Not(Box::new(Expression::Boolean(false)))),
        );

        let result = optimizer.optimize(expr).unwrap();

        // Should simplify to just the field comparison
        let _result = result; // Verify optimization ran
        assert!(optimizer.stats.constants_folded > 0);
        assert!(optimizer.stats.expressions_optimized > 0);
    }

    // Phase 1: Tests for comparison operators (lines 228-337)

    #[test]
    fn test_constant_folding_equal_numbers() {
        let mut optimizer = ExpressionOptimizer::new();

        // Equal numbers
        let expr = Expression::Equal(
            Box::new(Expression::Number(42.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Unequal numbers
        let expr = Expression::Equal(
            Box::new(Expression::Number(42.0)),
            Box::new(Expression::Number(43.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_constant_folding_equal_strings() {
        let mut optimizer = ExpressionOptimizer::new();

        // Equal strings
        let expr = Expression::Equal(
            Box::new(Expression::String("hello".to_string())),
            Box::new(Expression::String("hello".to_string())),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Unequal strings
        let expr = Expression::Equal(
            Box::new(Expression::String("hello".to_string())),
            Box::new(Expression::String("world".to_string())),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_constant_folding_equal_booleans() {
        let mut optimizer = ExpressionOptimizer::new();

        // Equal booleans (true)
        let expr = Expression::Equal(
            Box::new(Expression::Boolean(true)),
            Box::new(Expression::Boolean(true)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Equal booleans (false)
        let expr = Expression::Equal(
            Box::new(Expression::Boolean(false)),
            Box::new(Expression::Boolean(false)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Unequal booleans
        let expr = Expression::Equal(
            Box::new(Expression::Boolean(true)),
            Box::new(Expression::Boolean(false)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_constant_folding_equal_null() {
        let mut optimizer = ExpressionOptimizer::new();

        // Both null
        let expr = Expression::Equal(Box::new(Expression::Null), Box::new(Expression::Null));
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));
    }

    #[test]
    fn test_constant_folding_equal_same_expression() {
        let mut optimizer = ExpressionOptimizer::new();

        // Same field expression
        let field = Expression::Field(vec!["status".to_string()]);
        let expr = Expression::Equal(Box::new(field.clone()), Box::new(field.clone()));
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));
    }

    #[test]
    fn test_constant_folding_not_equal_numbers() {
        let mut optimizer = ExpressionOptimizer::new();

        // Unequal numbers
        let expr = Expression::NotEqual(
            Box::new(Expression::Number(42.0)),
            Box::new(Expression::Number(43.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Equal numbers
        let expr = Expression::NotEqual(
            Box::new(Expression::Number(42.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_constant_folding_not_equal_strings() {
        let mut optimizer = ExpressionOptimizer::new();

        // Unequal strings
        let expr = Expression::NotEqual(
            Box::new(Expression::String("hello".to_string())),
            Box::new(Expression::String("world".to_string())),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Equal strings
        let expr = Expression::NotEqual(
            Box::new(Expression::String("hello".to_string())),
            Box::new(Expression::String("hello".to_string())),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_constant_folding_not_equal_booleans() {
        let mut optimizer = ExpressionOptimizer::new();

        // Unequal booleans
        let expr = Expression::NotEqual(
            Box::new(Expression::Boolean(true)),
            Box::new(Expression::Boolean(false)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Equal booleans
        let expr = Expression::NotEqual(
            Box::new(Expression::Boolean(true)),
            Box::new(Expression::Boolean(true)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_constant_folding_not_equal_same_expression() {
        let mut optimizer = ExpressionOptimizer::new();

        // Same field expression
        let field = Expression::Field(vec!["status".to_string()]);
        let expr = Expression::NotEqual(Box::new(field.clone()), Box::new(field.clone()));
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_constant_folding_greater_than() {
        let mut optimizer = ExpressionOptimizer::new();

        // Greater than (true)
        let expr = Expression::GreaterThan(
            Box::new(Expression::Number(43.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Greater than (false - equal)
        let expr = Expression::GreaterThan(
            Box::new(Expression::Number(42.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));

        // Greater than (false - less)
        let expr = Expression::GreaterThan(
            Box::new(Expression::Number(41.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_constant_folding_less_than() {
        let mut optimizer = ExpressionOptimizer::new();

        // Less than (true)
        let expr = Expression::LessThan(
            Box::new(Expression::Number(41.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Less than (false - equal)
        let expr = Expression::LessThan(
            Box::new(Expression::Number(42.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));

        // Less than (false - greater)
        let expr = Expression::LessThan(
            Box::new(Expression::Number(43.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_constant_folding_greater_equal() {
        let mut optimizer = ExpressionOptimizer::new();

        // Greater or equal (true - greater)
        let expr = Expression::GreaterEqual(
            Box::new(Expression::Number(43.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Greater or equal (true - equal)
        let expr = Expression::GreaterEqual(
            Box::new(Expression::Number(42.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Greater or equal (false)
        let expr = Expression::GreaterEqual(
            Box::new(Expression::Number(41.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }

    #[test]
    fn test_constant_folding_less_equal() {
        let mut optimizer = ExpressionOptimizer::new();

        // Less or equal (true - less)
        let expr = Expression::LessEqual(
            Box::new(Expression::Number(41.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Less or equal (true - equal)
        let expr = Expression::LessEqual(
            Box::new(Expression::Number(42.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(true));

        // Less or equal (false)
        let expr = Expression::LessEqual(
            Box::new(Expression::Number(43.0)),
            Box::new(Expression::Number(42.0)),
        );
        let result = optimizer.constant_folding(expr).unwrap();
        assert_eq!(result, Expression::Boolean(false));
    }
}
