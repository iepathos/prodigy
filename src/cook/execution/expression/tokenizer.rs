//! Lexical analysis (tokenization) for expression parsing
//!
//! This module converts expression strings into sequences of tokens.
//! The tokenizer is designed to be a pure function with no side effects.
//!
//! ## Architecture
//!
//! The tokenizer uses a delegation pattern with specialized helper functions:
//!
//! - **`tokenize()`**: Main coordinator that iterates through characters
//! - **`parse_operator()`**: Handles operator tokens with lookahead (==, !=, >=, <=, &&, ||)
//! - **`parse_string()`**: Parses quoted string literals
//! - **`parse_number()`**: Parses numeric literals (integers, floats, negative numbers)
//! - **`parse_identifier()`**: Collects identifier characters (including field paths and array accessors)
//! - **`parse_keyword_or_identifier()`**: Distinguishes keywords from identifiers
//!
//! This design keeps each function focused on a single responsibility, making the code
//! easier to test, understand, and maintain.

use anyhow::{anyhow, Result};
use std::iter::Peekable;
use std::str::Chars;

/// Token types for the lexer
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    Identifier(String),

    // Operators
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterEqual,
    LessEqual,
    And,
    Or,
    Not,
    In,

    // Functions
    Contains,
    StartsWith,
    EndsWith,
    Matches,

    // Aggregate Functions
    Length,
    Sum,
    Count,
    Min,
    Max,
    Avg,

    // Punctuation
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
}

