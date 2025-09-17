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
    ArrayWildcard(Box<Expression>, Vec<String>), // Array wildcard access (e.g., items[*].score)

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
        // Find all OR positions at this level (not inside parentheses)
        let or_positions = self.find_operators(tokens, &Token::Or)?;

        if or_positions.is_empty() {
            return self.parse_and(tokens);
        }

        // Split by OR and parse each part
        let mut parts = Vec::new();
        let mut start = 0;
        for pos in or_positions {
            if pos > start {
                let part_tokens = &tokens[start..pos];
                parts.push(self.parse_and(part_tokens)?);
            }
            start = pos + 1;
        }
        if start < tokens.len() {
            parts.push(self.parse_and(&tokens[start..])?);
        }

        // Build OR tree from left to right
        let result = parts
            .into_iter()
            .reduce(|left, right| Expression::Or(Box::new(left), Box::new(right)))
            .ok_or_else(|| anyhow!("Empty OR expression"))?;

        Ok(result)
    }

    /// Parse AND expressions
    fn parse_and(&self, tokens: &[Token]) -> Result<Expression> {
        // Find all AND positions at this level
        let and_positions = self.find_operators(tokens, &Token::And)?;

        if and_positions.is_empty() {
            return self.parse_comparison(tokens);
        }

        // Split by AND and parse each part
        let mut parts = Vec::new();
        let mut start = 0;
        for pos in and_positions {
            if pos > start {
                let part_tokens = &tokens[start..pos];
                parts.push(self.parse_comparison(part_tokens)?);
            }
            start = pos + 1;
        }
        if start < tokens.len() {
            parts.push(self.parse_comparison(&tokens[start..])?);
        }

        // Build AND tree from left to right
        let result = parts
            .into_iter()
            .reduce(|left, right| Expression::And(Box::new(left), Box::new(right)))
            .ok_or_else(|| anyhow!("Empty AND expression"))?;

        Ok(result)
    }

    /// Find operator positions at the current level (not inside parentheses)
    fn find_operators(&self, tokens: &[Token], op: &Token) -> Result<Vec<usize>> {
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

    /// Parse comparison expressions
    fn parse_comparison(&self, tokens: &[Token]) -> Result<Expression> {
        if tokens.is_empty() {
            return Err(anyhow!("Empty comparison expression"));
        }

        // Handle parenthesized expressions first
        if tokens[0] == Token::LeftParen {
            // Check if entire expression is parenthesized
            let mut depth = 1;
            let mut end = 1;
            while end < tokens.len() && depth > 0 {
                match tokens[end] {
                    Token::LeftParen => depth += 1,
                    Token::RightParen => depth -= 1,
                    _ => {}
                }
                end += 1;
            }

            if depth == 0 && end == tokens.len() {
                // Entire expression is parenthesized, strip parentheses and re-parse
                return self.parse_or(&tokens[1..end - 1]);
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
                    let mut depth = 1;
                    let mut end = 2;
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
                        return Err(anyhow!("Mismatched parentheses in function call"));
                    }
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
                let mut depth = 1;
                let mut end = 1;
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
                // Parse the expression inside parentheses
                self.parse_expression(&tokens[1..end], 0)
            }
            _ => Err(anyhow!("Unexpected token: {:?}", tokens[0])),
        }
    }

    /// Parse a field path (e.g., "user.profile.name" or "items[0].value" or "items[*].score")
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

/// Token types for the lexer
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
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

impl Default for ExpressionParser {
    fn default() -> Self {
        Self::new()
    }
}
