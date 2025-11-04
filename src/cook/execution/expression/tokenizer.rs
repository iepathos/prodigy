//! Lexical analysis (tokenization) for expression parsing
//!
//! This module converts expression strings into sequences of tokens.
//! The tokenizer is designed to be a pure function with no side effects.

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
                let mut string = String::new();
                for ch in chars.by_ref() {
                    if ch == quote {
                        break;
                    }
                    string.push(ch);
                }
                tokens.push(Token::String(string));
            }
            '0'..='9' | '-' => {
                let mut num_str = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_numeric() || ch == '.' || ch == '-' {
                        num_str.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if let Ok(num) = num_str.parse::<f64>() {
                    tokens.push(Token::Number(num));
                } else {
                    return Err(anyhow!("Invalid number: {}", num_str));
                }
            }
            _ if ch.is_alphabetic() || ch == '_' => {
                let mut ident = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric()
                        || ch == '_'
                        || ch == '.'
                        || ch == '['
                        || ch == ']'
                        || ch == '*'
                    {
                        ident.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }

                // Check for keywords (case-insensitive for operators)
                match ident.to_lowercase().as_str() {
                    "true" => tokens.push(Token::Boolean(true)),
                    "false" => tokens.push(Token::Boolean(false)),
                    "null" => tokens.push(Token::Null),
                    "in" => tokens.push(Token::In),
                    "and" => tokens.push(Token::And),
                    "or" => tokens.push(Token::Or),
                    "not" => tokens.push(Token::Not),
                    "contains" => tokens.push(Token::Contains),
                    "starts_with" | "startswith" => tokens.push(Token::StartsWith),
                    "ends_with" | "endswith" => tokens.push(Token::EndsWith),
                    "matches" => tokens.push(Token::Matches),
                    "length" => tokens.push(Token::Length),
                    "sum" => tokens.push(Token::Sum),
                    "count" => tokens.push(Token::Count),
                    "min" => tokens.push(Token::Min),
                    "max" => tokens.push(Token::Max),
                    "avg" => tokens.push(Token::Avg),
                    _ => tokens.push(Token::Identifier(ident)),
                }
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
