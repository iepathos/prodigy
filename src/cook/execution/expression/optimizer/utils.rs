//! Expression hashing and equality utilities

use crate::cook::execution::expression::ast::Expression;
use std::hash::{Hash, Hasher};

/// Hash an expression for CSE
pub(super) fn hash_expression(expr: &Expression) -> u64 {
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();
    hash_expression_recursive(expr, &mut hasher);
    hasher.finish()
}

/// Recursively hash an expression
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
pub(super) fn expressions_equal(left: &Expression, right: &Expression) -> bool {
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
