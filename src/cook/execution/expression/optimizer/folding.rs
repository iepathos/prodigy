//! Constant folding and comparison optimization functions

use crate::cook::execution::expression::ast::Expression;
use anyhow::Result;

use super::{expressions_equal, OptimizationStats};

/// Helper function to fold Equal comparisons
pub(super) fn fold_equal_comparison(
    stats: &mut OptimizationStats,
    left: Expression,
    right: Expression,
) -> Result<Expression> {
    match (&left, &right) {
        (Expression::Number(a), Expression::Number(b)) => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean((a - b).abs() < f64::EPSILON))
        }
        (Expression::String(a), Expression::String(b)) => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean(a == b))
        }
        (Expression::Boolean(a), Expression::Boolean(b)) => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean(a == b))
        }
        (Expression::Null, Expression::Null) => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean(true))
        }
        _ => {
            // Check if same expression
            if expressions_equal(&left, &right) {
                stats.algebraic_simplifications += 1;
                Ok(Expression::Boolean(true))
            } else {
                Ok(Expression::Equal(Box::new(left), Box::new(right)))
            }
        }
    }
}

/// Helper function to fold NotEqual comparisons
pub(super) fn fold_not_equal_comparison(
    stats: &mut OptimizationStats,
    left: Expression,
    right: Expression,
) -> Result<Expression> {
    match (&left, &right) {
        (Expression::Number(a), Expression::Number(b)) => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean((a - b).abs() >= f64::EPSILON))
        }
        (Expression::String(a), Expression::String(b)) => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean(a != b))
        }
        (Expression::Boolean(a), Expression::Boolean(b)) => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean(a != b))
        }
        _ => {
            // Check if same expression
            if expressions_equal(&left, &right) {
                stats.algebraic_simplifications += 1;
                Ok(Expression::Boolean(false))
            } else {
                Ok(Expression::NotEqual(Box::new(left), Box::new(right)))
            }
        }
    }
}

/// Numeric comparison operator types
#[derive(Debug, Clone, Copy)]
pub(super) enum NumericComparisonOp {
    GreaterThan,
    LessThan,
    GreaterEqual,
    LessEqual,
}

/// Helper function to fold numeric comparisons
pub(super) fn fold_numeric_comparison(
    stats: &mut OptimizationStats,
    op: NumericComparisonOp,
    left: Expression,
    right: Expression,
) -> Result<Expression> {
    match (&left, &right) {
        (Expression::Number(a), Expression::Number(b)) => {
            stats.constants_folded += 1;
            let result = match op {
                NumericComparisonOp::GreaterThan => a > b,
                NumericComparisonOp::LessThan => a < b,
                NumericComparisonOp::GreaterEqual => a >= b,
                NumericComparisonOp::LessEqual => a <= b,
            };
            Ok(Expression::Boolean(result))
        }
        _ => {
            let expr = match op {
                NumericComparisonOp::GreaterThan => {
                    Expression::GreaterThan(Box::new(left), Box::new(right))
                }
                NumericComparisonOp::LessThan => {
                    Expression::LessThan(Box::new(left), Box::new(right))
                }
                NumericComparisonOp::GreaterEqual => {
                    Expression::GreaterEqual(Box::new(left), Box::new(right))
                }
                NumericComparisonOp::LessEqual => {
                    Expression::LessEqual(Box::new(left), Box::new(right))
                }
            };
            Ok(expr)
        }
    }
}

/// Helper function to fold IsNull type check
pub(super) fn fold_is_null(stats: &mut OptimizationStats, inner: Expression) -> Result<Expression> {
    match inner {
        Expression::Null => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean(true))
        }
        Expression::Number(_) | Expression::String(_) | Expression::Boolean(_) => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean(false))
        }
        _ => Ok(Expression::IsNull(Box::new(inner))),
    }
}

/// Helper function to fold IsNotNull type check
pub(super) fn fold_is_not_null(stats: &mut OptimizationStats, inner: Expression) -> Result<Expression> {
    match inner {
        Expression::Null => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean(false))
        }
        Expression::Number(_) | Expression::String(_) | Expression::Boolean(_) => {
            stats.constants_folded += 1;
            Ok(Expression::Boolean(true))
        }
        _ => Ok(Expression::IsNotNull(Box::new(inner))),
    }
}
