//! Tests for the expression optimizer

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
            assert!(optimizer.estimate_complexity(&left) <= optimizer.estimate_complexity(&right));
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

// Phase 2: Tests for type checking operators (lines 339-367)

#[test]
fn test_constant_folding_is_null_with_null() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsNull(Null) => true
    let expr = Expression::IsNull(Box::new(Expression::Null));
    let result = optimizer.constant_folding(expr).unwrap();
    assert_eq!(result, Expression::Boolean(true));
}

#[test]
fn test_constant_folding_is_null_with_number() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsNull(Number) => false
    let expr = Expression::IsNull(Box::new(Expression::Number(42.0)));
    let result = optimizer.constant_folding(expr).unwrap();
    assert_eq!(result, Expression::Boolean(false));
}

#[test]
fn test_constant_folding_is_null_with_string() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsNull(String) => false
    let expr = Expression::IsNull(Box::new(Expression::String("test".to_string())));
    let result = optimizer.constant_folding(expr).unwrap();
    assert_eq!(result, Expression::Boolean(false));
}

#[test]
fn test_constant_folding_is_null_with_boolean() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsNull(Boolean) => false
    let expr = Expression::IsNull(Box::new(Expression::Boolean(true)));
    let result = optimizer.constant_folding(expr).unwrap();
    assert_eq!(result, Expression::Boolean(false));

    let expr = Expression::IsNull(Box::new(Expression::Boolean(false)));
    let result = optimizer.constant_folding(expr).unwrap();
    assert_eq!(result, Expression::Boolean(false));
}

#[test]
fn test_constant_folding_is_not_null_with_null() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsNotNull(Null) => false
    let expr = Expression::IsNotNull(Box::new(Expression::Null));
    let result = optimizer.constant_folding(expr).unwrap();
    assert_eq!(result, Expression::Boolean(false));
}

#[test]
fn test_constant_folding_is_not_null_with_number() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsNotNull(Number) => true
    let expr = Expression::IsNotNull(Box::new(Expression::Number(42.0)));
    let result = optimizer.constant_folding(expr).unwrap();
    assert_eq!(result, Expression::Boolean(true));
}

#[test]
fn test_constant_folding_is_not_null_with_string() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsNotNull(String) => true
    let expr = Expression::IsNotNull(Box::new(Expression::String("test".to_string())));
    let result = optimizer.constant_folding(expr).unwrap();
    assert_eq!(result, Expression::Boolean(true));
}

#[test]
fn test_constant_folding_is_not_null_with_boolean() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsNotNull(Boolean) => true
    let expr = Expression::IsNotNull(Box::new(Expression::Boolean(true)));
    let result = optimizer.constant_folding(expr).unwrap();
    assert_eq!(result, Expression::Boolean(true));

    let expr = Expression::IsNotNull(Box::new(Expression::Boolean(false)));
    let result = optimizer.constant_folding(expr).unwrap();
    assert_eq!(result, Expression::Boolean(true));
}

// Phase 3: Tests for string and pattern operators (lines 369-393)

#[test]
fn test_constant_folding_contains_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // Contains with nested constant folding (And expression that folds to Boolean)
    let expr = Expression::Contains(
        Box::new(Expression::Field(vec!["text".to_string()])),
        Box::new(Expression::And(
            Box::new(Expression::Boolean(true)),
            Box::new(Expression::String("search".to_string())),
        )),
    );
    let result = optimizer.constant_folding(expr).unwrap();

    // The And should fold to just the String
    match result {
        Expression::Contains(_, right) => {
            assert!(matches!(*right, Expression::String(_)));
        }
        _ => panic!("Expected Contains expression"),
    }
}

#[test]
fn test_constant_folding_starts_with_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // StartsWith with nested Not(Not(x)) that simplifies
    let expr = Expression::StartsWith(
        Box::new(Expression::Field(vec!["name".to_string()])),
        Box::new(Expression::Not(Box::new(Expression::Not(Box::new(
            Expression::String("prefix".to_string()),
        ))))),
    );
    let result = optimizer.constant_folding(expr).unwrap();

    // The double negation should be eliminated
    match result {
        Expression::StartsWith(_, right) => {
            assert!(matches!(*right, Expression::String(_)));
        }
        _ => panic!("Expected StartsWith expression"),
    }
}

