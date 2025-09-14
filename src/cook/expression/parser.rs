//! Expression parser for conditional workflow execution

use super::value::Value;
use anyhow::{anyhow, Result};

/// Expression types
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// Variable reference (e.g., ${var})
    Variable(String),
    /// Literal value
    Literal(Value),
    /// Comparison operation
    Comparison {
        left: Box<Expression>,
        op: ComparisonOp,
        right: Box<Expression>,
    },
    /// Logical operation
    Logical {
        left: Box<Expression>,
        op: LogicalOp,
        right: Box<Expression>,
    },
    /// Negation
    Not(Box<Expression>),
    /// Check if variable exists
    Exists(String),
}

/// Comparison operators
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

/// Logical operators
#[derive(Debug, Clone, PartialEq)]
pub enum LogicalOp {
    And,
    Or,
}

/// Expression parser
#[derive(Debug)]
pub struct ExpressionParser;

impl ExpressionParser {
    /// Create a new parser
    pub fn new() -> Self {
        Self
    }

    /// Parse an expression string
    pub fn parse(&self, input: &str) -> Result<Expression> {
        let mut tokens = tokenize(input)?;
        parse_logical_or(&mut tokens)
    }
}

/// Token types
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Variable(String),
    String(String),
    Number(f64),
    Bool(bool),
    ComparisonOp(ComparisonOp),
    LogicalOp(LogicalOp),
    Not,
    LeftParen,
    RightParen,
}

/// Tokenize the input string
fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '$' => {
                chars.next();
                if chars.peek() == Some(&'{') {
                    chars.next();
                    let var_name = consume_until(&mut chars, '}')?;

                    // Check for .exists suffix
                    if var_name.ends_with(".exists") {
                        let base_name = var_name.trim_end_matches(".exists");
                        tokens.push(Token::Variable(format!("{}.exists", base_name)));
                    } else {
                        tokens.push(Token::Variable(var_name));
                    }
                } else {
                    return Err(anyhow!("Expected '{{' after '$'"));
                }
            }
            '\'' | '"' => {
                let quote = ch;
                chars.next();
                let string = consume_until(&mut chars, quote)?;
                tokens.push(Token::String(string));
            }
            '(' => {
                chars.next();
                tokens.push(Token::LeftParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RightParen);
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::ComparisonOp(ComparisonOp::NotEqual));
                } else {
                    tokens.push(Token::Not);
                }
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::ComparisonOp(ComparisonOp::Equal));
                } else {
                    return Err(anyhow!("Expected '==' for equality comparison"));
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::ComparisonOp(ComparisonOp::GreaterThanOrEqual));
                } else {
                    tokens.push(Token::ComparisonOp(ComparisonOp::GreaterThan));
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::ComparisonOp(ComparisonOp::LessThanOrEqual));
                } else {
                    tokens.push(Token::ComparisonOp(ComparisonOp::LessThan));
                }
            }
            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Token::LogicalOp(LogicalOp::And));
                } else {
                    return Err(anyhow!("Expected '&&' for logical AND"));
                }
            }
            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push(Token::LogicalOp(LogicalOp::Or));
                } else {
                    return Err(anyhow!("Expected '||' for logical OR"));
                }
            }
            _ if ch.is_ascii_digit() || ch == '-' => {
                let num_str = consume_number(&mut chars)?;
                let num = num_str
                    .parse::<f64>()
                    .map_err(|_| anyhow!("Invalid number: {}", num_str))?;
                tokens.push(Token::Number(num));
            }
            _ if ch.is_ascii_alphabetic() => {
                let word = consume_word(&mut chars);
                match word.as_str() {
                    "true" => tokens.push(Token::Bool(true)),
                    "false" => tokens.push(Token::Bool(false)),
                    _ => return Err(anyhow!("Unexpected word: {}", word)),
                }
            }
            _ => {
                return Err(anyhow!("Unexpected character: '{}'", ch));
            }
        }
    }

    Ok(tokens)
}

/// Consume characters until the delimiter
fn consume_until(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    delimiter: char,
) -> Result<String> {
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch == delimiter {
            return Ok(result);
        }
        result.push(ch);
    }
    Err(anyhow!("Expected '{}' but reached end of input", delimiter))
}

/// Consume a number
fn consume_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String> {
    let mut result = String::new();

    // Handle negative numbers
    if chars.peek() == Some(&'-') {
        result.push(chars.next().unwrap());
    }

    let mut has_dot = false;
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            result.push(chars.next().unwrap());
        } else if ch == '.' && !has_dot {
            has_dot = true;
            result.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    if result.is_empty() || result == "-" {
        Err(anyhow!("Invalid number"))
    } else {
        Ok(result)
    }
}

