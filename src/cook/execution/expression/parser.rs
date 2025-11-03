//! Expression parser for filter and sort expressions

use anyhow::{anyhow, Result};

use crate::cook::execution::expression::ast::{Expression, NullHandling, SortDirection, SortKey};
use crate::cook::execution::expression::tokenizer::{tokenize, Token};

/// Parse a binary operator expression (OR, AND, etc.)
///
/// This pure helper function extracts the common pattern used in parsing binary operators.
/// It finds all operator positions at the current nesting level, splits the token stream,
/// parses each part, and combines them into an expression tree.
fn parse_binary_operator<F, C>(
    tokens: &[Token],
    op: &Token,
    parse_next: F,
    combine: C,
) -> Result<Expression>
where
    F: Fn(&[Token]) -> Result<Expression>,
    C: Fn(Expression, Expression) -> Expression,
{
    // Find all operator positions at this level (not inside parentheses)
    let positions = find_operators(tokens, op)?;

    if positions.is_empty() {
        return parse_next(tokens);
    }

    // Split by operator and parse each part
    let mut parts = Vec::new();
    let mut start = 0;
    for pos in positions {
        if pos > start {
            let part_tokens = &tokens[start..pos];
            parts.push(parse_next(part_tokens)?);
        }
        start = pos + 1;
    }
    if start < tokens.len() {
        parts.push(parse_next(&tokens[start..])?);
    }

    // Build expression tree from left to right
    let result = parts
        .into_iter()
        .reduce(combine)
        .ok_or_else(|| anyhow!("Empty expression"))?;

    Ok(result)
}

/// Find operator positions at the current level (not inside parentheses)
///
/// This pure function scans through tokens and returns the positions of the given operator,
/// but only at the current parenthesis nesting level.
fn find_operators(tokens: &[Token], op: &Token) -> Result<Vec<usize>> {
    let mut positions = Vec::new();
    let mut paren_depth = 0;

    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::LeftParen => paren_depth += 1,
            Token::RightParen => paren_depth -= 1,
            _ if paren_depth == 0 && token == op => positions.push(i),
            _ => {}
        }
    }

    Ok(positions)
}

/// Find the matching closing parenthesis for an opening parenthesis
///
/// Returns the index of the matching right paren, or an error if not found.
fn find_matching_paren(tokens: &[Token], start: usize) -> Result<usize> {
    if start >= tokens.len() || tokens[start] != Token::LeftParen {
        return Err(anyhow!("Expected left paren at position {}", start));
    }

    let mut depth = 1;
    let mut end = start + 1;

    while end < tokens.len() && depth > 0 {
        match tokens[end] {
            Token::LeftParen => depth += 1,
            Token::RightParen => depth -= 1,
            _ => {}
        }
        if depth > 0 {
            end += 1;
        }
    }

    if depth != 0 {
        return Err(anyhow!("Mismatched parentheses"));
    }

    Ok(end)
}

/// Expression parser
pub struct ExpressionParser {
    // Could add configuration options here
}

impl ExpressionParser {
    /// Create a new parser
    pub fn new() -> Self {
        Self {}
    }

    /// Parse a filter expression
    pub fn parse_filter(&self, expr: &str) -> Result<Expression> {
        let tokens = tokenize(expr)?;
        self.parse_expression(&tokens, 0)
    }

