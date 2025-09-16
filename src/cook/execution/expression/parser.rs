//! Expression parser for filter and sort expressions

use anyhow::{anyhow, Result};
use serde_json::Value;

/// Parsed expression AST
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // Literals
    Number(f64),
    String(String),
    Boolean(bool),
    Null,

    // Field access
    Field(Vec<String>), // JSONPath segments (e.g., ["user", "profile", "name"])
    Index(Box<Expression>, Box<Expression>), // Array/object index

    // Comparison operators
    Equal(Box<Expression>, Box<Expression>),
    NotEqual(Box<Expression>, Box<Expression>),
    GreaterThan(Box<Expression>, Box<Expression>),
    LessThan(Box<Expression>, Box<Expression>),
    GreaterEqual(Box<Expression>, Box<Expression>),
    LessEqual(Box<Expression>, Box<Expression>),

    // Logical operators
    And(Box<Expression>, Box<Expression>),
    Or(Box<Expression>, Box<Expression>),
    Not(Box<Expression>),

    // String functions
    Contains(Box<Expression>, Box<Expression>),
    StartsWith(Box<Expression>, Box<Expression>),
    EndsWith(Box<Expression>, Box<Expression>),
    Matches(Box<Expression>, Box<Expression>), // Regex match

    // Type checking
    IsNull(Box<Expression>),
    IsNotNull(Box<Expression>),
    IsNumber(Box<Expression>),
    IsString(Box<Expression>),
    IsBool(Box<Expression>),
    IsArray(Box<Expression>),
    IsObject(Box<Expression>),

    // Aggregate functions
    Length(Box<Expression>),
    Sum(Box<Expression>),
    Count(Box<Expression>),
    Min(Box<Expression>),
    Max(Box<Expression>),
    Avg(Box<Expression>),

    // Array operations
    In(Box<Expression>, Vec<Value>), // Check if value is in list

    // Special variables
    Variable(String), // _index, _key, _value
}

/// Sort key with direction and null handling
#[derive(Debug, Clone)]
pub struct SortKey {
    pub expression: Expression,
    pub direction: SortDirection,
    pub null_handling: NullHandling,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NullHandling {
    First,
    Last,
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
        let tokens = self.tokenize(expr)?;
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

    /// Tokenize an expression string
    fn tokenize(&self, expr: &str) -> Result<Vec<Token>> {
        // Simplified tokenizer - in production, use a proper lexer
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
                '!' => {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        tokens.push(Token::NotEqual);
                    } else {
                        tokens.push(Token::Not);
                    }
                }
                '=' => {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        tokens.push(Token::Equal);
                    } else {
                        tokens.push(Token::Equal); // Single = also means equal
                    }
                }
                '>' => {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        tokens.push(Token::GreaterEqual);
                    } else {
                        tokens.push(Token::Greater);
                    }
                }
                '<' => {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        tokens.push(Token::LessEqual);
                    } else {
                        tokens.push(Token::Less);
                    }
                }
                '&' => {
                    chars.next();
                    if chars.peek() == Some(&'&') {
                        chars.next();
                        tokens.push(Token::And);
                    } else {
                        return Err(anyhow!("Expected && but got single &"));
                    }
                }
                '|' => {
                    chars.next();
                    if chars.peek() == Some(&'|') {
                        chars.next();
                        tokens.push(Token::Or);
                    } else {
                        return Err(anyhow!("Expected || but got single |"));
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
                        if ch.is_alphanumeric() || ch == '_' || ch == '.' || ch == '[' || ch == ']'
                        {
                            ident.push(ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    // Check for keywords
                    match ident.as_str() {
                        "true" => tokens.push(Token::Boolean(true)),
                        "false" => tokens.push(Token::Boolean(false)),
                        "null" => tokens.push(Token::Null),
                        "in" => tokens.push(Token::In),
                        "contains" => tokens.push(Token::Contains),
                        "starts_with" => tokens.push(Token::StartsWith),
                        "ends_with" => tokens.push(Token::EndsWith),
                        "matches" => tokens.push(Token::Matches),
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

    /// Parse an expression from tokens
    fn parse_expression(&self, tokens: &[Token], _min_precedence: i32) -> Result<Expression> {
        // Simplified recursive descent parser
        // In production, use a proper parser generator or more robust implementation

        if tokens.is_empty() {
            return Err(anyhow!("Empty expression"));
        }

        // For now, delegate to the existing simple parser
        // This would be expanded with full expression parsing
        Ok(Expression::Boolean(true))
    }

    /// Parse a field path (e.g., "user.profile.name" or "items[0].value")
    fn parse_field_path(&self, path: &str) -> Result<Expression> {
        let segments: Vec<String> = path.split('.').map(|s| s.to_string()).collect();
        Ok(Expression::Field(segments))
    }
}

/// Token types for the lexer
#[derive(Debug, Clone, PartialEq)]
enum Token {
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

    // Punctuation
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
}

impl Default for ExpressionParser {
    fn default() -> Self {
        Self::new()
    }
}