/// Consume a word
fn consume_word(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut result = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            result.push(chars.next().unwrap());
        } else {
            break;
        }
    }
    result
}

/// Parse logical OR expressions (lowest precedence)
fn parse_logical_or(tokens: &mut Vec<Token>) -> Result<Expression> {
    let mut left = parse_logical_and(tokens)?;

    while !tokens.is_empty() {
        if let Some(Token::LogicalOp(LogicalOp::Or)) = tokens.first() {
            tokens.remove(0);
            let right = parse_logical_and(tokens)?;
            left = Expression::Logical {
                left: Box::new(left),
                op: LogicalOp::Or,
                right: Box::new(right),
            };
        } else {
            break;
        }
    }

    Ok(left)
}

/// Parse logical AND expressions
fn parse_logical_and(tokens: &mut Vec<Token>) -> Result<Expression> {
    let mut left = parse_comparison(tokens)?;

    while !tokens.is_empty() {
        if let Some(Token::LogicalOp(LogicalOp::And)) = tokens.first() {
            tokens.remove(0);
            let right = parse_comparison(tokens)?;
            left = Expression::Logical {
                left: Box::new(left),
                op: LogicalOp::And,
                right: Box::new(right),
            };
        } else {
            break;
        }
    }

    Ok(left)
}

/// Parse comparison expressions
fn parse_comparison(tokens: &mut Vec<Token>) -> Result<Expression> {
    let left = parse_unary(tokens)?;

    if !tokens.is_empty() {
        if let Some(Token::ComparisonOp(op)) = tokens.first() {
            let op = op.clone();
            tokens.remove(0);
            let right = parse_unary(tokens)?;
            return Ok(Expression::Comparison {
                left: Box::new(left),
                op,
                right: Box::new(right),
            });
        }
    }

    Ok(left)
}

/// Parse unary expressions (NOT, parentheses, literals)
fn parse_unary(tokens: &mut Vec<Token>) -> Result<Expression> {
    if tokens.is_empty() {
        return Err(anyhow!("Unexpected end of expression"));
    }

    match tokens.remove(0) {
        Token::Not => {
            let inner = parse_unary(tokens)?;
            Ok(Expression::Not(Box::new(inner)))
        }
        Token::LeftParen => {
            let inner = parse_logical_or(tokens)?;
            if tokens.is_empty() || tokens.remove(0) != Token::RightParen {
                return Err(anyhow!("Expected closing parenthesis"));
            }
            Ok(inner)
        }
        Token::Variable(name) => {
            if name.ends_with(".exists") {
                let base_name = name.trim_end_matches(".exists");
                Ok(Expression::Exists(base_name.to_string()))
            } else {
                Ok(Expression::Variable(name))
            }
        }
        Token::String(s) => Ok(Expression::Literal(Value::String(s))),
        Token::Number(n) => Ok(Expression::Literal(Value::Number(n))),
        Token::Bool(b) => Ok(Expression::Literal(Value::Bool(b))),
        _ => Err(anyhow!("Unexpected token in expression")),
    }
}

/// Parse a simple expression string (public helper)
pub fn parse_expression(input: &str) -> Result<Expression> {
    let parser = ExpressionParser::new();
    parser.parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple() {
        let tokens = tokenize("${var} == 'value'").unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], Token::Variable("var".to_string()));
        assert_eq!(tokens[1], Token::ComparisonOp(ComparisonOp::Equal));
        assert_eq!(tokens[2], Token::String("value".to_string()));
    }

    #[test]
    fn test_tokenize_complex() {
        let tokens = tokenize("${a} > 10 && ${b} != 'test'").unwrap();
        assert_eq!(tokens.len(), 7);
    }

    #[test]
    fn test_parse_simple_variable() {
        let expr = parse_expression("${test}").unwrap();
        assert!(matches!(expr, Expression::Variable(ref name) if name == "test"));
    }

    #[test]
    fn test_parse_comparison() {
        let expr = parse_expression("${score} >= 80").unwrap();
        assert!(matches!(expr, Expression::Comparison { .. }));
    }

    #[test]
    fn test_parse_logical() {
        let expr = parse_expression("${a} && ${b}").unwrap();
        assert!(matches!(expr, Expression::Logical { .. }));
    }

    #[test]
    fn test_parse_exists() {
        let expr = parse_expression("${var.exists}").unwrap();
        assert!(matches!(expr, Expression::Exists(ref name) if name == "var"));
    }

    #[test]
    fn test_parse_parentheses() {
        let expr = parse_expression("(${a} || ${b}) && ${c}").unwrap();
        assert!(matches!(expr, Expression::Logical { .. }));
    }

    #[test]
    fn test_parse_not() {
        let expr = parse_expression("!${flag}").unwrap();
        assert!(matches!(expr, Expression::Not(_)));
    }
}