/// Parse operator tokens (!, !=, =, ==, >, >=, <, <=, &&, ||)
///
/// This helper function handles all operator parsing, including lookahead
/// for multi-character operators.
///
/// Returns `Some(Token)` if the character is an operator, `None` otherwise.
fn parse_operator(ch: char, chars: &mut Peekable<Chars>) -> Result<Option<Token>> {
    let token = match ch {
        '!' => {
            chars.next();
            if chars.peek() == Some(&'=') {
                chars.next();
                Token::NotEqual
            } else {
                Token::Not
            }
        }
        '=' => {
            chars.next();
            if chars.peek() == Some(&'=') {
                chars.next();
                Token::Equal
            } else {
                Token::Equal // Single = also means equal
            }
        }
        '>' => {
            chars.next();
            if chars.peek() == Some(&'=') {
                chars.next();
                Token::GreaterEqual
            } else {
                Token::Greater
            }
        }
        '<' => {
            chars.next();
            if chars.peek() == Some(&'=') {
                chars.next();
                Token::LessEqual
            } else {
                Token::Less
            }
        }
        '&' => {
            chars.next();
            if chars.peek() == Some(&'&') {
                chars.next();
                Token::And
            } else {
                return Err(anyhow!("Expected && but got single &"));
            }
        }
        '|' => {
            chars.next();
            if chars.peek() == Some(&'|') {
                chars.next();
                Token::Or
            } else {
                return Err(anyhow!("Expected || but got single |"));
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(token))
}

/// Parse a quoted string literal
///
/// Consumes characters from the iterator until the closing quote is found.
/// The opening quote has already been consumed by the caller.
///
/// # Arguments
/// * `quote` - The quote character (' or ") that opened the string
/// * `chars` - The character iterator
///
/// # Returns
/// The parsed string content (without quotes)
fn parse_string(quote: char, chars: &mut Peekable<Chars>) -> String {
    let mut string = String::new();
    for ch in chars.by_ref() {
        if ch == quote {
            break;
        }
        string.push(ch);
    }
    string
}

/// Parse a numeric literal (integer or float, positive or negative)
///
/// Consumes numeric characters and returns the parsed number.
/// The first character (digit or '-') is still in the iterator.
///
/// # Arguments
/// * `chars` - The character iterator
///
/// # Returns
/// The parsed number as f64
fn parse_number(chars: &mut Peekable<Chars>) -> Result<f64> {
    let mut num_str = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_numeric() || ch == '.' || ch == '-' {
            num_str.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    num_str
        .parse::<f64>()
        .map_err(|_| anyhow!("Invalid number: {}", num_str))
}

/// Convert identifier string to keyword token or Identifier token
///
/// Checks if the identifier matches a known keyword (case-insensitive for
/// operators) and returns the appropriate token type.
///
/// # Arguments
/// * `ident` - The raw identifier string
///
/// # Returns
/// Token::Identifier if not a keyword, otherwise the appropriate keyword token
fn parse_keyword_or_identifier(ident: String) -> Token {
    match ident.to_lowercase().as_str() {
        "true" => Token::Boolean(true),
        "false" => Token::Boolean(false),
        "null" => Token::Null,
        "in" => Token::In,
        "and" => Token::And,
        "or" => Token::Or,
        "not" => Token::Not,
        "contains" => Token::Contains,
        "starts_with" | "startswith" => Token::StartsWith,
        "ends_with" | "endswith" => Token::EndsWith,
        "matches" => Token::Matches,
        "length" => Token::Length,
        "sum" => Token::Sum,
        "count" => Token::Count,
        "min" => Token::Min,
        "max" => Token::Max,
        "avg" => Token::Avg,
        _ => Token::Identifier(ident),
    }
}

/// Parse an identifier (variable name, field path, or array accessor)
///
/// Collects identifier characters including alphanumerics, underscores,
/// dots (for field paths), brackets (for array access), and wildcards.
///
/// # Arguments
/// * `chars` - The character iterator
///
/// # Returns
/// The raw identifier string (e.g., "user.profile.name" or "items\[*\].score")
fn parse_identifier(chars: &mut Peekable<Chars>) -> String {
    let mut ident = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_alphanumeric() || ch == '_' || ch == '.' || ch == '[' || ch == ']' || ch == '*' {
            ident.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    ident
}

/// Tokenize an expression string into a sequence of tokens
///
/// This is a pure function that converts a string into tokens.
/// It handles:
/// - Literals (numbers, strings, booleans, null)
/// - Operators (==, !=, &&, ||, >, <, >=, <=, !)
/// - Identifiers and keywords
/// - Punctuation (parentheses, brackets)
///
/// # Examples
///
/// ```
/// use prodigy::cook::execution::expression::tokenizer::tokenize;
///
/// let tokens = tokenize("priority > 5").unwrap();
/// assert_eq!(tokens.len(), 3);
/// ```
pub fn tokenize(expr: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\n' => {
                chars.next();
            }
            '(' => {
                tokens.push(Token::LeftParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RightParen);
                chars.next();
            }
            '!' | '=' | '>' | '<' | '&' | '|' => {
                if let Some(token) = parse_operator(ch, &mut chars)? {
                    tokens.push(token);
                }
            }
            '"' | '\'' => {
                let quote = ch;
                chars.next();
                let string = parse_string(quote, &mut chars);
                tokens.push(Token::String(string));
            }
            '0'..='9' | '-' => {
                let num = parse_number(&mut chars)?;
                tokens.push(Token::Number(num));
            }
            _ if ch.is_alphabetic() || ch == '_' => {
                let ident = parse_identifier(&mut chars);
                let token = parse_keyword_or_identifier(ident);
                tokens.push(token);
            }
            _ => {
                chars.next();
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_number() {
        let tokens = tokenize("42").unwrap();
        assert_eq!(tokens, vec![Token::Number(42.0)]);
    }

    #[test]
    fn test_tokenize_negative_number() {
        let tokens = tokenize("-42").unwrap();
        assert_eq!(tokens, vec![Token::Number(-42.0)]);
    }

    #[test]
    fn test_tokenize_float() {
        let tokens = tokenize("3.25").unwrap();
        assert_eq!(tokens, vec![Token::Number(3.25)]);
    }

    #[test]
    fn test_tokenize_string_double_quotes() {
        let tokens = tokenize("\"hello\"").unwrap();
        assert_eq!(tokens, vec![Token::String("hello".to_string())]);
    }

    #[test]
    fn test_tokenize_string_single_quotes() {
        let tokens = tokenize("'world'").unwrap();
        assert_eq!(tokens, vec![Token::String("world".to_string())]);
    }

    #[test]
    fn test_tokenize_boolean_true() {
        let tokens = tokenize("true").unwrap();
        assert_eq!(tokens, vec![Token::Boolean(true)]);
    }

    #[test]
    fn test_tokenize_boolean_false() {
        let tokens = tokenize("false").unwrap();
        assert_eq!(tokens, vec![Token::Boolean(false)]);
    }

    #[test]
    fn test_tokenize_null() {
        let tokens = tokenize("null").unwrap();
        assert_eq!(tokens, vec![Token::Null]);
    }

    #[test]
    fn test_tokenize_identifier() {
        let tokens = tokenize("priority").unwrap();
        assert_eq!(tokens, vec![Token::Identifier("priority".to_string())]);
    }

    #[test]
    fn test_tokenize_field_path() {
        let tokens = tokenize("user.profile.name").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Identifier("user.profile.name".to_string())]
        );
    }

    #[test]
    fn test_tokenize_equal() {
        let tokens = tokenize("==").unwrap();
        assert_eq!(tokens, vec![Token::Equal]);
    }

    #[test]
    fn test_tokenize_single_equal() {
        let tokens = tokenize("=").unwrap();
        assert_eq!(tokens, vec![Token::Equal]);
    }

    #[test]
    fn test_tokenize_not_equal() {
        let tokens = tokenize("!=").unwrap();
        assert_eq!(tokens, vec![Token::NotEqual]);
    }

    #[test]
    fn test_tokenize_greater() {
        let tokens = tokenize(">").unwrap();
        assert_eq!(tokens, vec![Token::Greater]);
    }

    #[test]
    fn test_tokenize_greater_equal() {
        let tokens = tokenize(">=").unwrap();
        assert_eq!(tokens, vec![Token::GreaterEqual]);
    }

    #[test]
    fn test_tokenize_less() {
        let tokens = tokenize("<").unwrap();
        assert_eq!(tokens, vec![Token::Less]);
    }

    #[test]
    fn test_tokenize_less_equal() {
        let tokens = tokenize("<=").unwrap();
        assert_eq!(tokens, vec![Token::LessEqual]);
    }

    #[test]
    fn test_tokenize_and_symbols() {
        let tokens = tokenize("&&").unwrap();
        assert_eq!(tokens, vec![Token::And]);
    }

    #[test]
    fn test_tokenize_and_word() {
        let tokens = tokenize("and").unwrap();
        assert_eq!(tokens, vec![Token::And]);
    }

    #[test]
    fn test_tokenize_or_symbols() {
        let tokens = tokenize("||").unwrap();
        assert_eq!(tokens, vec![Token::Or]);
    }

    #[test]
    fn test_tokenize_or_word() {
        let tokens = tokenize("or").unwrap();
        assert_eq!(tokens, vec![Token::Or]);
    }

    #[test]
    fn test_tokenize_not_symbol() {
        let tokens = tokenize("!").unwrap();
        assert_eq!(tokens, vec![Token::Not]);
    }

    #[test]
    fn test_tokenize_not_word() {
        let tokens = tokenize("not").unwrap();
        assert_eq!(tokens, vec![Token::Not]);
    }

    #[test]
    fn test_tokenize_contains() {
        let tokens = tokenize("contains").unwrap();
        assert_eq!(tokens, vec![Token::Contains]);
    }

    #[test]
    fn test_tokenize_starts_with() {
        let tokens = tokenize("starts_with").unwrap();
        assert_eq!(tokens, vec![Token::StartsWith]);
    }

    #[test]
    fn test_tokenize_starts_with_no_underscore() {
        let tokens = tokenize("startswith").unwrap();
        assert_eq!(tokens, vec![Token::StartsWith]);
    }

    #[test]
    fn test_tokenize_ends_with() {
        let tokens = tokenize("ends_with").unwrap();
        assert_eq!(tokens, vec![Token::EndsWith]);
    }

    #[test]
    fn test_tokenize_matches() {
        let tokens = tokenize("matches").unwrap();
        assert_eq!(tokens, vec![Token::Matches]);
    }

    #[test]
    fn test_tokenize_length() {
        let tokens = tokenize("length").unwrap();
        assert_eq!(tokens, vec![Token::Length]);
    }

    #[test]
    fn test_tokenize_sum() {
        let tokens = tokenize("sum").unwrap();
        assert_eq!(tokens, vec![Token::Sum]);
    }

    #[test]
    fn test_tokenize_count() {
        let tokens = tokenize("count").unwrap();
        assert_eq!(tokens, vec![Token::Count]);
    }

    #[test]
    fn test_tokenize_min() {
        let tokens = tokenize("min").unwrap();
        assert_eq!(tokens, vec![Token::Min]);
    }

    #[test]
    fn test_tokenize_max() {
        let tokens = tokenize("max").unwrap();
        assert_eq!(tokens, vec![Token::Max]);
    }

    #[test]
    fn test_tokenize_avg() {
        let tokens = tokenize("avg").unwrap();
        assert_eq!(tokens, vec![Token::Avg]);
    }

    #[test]
    fn test_tokenize_parentheses() {
        let tokens = tokenize("()").unwrap();
        assert_eq!(tokens, vec![Token::LeftParen, Token::RightParen]);
    }

    #[test]
    fn test_tokenize_simple_comparison() {
        let tokens = tokenize("priority > 5").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Identifier("priority".to_string()),
                Token::Greater,
                Token::Number(5.0),
            ]
        );
    }

    #[test]
    fn test_tokenize_complex_expression() {
        let tokens = tokenize("priority > 5 && status == 'active'").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Identifier("priority".to_string()),
                Token::Greater,
                Token::Number(5.0),
                Token::And,
                Token::Identifier("status".to_string()),
                Token::Equal,
                Token::String("active".to_string()),
            ]
        );
    }

    #[test]
    fn test_tokenize_with_parentheses() {
        let tokens = tokenize("(priority > 5)").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::LeftParen,
                Token::Identifier("priority".to_string()),
                Token::Greater,
                Token::Number(5.0),
                Token::RightParen,
            ]
        );
    }

    #[test]
    fn test_tokenize_function_call() {
        let tokens = tokenize("length(items)").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Length,
                Token::LeftParen,
                Token::Identifier("items".to_string()),
                Token::RightParen,
            ]
        );
    }

    #[test]
    fn test_tokenize_array_wildcard() {
        let tokens = tokenize("items[*].score").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Identifier("items[*].score".to_string())]
        );
    }

    #[test]
    fn test_tokenize_number_followed_by_identifier() {
        // "123abc" is tokenized as a number "123" followed by identifier "abc"
        // This is valid tokenization - semantic errors are caught by the parser
        let tokens = tokenize("123abc").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::Number(123.0));
        assert_eq!(tokens[1], Token::Identifier("abc".to_string()));
    }

    #[test]
    fn test_tokenize_error_single_ampersand() {
        let result = tokenize("priority > 5 & status == 'active'");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected && but got single &"));
    }

    #[test]
    fn test_tokenize_error_single_pipe() {
        let result = tokenize("priority > 5 | status == 'active'");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected || but got single |"));
    }
}