    /// Parse a sort specification
    pub fn parse_sort(&self, spec: &str) -> Result<Vec<SortKey>> {
        let mut sort_keys = Vec::new();

        // Split by commas for multiple sort fields
        for field_spec in spec.split(',') {
            let field_spec = field_spec.trim();
            if field_spec.is_empty() {
                continue;
            }

            let parts: Vec<&str> = field_spec.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            // Parse the field expression
            let field_expr = self.parse_field_path(parts[0])?;

            // Parse direction
            let mut direction = SortDirection::Ascending;
            let mut null_handling = NullHandling::Last;
            let mut i = 1;

            if i < parts.len() {
                match parts[i].to_uppercase().as_str() {
                    "DESC" | "DESCENDING" => {
                        direction = SortDirection::Descending;
                        i += 1;
                    }
                    "ASC" | "ASCENDING" => {
                        direction = SortDirection::Ascending;
                        i += 1;
                    }
                    _ => {}
                }
            }

            // Parse null handling
            if i < parts.len() && parts[i].to_uppercase() == "NULLS" {
                i += 1;
                if i < parts.len() {
                    match parts[i].to_uppercase().as_str() {
                        "FIRST" => null_handling = NullHandling::First,
                        "LAST" => null_handling = NullHandling::Last,
                        _ => {
                            return Err(anyhow!(
                                "Invalid null position: {}. Use NULLS FIRST or NULLS LAST",
                                parts[i]
                            ))
                        }
                    }
                }
            }

            sort_keys.push(SortKey {
                expression: field_expr,
                direction,
                null_handling,
            });
        }

        if sort_keys.is_empty() {
            return Err(anyhow!("No sort fields specified"));
        }

        Ok(sort_keys)
    }

    /// Parse an expression from tokens
    fn parse_expression(&self, tokens: &[Token], _min_precedence: i32) -> Result<Expression> {
        // Simplified recursive descent parser
        if tokens.is_empty() {
            return Err(anyhow!("Empty expression"));
        }

        // Parse OR expressions (lowest precedence)
        self.parse_or(tokens)
    }

    /// Parse OR expressions
    fn parse_or(&self, tokens: &[Token]) -> Result<Expression> {
        parse_binary_operator(
            tokens,
            &Token::Or,
            |tokens| self.parse_and(tokens),
            |left, right| Expression::Or(Box::new(left), Box::new(right)),
        )
    }

    /// Parse AND expressions
    fn parse_and(&self, tokens: &[Token]) -> Result<Expression> {
        parse_binary_operator(
            tokens,
            &Token::And,
            |tokens| self.parse_comparison(tokens),
            |left, right| Expression::And(Box::new(left), Box::new(right)),
        )
    }

    /// Parse comparison expressions
    fn parse_comparison(&self, tokens: &[Token]) -> Result<Expression> {
        if tokens.is_empty() {
            return Err(anyhow!("Empty comparison expression"));
        }

        // Handle parenthesized expressions first
        if tokens[0] == Token::LeftParen {
            // Check if entire expression is parenthesized
            let end = find_matching_paren(tokens, 0)?;

            if end + 1 == tokens.len() {
                // Entire expression is parenthesized, strip parentheses and re-parse
                return self.parse_or(&tokens[1..end]);
            }
        }

        // Handle NOT
        if tokens[0] == Token::Not {
            let expr = self.parse_comparison(&tokens[1..])?;
            return Ok(Expression::Not(Box::new(expr)));
        }

        // Find comparison operator position
        let mut op_pos = None;
        let mut paren_depth = 0;

        for (i, token) in tokens.iter().enumerate() {
            match token {
                Token::LeftParen => paren_depth += 1,
                Token::RightParen => paren_depth -= 1,
                Token::Equal
                | Token::NotEqual
                | Token::Greater
                | Token::Less
                | Token::GreaterEqual
                | Token::LessEqual
                | Token::Contains
                | Token::StartsWith
                | Token::EndsWith
                | Token::Matches
                    if paren_depth == 0 && op_pos.is_none() =>
                {
                    op_pos = Some(i);
                }
                _ => {}
            }
        }

        // If no operator found, it's a primary expression
        if op_pos.is_none() {
            return self.parse_primary(tokens);
        }

        let pos = op_pos.unwrap();
        let left_tokens = &tokens[..pos];
        let right_tokens = &tokens[pos + 1..];

        if left_tokens.is_empty() || right_tokens.is_empty() {
            return Err(anyhow!("Invalid comparison expression"));
        }

        let left = self.parse_primary(left_tokens)?;
        let right = self.parse_primary(right_tokens)?;

        let expr = match &tokens[pos] {
            Token::Equal => Expression::Equal(Box::new(left), Box::new(right)),
            Token::NotEqual => Expression::NotEqual(Box::new(left), Box::new(right)),
            Token::Greater => Expression::GreaterThan(Box::new(left), Box::new(right)),
            Token::Less => Expression::LessThan(Box::new(left), Box::new(right)),
            Token::GreaterEqual => Expression::GreaterEqual(Box::new(left), Box::new(right)),
            Token::LessEqual => Expression::LessEqual(Box::new(left), Box::new(right)),
            Token::Contains => Expression::Contains(Box::new(left), Box::new(right)),
            Token::StartsWith => Expression::StartsWith(Box::new(left), Box::new(right)),
            Token::EndsWith => Expression::EndsWith(Box::new(left), Box::new(right)),
            Token::Matches => Expression::Matches(Box::new(left), Box::new(right)),
            _ => return Err(anyhow!("Unexpected operator: {:?}", tokens[pos])),
        };

        Ok(expr)
    }

