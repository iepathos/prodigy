//! Constant folding and comparison optimization functions

use crate::cook::execution::expression::ast::Expression;
use anyhow::Result;

use super::utils::expressions_equal;
use super::OptimizationStats;

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
pub(super) fn fold_is_not_null(
    stats: &mut OptimizationStats,
    inner: Expression,
) -> Result<Expression> {
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

/// Helper function to fold logical operators (And, Or, Not)
pub(super) fn fold_logical_operators(
    optimizer: &mut super::ExpressionOptimizer,
    expr: Expression,
) -> Result<Expression> {
    match expr {
        Expression::And(left, right) => {
            let left = optimizer.constant_folding(*left)?;
            let right = optimizer.constant_folding(*right)?;

            match (&left, &right) {
                (Expression::Boolean(false), _) | (_, Expression::Boolean(false)) => {
                    optimizer.stats.constants_folded += 1;
                    Ok(Expression::Boolean(false))
                }
                (Expression::Boolean(true), other) | (other, Expression::Boolean(true)) => {
                    optimizer.stats.constants_folded += 1;
                    Ok(other.clone())
                }
                _ => Ok(Expression::And(Box::new(left), Box::new(right))),
            }
        }
        Expression::Or(left, right) => {
            let left = optimizer.constant_folding(*left)?;
            let right = optimizer.constant_folding(*right)?;

            match (&left, &right) {
                (Expression::Boolean(true), _) | (_, Expression::Boolean(true)) => {
                    optimizer.stats.constants_folded += 1;
                    Ok(Expression::Boolean(true))
                }
                (Expression::Boolean(false), other) | (other, Expression::Boolean(false)) => {
                    optimizer.stats.constants_folded += 1;
                    Ok(other.clone())
                }
                _ => Ok(Expression::Or(Box::new(left), Box::new(right))),
            }
        }
        Expression::Not(inner) => {
            let inner = optimizer.constant_folding(*inner)?;
            match inner {
                Expression::Boolean(b) => {
                    optimizer.stats.constants_folded += 1;
                    Ok(Expression::Boolean(!b))
                }
                Expression::Not(double_inner) => {
                    optimizer.stats.algebraic_simplifications += 1;
                    Ok(*double_inner) // Double negation
                }
                _ => Ok(Expression::Not(Box::new(inner))),
            }
        }
        _ => Ok(expr),
    }
}

/// Helper function to fold string operators (Contains, StartsWith, EndsWith, Matches)
pub(super) fn fold_string_operators(
    optimizer: &mut super::ExpressionOptimizer,
    expr: Expression,
) -> Result<Expression> {
    match expr {
        Expression::Contains(left, right) => Ok(Expression::Contains(
            Box::new(optimizer.constant_folding(*left)?),
            Box::new(optimizer.constant_folding(*right)?),
        )),
        Expression::StartsWith(left, right) => Ok(Expression::StartsWith(
            Box::new(optimizer.constant_folding(*left)?),
            Box::new(optimizer.constant_folding(*right)?),
        )),
        Expression::EndsWith(left, right) => Ok(Expression::EndsWith(
            Box::new(optimizer.constant_folding(*left)?),
            Box::new(optimizer.constant_folding(*right)?),
        )),
        Expression::Matches(left, right) => Ok(Expression::Matches(
            Box::new(optimizer.constant_folding(*left)?),
            Box::new(optimizer.constant_folding(*right)?),
        )),
        _ => Ok(expr),
    }
}

/// Helper function to fold type check operators
pub(super) fn fold_type_checks(
    optimizer: &mut super::ExpressionOptimizer,
    expr: Expression,
) -> Result<Expression> {
    match expr {
        Expression::IsNull(inner) => {
            let inner = optimizer.constant_folding(*inner)?;
            fold_is_null(&mut optimizer.stats, inner)
        }
        Expression::IsNotNull(inner) => {
            let inner = optimizer.constant_folding(*inner)?;
            fold_is_not_null(&mut optimizer.stats, inner)
        }
        Expression::IsNumber(inner) => Ok(Expression::IsNumber(Box::new(
            optimizer.constant_folding(*inner)?,
        ))),
        Expression::IsString(inner) => Ok(Expression::IsString(Box::new(
            optimizer.constant_folding(*inner)?,
        ))),
        Expression::IsBool(inner) => {
            Ok(Expression::IsBool(Box::new(optimizer.constant_folding(*inner)?)))
        }
        Expression::IsArray(inner) => Ok(Expression::IsArray(Box::new(
            optimizer.constant_folding(*inner)?,
        ))),
        Expression::IsObject(inner) => Ok(Expression::IsObject(Box::new(
            optimizer.constant_folding(*inner)?,
        ))),
        _ => Ok(expr),
    }
}

/// Helper function to fold aggregate functions (Length, Sum, Count, Min, Max, Avg)
pub(super) fn fold_aggregate_functions(
    optimizer: &mut super::ExpressionOptimizer,
    expr: Expression,
) -> Result<Expression> {
    match expr {
        Expression::Length(inner) => {
            Ok(Expression::Length(Box::new(optimizer.constant_folding(*inner)?)))
        }
        Expression::Sum(inner) => Ok(Expression::Sum(Box::new(optimizer.constant_folding(*inner)?))),
        Expression::Count(inner) => {
            Ok(Expression::Count(Box::new(optimizer.constant_folding(*inner)?)))
        }
        Expression::Min(inner) => Ok(Expression::Min(Box::new(optimizer.constant_folding(*inner)?))),
        Expression::Max(inner) => Ok(Expression::Max(Box::new(optimizer.constant_folding(*inner)?))),
        Expression::Avg(inner) => Ok(Expression::Avg(Box::new(optimizer.constant_folding(*inner)?))),
        _ => Ok(expr),
    }
}