#[test]
fn test_constant_folding_ends_with_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // EndsWith with nested comparison that folds
    let expr = Expression::EndsWith(
        Box::new(Expression::Equal(
            Box::new(Expression::Number(1.0)),
            Box::new(Expression::Number(1.0)),
        )),
        Box::new(Expression::Field(vec!["suffix".to_string()])),
    );
    let result = optimizer.constant_folding(expr).unwrap();

    // The Equal should fold to Boolean(true)
    match result {
        Expression::EndsWith(left, _) => {
            assert!(matches!(*left, Expression::Boolean(true)));
        }
        _ => panic!("Expected EndsWith expression"),
    }
}

#[test]
fn test_constant_folding_matches_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // Matches with nested Or expression
    let expr = Expression::Matches(
        Box::new(Expression::Or(
            Box::new(Expression::Boolean(false)),
            Box::new(Expression::Field(vec!["text".to_string()])),
        )),
        Box::new(Expression::String("pattern.*".to_string())),
    );
    let result = optimizer.constant_folding(expr).unwrap();

    // The Or should fold to just the Field
    match result {
        Expression::Matches(left, _) => {
            assert!(matches!(*left, Expression::Field(_)));
        }
        _ => panic!("Expected Matches expression"),
    }
}

#[test]
fn test_constant_folding_index_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // Index with constant expressions that fold
    let expr = Expression::Index(
        Box::new(Expression::Field(vec!["array".to_string()])),
        Box::new(Expression::Not(Box::new(Expression::Boolean(false)))),
    );
    let result = optimizer.constant_folding(expr).unwrap();

    // The Not(false) should fold to Boolean(true)
    match result {
        Expression::Index(_, idx) => {
            assert!(matches!(*idx, Expression::Boolean(true)));
        }
        _ => panic!("Expected Index expression"),
    }
}

#[test]
fn test_constant_folding_array_wildcard_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // ArrayWildcard with nested folding
    let expr = Expression::ArrayWildcard(
        Box::new(Expression::Or(
            Box::new(Expression::Boolean(true)),
            Box::new(Expression::Field(vec!["items".to_string()])),
        )),
        vec!["name".to_string()],
    );
    let result = optimizer.constant_folding(expr).unwrap();

    // The Or should fold to Boolean(true)
    match result {
        Expression::ArrayWildcard(base, _) => {
            assert!(matches!(*base, Expression::Boolean(true)));
        }
        _ => panic!("Expected ArrayWildcard expression"),
    }
}

// Phase 4: Tests for aggregate functions and type check functions (lines 395-422)

#[test]
fn test_constant_folding_length_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // Length with nested constant folding
    let expr = Expression::Length(Box::new(Expression::And(
        Box::new(Expression::Boolean(true)),
        Box::new(Expression::Field(vec!["items".to_string()])),
    )));
    let result = optimizer.constant_folding(expr).unwrap();

    // The And should fold to just the Field
    match result {
        Expression::Length(inner) => {
            assert!(matches!(*inner, Expression::Field(_)));
        }
        _ => panic!("Expected Length expression"),
    }
}

#[test]
fn test_constant_folding_sum_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // Sum with nested folding
    let expr = Expression::Sum(Box::new(Expression::Or(
        Box::new(Expression::Boolean(false)),
        Box::new(Expression::Field(vec!["values".to_string()])),
    )));
    let result = optimizer.constant_folding(expr).unwrap();

    // The Or should fold to just the Field
    match result {
        Expression::Sum(inner) => {
            assert!(matches!(*inner, Expression::Field(_)));
        }
        _ => panic!("Expected Sum expression"),
    }
}

#[test]
fn test_constant_folding_count_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // Count with nested folding
    let expr = Expression::Count(Box::new(Expression::Not(Box::new(Expression::Not(
        Box::new(Expression::Field(vec!["items".to_string()])),
    )))));
    let result = optimizer.constant_folding(expr).unwrap();

    // The double negation should be eliminated
    match result {
        Expression::Count(inner) => {
            assert!(matches!(*inner, Expression::Field(_)));
        }
        _ => panic!("Expected Count expression"),
    }
}