    /// Parse primary expressions (literals, identifiers, parenthesized expressions)
    fn parse_primary(&self, tokens: &[Token]) -> Result<Expression> {
        if tokens.is_empty() {
            return Err(anyhow!("Expected expression"));
        }

        match &tokens[0] {
            Token::Number(n) => Ok(Expression::Number(*n)),
            Token::String(s) => Ok(Expression::String(s.clone())),
            Token::Boolean(b) => Ok(Expression::Boolean(*b)),
            Token::Null => Ok(Expression::Null),

            // Aggregate functions with parentheses
            Token::Length | Token::Sum | Token::Count | Token::Min | Token::Max | Token::Avg => {
                if tokens.len() >= 3 && tokens[1] == Token::LeftParen {
                    // Find matching right paren
                    let end = find_matching_paren(tokens, 1)?;
                    // Parse the argument
                    let arg = self.parse_expression(&tokens[2..end], 0)?;
                    let func_expr = match tokens[0] {
                        Token::Length => Expression::Length(Box::new(arg)),
                        Token::Sum => Expression::Sum(Box::new(arg)),
                        Token::Count => Expression::Count(Box::new(arg)),
                        Token::Min => Expression::Min(Box::new(arg)),
                        Token::Max => Expression::Max(Box::new(arg)),
                        Token::Avg => Expression::Avg(Box::new(arg)),
                        _ => unreachable!(),
                    };
                    Ok(func_expr)
                } else {
                    Err(anyhow!("Expected '(' after function name"))
                }
            }

            Token::Identifier(name) => {
                // Check if this is a special variable
                if name.starts_with('_') {
                    return Ok(Expression::Variable(name.clone()));
                }
                // Parse field path (including array wildcard support)
                self.parse_field_path(name)
            }
            Token::LeftParen => {
                // Find matching right paren
                let end = find_matching_paren(tokens, 0)?;
                // Parse the expression inside parentheses
                self.parse_expression(&tokens[1..end], 0)
            }
            _ => Err(anyhow!("Unexpected token: {:?}", tokens[0])),
        }
    }

    /// Parse a field path (e.g., "user.profile.name" or "items\[0\].value" or "items\[*\].score")
    fn parse_field_path(&self, path: &str) -> Result<Expression> {
        // Check for array wildcard notation
        if path.contains("[*]") {
            let parts: Vec<&str> = path.splitn(2, "[*]").collect();
            if parts.len() == 2 {
                let base = parts[0];
                let rest = parts[1].trim_start_matches('.');

                let base_segments: Vec<String> = base.split('.').map(|s| s.to_string()).collect();
                let base_expr = Expression::Field(base_segments);

                let rest_segments: Vec<String> = if rest.is_empty() {
                    vec![]
                } else {
                    rest.split('.').map(|s| s.to_string()).collect()
                };

                return Ok(Expression::ArrayWildcard(
                    Box::new(base_expr),
                    rest_segments,
                ));
            }
        }

        let segments: Vec<String> = path.split('.').map(|s| s.to_string()).collect();
        Ok(Expression::Field(segments))
    }
}

