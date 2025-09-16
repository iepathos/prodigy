---
number: 60
title: Filter and Sort Expression Implementation
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-15
---

# Specification 60: Filter and Sort Expression Implementation

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The MapReduce configuration supports `filter` and `sort_by` fields for controlling which items are processed and in what order, but the actual implementation is missing. Users cannot filter work items based on properties (e.g., `item.score >= 5`) or sort them by priority (e.g., `item.priority DESC`). This limits the ability to implement sophisticated processing strategies like focusing on high-value items first or skipping items that don't meet criteria.

## Objective

Implement a complete expression evaluation system for filtering and sorting MapReduce work items, supporting JSONPath field access, comparison operators, logical combinations, and multiple sort keys.

## Requirements

### Functional Requirements

1. **Filter Expression Engine**
   - Support comparison operators: `==`, `!=`, `>`, `>=`, `<`, `<=`
   - Support logical operators: `&&`, `||`, `!`
   - Support string operations: `contains`, `starts_with`, `ends_with`
   - Support regex matching: `matches(/pattern/)`
   - Support null checking: `is_null`, `is_not_null`
   - Support type checking: `is_number`, `is_string`, `is_bool`

2. **Field Access**
   - JSONPath notation: `item.field.nested.value`
   - Array access: `item.tags[0]`, `item.results[*].score`
   - Special variables: `_index`, `_key`, `_value`
   - Computed fields: `length(item.tags)`, `sum(item.scores)`

3. **Sort Expression Support**
   - Single field sorting: `item.priority`
   - Multiple sort keys: `item.category, item.score DESC`
   - Direction specifiers: `ASC` (default), `DESC`
   - Null handling: `NULLS FIRST`, `NULLS LAST`
   - Custom collation for strings

4. **Expression Validation**
   - Syntax validation at parse time
   - Type checking where possible
   - Clear error messages for invalid expressions
   - Suggestions for common mistakes

5. **Performance Optimization**
   - Compile expressions once, evaluate many times
   - Short-circuit evaluation for logical operators
   - Index-based filtering when possible
   - Lazy evaluation for complex expressions

### Non-Functional Requirements

1. **Performance**
   - Filter 10,000 items in under 100ms
   - Minimal memory overhead for expression evaluation
   - Efficient handling of deeply nested JSON

2. **Usability**
   - Intuitive syntax similar to JavaScript/SQL
   - Helpful error messages with position indicators
   - Expression builder/validator tool

3. **Security**
   - Prevent code injection attacks
   - Limit expression complexity to prevent DoS
   - Sandbox expression evaluation

## Acceptance Criteria

- [ ] Filter expressions correctly evaluate for all supported operators
- [ ] Sort expressions produce correct ordering for all data types
- [ ] Complex expressions with nested logic work correctly
- [ ] Invalid expressions produce clear error messages
- [ ] JSONPath field access works for arbitrarily nested structures
- [ ] Performance meets specified benchmarks
- [ ] Expression validation catches syntax errors before execution
- [ ] Multiple sort keys are applied in correct precedence
- [ ] Null values are handled consistently in filters and sorts
- [ ] Documentation includes comprehensive examples
- [ ] Expression builder CLI tool available for testing

## Technical Details

### Implementation Approach

```rust
pub struct ExpressionEngine {
    parser: ExpressionParser,
    evaluator: ExpressionEvaluator,
    optimizer: ExpressionOptimizer,
}

impl ExpressionEngine {
    pub fn compile_filter(expr: &str) -> Result<CompiledFilter> {
        let ast = self.parser.parse_filter(expr)?;
        let optimized = self.optimizer.optimize(ast)?;
        Ok(CompiledFilter::new(optimized))
    }

    pub fn compile_sort(expr: &str) -> Result<CompiledSort> {
        let sort_keys = self.parser.parse_sort(expr)?;
        Ok(CompiledSort::new(sort_keys))
    }
}

impl CompiledFilter {
    pub fn evaluate(&self, item: &Value) -> Result<bool> {
        self.evaluator.eval_bool(&self.ast, item)
    }
}
```

### Architecture Changes

1. **Expression Module**
   - New `expression` module in `cook::execution`
   - Parser, evaluator, and optimizer components
   - Expression AST representation

2. **Integration Points**
   - Hook into MapReduceExecutor's item processing
   - Modify data pipeline for filtering/sorting
   - Update configuration parsing

### Data Structures

```rust
pub enum Expression {
    // Literals
    Number(f64),
    String(String),
    Boolean(bool),
    Null,

    // Field access
    Field(Vec<String>), // JSONPath segments
    Index(Box<Expression>, Box<Expression>),

    // Operators
    Equal(Box<Expression>, Box<Expression>),
    NotEqual(Box<Expression>, Box<Expression>),
    GreaterThan(Box<Expression>, Box<Expression>),
    LessThan(Box<Expression>, Box<Expression>),
    And(Box<Expression>, Box<Expression>),
    Or(Box<Expression>, Box<Expression>),
    Not(Box<Expression>),

    // Functions
    Contains(Box<Expression>, Box<Expression>),
    Length(Box<Expression>),
    Sum(Box<Expression>),
}

pub struct SortKey {
    pub expression: Expression,
    pub direction: SortDirection,
    pub null_handling: NullHandling,
}

pub enum SortDirection {
    Ascending,
    Descending,
}

pub enum NullHandling {
    First,
    Last,
}
```

### APIs and Interfaces

```rust
pub trait Filterable {
    fn apply_filter(&self, filter: &CompiledFilter) -> Result<Vec<Value>>;
}

pub trait Sortable {
    fn apply_sort(&mut self, sort: &CompiledSort) -> Result<()>;
}

pub trait ExpressionEvaluator {
    fn evaluate(&self, expr: &Expression, context: &Value) -> Result<Value>;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/cook/execution/mapreduce.rs`
  - `src/cook/execution/data_pipeline.rs`
  - `src/config/mapreduce.rs`
- **External Dependencies**:
  - Consider `serde_json` for JSONPath
  - Possible `pest` for expression parsing

## Testing Strategy

- **Unit Tests**:
  - Expression parser for all syntax variations
  - Evaluator for all operators and functions
  - Sort comparator for different data types

- **Integration Tests**:
  - End-to-end filtering in MapReduce workflow
  - Complex multi-key sorting scenarios
  - Performance benchmarks with large datasets

- **Fuzz Testing**:
  - Random expression generation
  - Malformed input handling
  - Edge cases and boundary conditions

- **User Acceptance**:
  - Real-world filtering scenarios
  - Performance with production data
  - Expression builder tool usability

## Documentation Requirements

- **Code Documentation**:
  - Expression syntax reference
  - Operator precedence table
  - Function reference with examples

- **User Documentation**:
  - Expression cookbook with common patterns
  - Migration guide from other systems
  - Performance tuning guidelines

- **Architecture Updates**:
  - Expression evaluation architecture
  - Integration with MapReduce pipeline
  - Optimization strategies

## Implementation Notes

1. **Parser Choice**: Consider using `pest` or `nom` for robust parsing
2. **Type Coercion**: Define clear rules for automatic type conversion
3. **Error Recovery**: Implement partial evaluation for debugging
4. **Caching**: Cache compiled expressions for reuse
5. **Benchmarking**: Create comprehensive benchmark suite

## Migration and Compatibility

- No breaking changes to existing workflows without filters
- Graceful handling of missing fields in expressions
- Optional strict mode for type checking
- Expression version field for future syntax evolution