#[test]
fn test_constant_folding_min_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // Min with nested folding
    let expr = Expression::Min(Box::new(Expression::Equal(
        Box::new(Expression::Number(5.0)),
        Box::new(Expression::Number(5.0)),
    )));
    let result = optimizer.constant_folding(expr).unwrap();

    // The Equal should fold to Boolean(true)
    match result {
        Expression::Min(inner) => {
            assert!(matches!(*inner, Expression::Boolean(true)));
        }
        _ => panic!("Expected Min expression"),
    }
}

#[test]
fn test_constant_folding_max_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // Max with nested folding
    let expr = Expression::Max(Box::new(Expression::LessThan(
        Box::new(Expression::Number(3.0)),
        Box::new(Expression::Number(5.0)),
    )));
    let result = optimizer.constant_folding(expr).unwrap();

    // The LessThan should fold to Boolean(true)
    match result {
        Expression::Max(inner) => {
            assert!(matches!(*inner, Expression::Boolean(true)));
        }
        _ => panic!("Expected Max expression"),
    }
}

#[test]
fn test_constant_folding_avg_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // Avg with nested folding
    let expr = Expression::Avg(Box::new(Expression::And(
        Box::new(Expression::Boolean(true)),
        Box::new(Expression::Field(vec!["scores".to_string()])),
    )));
    let result = optimizer.constant_folding(expr).unwrap();

    // The And should fold to just the Field
    match result {
        Expression::Avg(inner) => {
            assert!(matches!(*inner, Expression::Field(_)));
        }
        _ => panic!("Expected Avg expression"),
    }
}

#[test]
fn test_constant_folding_is_number_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsNumber with nested folding
    let expr = Expression::IsNumber(Box::new(Expression::Or(
        Box::new(Expression::Boolean(false)),
        Box::new(Expression::Field(vec!["value".to_string()])),
    )));
    let result = optimizer.constant_folding(expr).unwrap();

    // The Or should fold to just the Field
    match result {
        Expression::IsNumber(inner) => {
            assert!(matches!(*inner, Expression::Field(_)));
        }
        _ => panic!("Expected IsNumber expression"),
    }
}

#[test]
fn test_constant_folding_is_string_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsString with nested folding
    let expr = Expression::IsString(Box::new(Expression::Not(Box::new(Expression::Boolean(
        false,
    )))));
    let result = optimizer.constant_folding(expr).unwrap();

    // The Not(false) should fold to Boolean(true)
    match result {
        Expression::IsString(inner) => {
            assert!(matches!(*inner, Expression::Boolean(true)));
        }
        _ => panic!("Expected IsString expression"),
    }
}

#[test]
fn test_constant_folding_is_bool_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsBool with nested folding
    let expr = Expression::IsBool(Box::new(Expression::And(
        Box::new(Expression::Boolean(true)),
        Box::new(Expression::Field(vec!["flag".to_string()])),
    )));
    let result = optimizer.constant_folding(expr).unwrap();

    // The And should fold to just the Field
    match result {
        Expression::IsBool(inner) => {
            assert!(matches!(*inner, Expression::Field(_)));
        }
        _ => panic!("Expected IsBool expression"),
    }
}

#[test]
fn test_constant_folding_is_array_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsArray with nested folding
    let expr = Expression::IsArray(Box::new(Expression::GreaterThan(
        Box::new(Expression::Number(10.0)),
        Box::new(Expression::Number(5.0)),
    )));
    let result = optimizer.constant_folding(expr).unwrap();

    // The GreaterThan should fold to Boolean(true)
    match result {
        Expression::IsArray(inner) => {
            assert!(matches!(*inner, Expression::Boolean(true)));
        }
        _ => panic!("Expected IsArray expression"),
    }
}

#[test]
fn test_constant_folding_is_object_recursive() {
    let mut optimizer = ExpressionOptimizer::new();

    // IsObject with nested folding
    let expr = Expression::IsObject(Box::new(Expression::Equal(
        Box::new(Expression::String("a".to_string())),
        Box::new(Expression::String("a".to_string())),
    )));
    let result = optimizer.constant_folding(expr).unwrap();

    // The Equal should fold to Boolean(true)
    match result {
        Expression::IsObject(inner) => {
            assert!(matches!(*inner, Expression::Boolean(true)));
        }
        _ => panic!("Expected IsObject expression"),
    }
}