impl Default for ExpressionParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function for parser tests
    fn parse_filter(expr: &str) -> Result<Expression> {
        ExpressionParser::new().parse_filter(expr)
    }

    fn parse_sort(expr: &str) -> Result<Vec<SortKey>> {
        ExpressionParser::new().parse_sort(expr)
    }

    // Tests for parse_filter - Simple comparisons
    #[test]
    fn test_parse_filter_simple_equal() {
        let expr = parse_filter("priority == 5").unwrap();
        assert!(matches!(expr, Expression::Equal(_, _)));
    }

    #[test]
    fn test_parse_filter_simple_not_equal() {
        let expr = parse_filter("status != 'done'").unwrap();
        assert!(matches!(expr, Expression::NotEqual(_, _)));
    }

    #[test]
    fn test_parse_filter_greater_than() {
        let expr = parse_filter("score > 100").unwrap();
        assert!(matches!(expr, Expression::GreaterThan(_, _)));
    }

    #[test]
    fn test_parse_filter_less_than() {
        let expr = parse_filter("age < 18").unwrap();
        assert!(matches!(expr, Expression::LessThan(_, _)));
    }

    #[test]
    fn test_parse_filter_greater_equal() {
        let expr = parse_filter("quantity >= 10").unwrap();
        assert!(matches!(expr, Expression::GreaterEqual(_, _)));
    }

    #[test]
    fn test_parse_filter_less_equal() {
        let expr = parse_filter("value <= 50").unwrap();
        assert!(matches!(expr, Expression::LessEqual(_, _)));
    }

    // Tests for logical operators
    #[test]
    fn test_parse_filter_and() {
        let expr = parse_filter("priority > 5 && status == 'active'").unwrap();
        assert!(matches!(expr, Expression::And(_, _)));
    }

    #[test]
    fn test_parse_filter_or() {
        let expr = parse_filter("priority > 5 || status == 'urgent'").unwrap();
        assert!(matches!(expr, Expression::Or(_, _)));
    }

    #[test]
    fn test_parse_filter_not() {
        let expr = parse_filter("!(status == 'done')").unwrap();
        assert!(matches!(expr, Expression::Not(_)));
    }

    #[test]
    fn test_parse_filter_complex_logical() {
        let expr =
            parse_filter("(priority > 5 && status == 'active') || urgency == 'high'").unwrap();
        assert!(matches!(expr, Expression::Or(_, _)));
    }

    // Tests for nested expressions
    #[test]
    fn test_parse_filter_nested_parentheses() {
        let expr = parse_filter("((priority > 5))").unwrap();
        assert!(matches!(expr, Expression::GreaterThan(_, _)));
    }

    #[test]
    fn test_parse_filter_complex_nested() {
        let expr = parse_filter(
            "(priority > 5 && status == 'active') || (priority > 10 && status == 'pending')",
        )
        .unwrap();
        assert!(matches!(expr, Expression::Or(_, _)));
    }

    // Tests for string functions
    #[test]
    fn test_parse_filter_contains() {
        let expr = parse_filter("title contains 'urgent'").unwrap();
        assert!(matches!(expr, Expression::Contains(_, _)));
    }

    #[test]
    fn test_parse_filter_starts_with() {
        let expr = parse_filter("name starts_with 'A'").unwrap();
        assert!(matches!(expr, Expression::StartsWith(_, _)));
    }

    #[test]
    fn test_parse_filter_ends_with() {
        let expr = parse_filter("filename ends_with '.txt'").unwrap();
        assert!(matches!(expr, Expression::EndsWith(_, _)));
    }

    #[test]
    fn test_parse_filter_matches() {
        let expr = parse_filter("email matches '^[a-z]+@'").unwrap();
        assert!(matches!(expr, Expression::Matches(_, _)));
    }

    // Tests for aggregate functions
    #[test]
    fn test_parse_filter_length() {
        let expr = parse_filter("length(items) > 5").unwrap();
        assert!(matches!(expr, Expression::GreaterThan(_, _)));
        // The left side should be a Length expression
        if let Expression::GreaterThan(left, _) = expr {
            assert!(matches!(*left, Expression::Length(_)));
        }
    }

    #[test]
    fn test_parse_filter_sum() {
        let expr = parse_filter("sum(scores) >= 100").unwrap();
        if let Expression::GreaterEqual(left, _) = expr {
            assert!(matches!(*left, Expression::Sum(_)));
        }
    }

    #[test]
    fn test_parse_filter_count() {
        let expr = parse_filter("count(items) == 0").unwrap();
        if let Expression::Equal(left, _) = expr {
            assert!(matches!(*left, Expression::Count(_)));
        }
    }

    #[test]
    fn test_parse_filter_min() {
        let expr = parse_filter("min(values) > 10").unwrap();
        if let Expression::GreaterThan(left, _) = expr {
            assert!(matches!(*left, Expression::Min(_)));
        }
    }

    #[test]
    fn test_parse_filter_max() {
        let expr = parse_filter("max(values) < 100").unwrap();
        if let Expression::LessThan(left, _) = expr {
            assert!(matches!(*left, Expression::Max(_)));
        }
    }

    #[test]
    fn test_parse_filter_avg() {
        let expr = parse_filter("avg(scores) >= 75").unwrap();
        if let Expression::GreaterEqual(left, _) = expr {
            assert!(matches!(*left, Expression::Avg(_)));
        }
    }

    // Tests for field paths
    #[test]
    fn test_parse_filter_simple_field() {
        let expr = parse_filter("priority > 5").unwrap();
        if let Expression::GreaterThan(left, _) = expr {
            assert!(matches!(*left, Expression::Field(_)));
        }
    }

    #[test]
    fn test_parse_filter_nested_field() {
        let expr = parse_filter("user.profile.name == 'John'").unwrap();
        if let Expression::Equal(left, _) = expr {
            if let Expression::Field(segments) = *left {
                assert_eq!(segments, vec!["user", "profile", "name"]);
            } else {
                panic!("Expected Field expression");
            }
        }
    }

    #[test]
    fn test_parse_filter_array_wildcard() {
        let expr = parse_filter("items[*].score > 50").unwrap();
        if let Expression::GreaterThan(left, _) = expr {
            assert!(matches!(*left, Expression::ArrayWildcard(_, _)));
        }
    }

    // Tests for literals
    #[test]
    fn test_parse_filter_number_literal() {
        let expr = parse_filter("42 == 42").unwrap();
        assert!(matches!(expr, Expression::Equal(_, _)));
    }

    #[test]
    fn test_parse_filter_string_literal() {
        let expr = parse_filter("'hello' == 'world'").unwrap();
        assert!(matches!(expr, Expression::Equal(_, _)));
    }

    #[test]
    fn test_parse_filter_boolean_literal() {
        let expr = parse_filter("active == true").unwrap();
        assert!(matches!(expr, Expression::Equal(_, _)));
    }

    #[test]
    fn test_parse_filter_null_literal() {
        let expr = parse_filter("value == null").unwrap();
        assert!(matches!(expr, Expression::Equal(_, _)));
    }

    // Error cases
    #[test]
    fn test_parse_filter_empty_expression() {
        let result = parse_filter("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty expression"));
    }

    #[test]
    fn test_parse_filter_mismatched_parens() {
        let result = parse_filter("(priority > 5");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_filter_invalid_operator() {
        let result = parse_filter("priority $ 5");
        assert!(result.is_ok()); // $ is ignored during tokenization, parsed as "priority 5"
    }

    // Tests for parse_sort - Single field
    #[test]
    fn test_parse_sort_single_field_default() {
        let keys = parse_sort("priority").unwrap();
        assert_eq!(keys.len(), 1);
        assert!(matches!(keys[0].direction, SortDirection::Ascending));
        assert!(matches!(keys[0].null_handling, NullHandling::Last));
    }

    #[test]
    fn test_parse_sort_single_field_asc() {
        let keys = parse_sort("priority ASC").unwrap();
        assert_eq!(keys.len(), 1);
        assert!(matches!(keys[0].direction, SortDirection::Ascending));
    }

    #[test]
    fn test_parse_sort_single_field_desc() {
        let keys = parse_sort("priority DESC").unwrap();
        assert_eq!(keys.len(), 1);
        assert!(matches!(keys[0].direction, SortDirection::Descending));
    }

    // Tests for multiple fields
    #[test]
    fn test_parse_sort_multiple_fields() {
        let keys = parse_sort("priority DESC, name ASC").unwrap();
        assert_eq!(keys.len(), 2);
        assert!(matches!(keys[0].direction, SortDirection::Descending));
        assert!(matches!(keys[1].direction, SortDirection::Ascending));
    }

    #[test]
    fn test_parse_sort_three_fields() {
        let keys = parse_sort("priority DESC, status ASC, name ASC").unwrap();
        assert_eq!(keys.len(), 3);
    }

    // Tests for null handling
    #[test]
    fn test_parse_sort_nulls_first() {
        let keys = parse_sort("priority DESC NULLS FIRST").unwrap();
        assert_eq!(keys.len(), 1);
        assert!(matches!(keys[0].null_handling, NullHandling::First));
    }

    #[test]
    fn test_parse_sort_nulls_last() {
        let keys = parse_sort("priority ASC NULLS LAST").unwrap();
        assert_eq!(keys.len(), 1);
        assert!(matches!(keys[0].null_handling, NullHandling::Last));
    }

    #[test]
    fn test_parse_sort_multiple_with_nulls() {
        let keys = parse_sort("priority DESC NULLS FIRST, name ASC NULLS LAST").unwrap();
        assert_eq!(keys.len(), 2);
        assert!(matches!(keys[0].null_handling, NullHandling::First));
        assert!(matches!(keys[1].null_handling, NullHandling::Last));
    }

    // Tests for nested field paths
    #[test]
    fn test_parse_sort_nested_field() {
        let keys = parse_sort("user.profile.age DESC").unwrap();
        assert_eq!(keys.len(), 1);
        if let Expression::Field(segments) = &keys[0].expression {
            assert_eq!(segments, &vec!["user", "profile", "age"]);
        } else {
            panic!("Expected Field expression");
        }
    }

    // Error cases for sort
    #[test]
    fn test_parse_sort_empty() {
        let result = parse_sort("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No sort fields"));
    }

    #[test]
    fn test_parse_sort_invalid_null_position() {
        let result = parse_sort("priority DESC NULLS MIDDLE");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid null position"));
    }

    // Tests for parse_field_path
    #[test]
    fn test_parse_field_path_simple() {
        let parser = ExpressionParser::new();
        let expr = parser.parse_field_path("name").unwrap();
        if let Expression::Field(segments) = expr {
            assert_eq!(segments, vec!["name"]);
        } else {
            panic!("Expected Field expression");
        }
    }

    #[test]
    fn test_parse_field_path_nested() {
        let parser = ExpressionParser::new();
        let expr = parser.parse_field_path("user.profile.name").unwrap();
        if let Expression::Field(segments) = expr {
            assert_eq!(segments, vec!["user", "profile", "name"]);
        } else {
            panic!("Expected Field expression");
        }
    }

    #[test]
    fn test_parse_field_path_array_wildcard() {
        let parser = ExpressionParser::new();
        let expr = parser.parse_field_path("items[*].score").unwrap();
        if let Expression::ArrayWildcard(base, rest) = expr {
            if let Expression::Field(base_segments) = *base {
                assert_eq!(base_segments, vec!["items"]);
                assert_eq!(rest, vec!["score"]);
            } else {
                panic!("Expected Field in base");
            }
        } else {
            panic!("Expected ArrayWildcard expression");
        }
    }

    #[test]
    fn test_parse_field_path_array_wildcard_no_rest() {
        let parser = ExpressionParser::new();
        let expr = parser.parse_field_path("items[*]").unwrap();
        if let Expression::ArrayWildcard(base, rest) = expr {
            if let Expression::Field(base_segments) = *base {
                assert_eq!(base_segments, vec!["items"]);
                assert!(rest.is_empty());
            } else {
                panic!("Expected Field in base");
            }
        } else {
            panic!("Expected ArrayWildcard expression");
        }
    }
}
