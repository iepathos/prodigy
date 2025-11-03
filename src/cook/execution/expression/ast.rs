//! Abstract Syntax Tree types for expression parsing
//!
//! This module defines the AST types used to represent parsed expressions.
//! These types are data-only and separate from parsing logic.

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

/// Sort direction
#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Null handling strategy for sorting
#[derive(Debug, Clone, PartialEq)]
pub enum NullHandling {
    First,
    Last,
}
