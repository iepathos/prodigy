---
number: 52
title: JSON Path and Data Filtering for MapReduce
category: foundation
priority: high
status: draft
dependencies: [49, 50]
created: 2025-08-18
---

# Specification 52: JSON Path and Data Filtering for MapReduce

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [49 - MapReduce Parallel Execution, 50 - Variable Interpolation Engine]

## Context

The MapReduce implementation includes configuration fields for JSON path expressions (`json_path`), filtering (`filter`), and sorting (`sort_by`), but these features are not implemented. Currently, the `load_work_items` function would need to load an entire JSON file and extract all items, without the ability to:
1. Select specific items using JSON path expressions
2. Filter items based on criteria
3. Sort items by priority or other fields

This is particularly important for tools like Debtmap that might generate hundreds of items, where users want to process only high-priority issues or specific categories.

## Objective

Implement comprehensive data selection and filtering capabilities for MapReduce workflows:
1. JSON path expressions to extract work items from complex JSON structures
2. Filtering syntax to select items based on conditions
3. Sorting capabilities to process items in priority order
4. Support for limiting the number of items processed
5. Data transformation to normalize different input formats

## Requirements

### Functional Requirements

1. **JSON Path Support**
   - Standard JSON path syntax ($.path.to.items[*])
   - Array selectors and filters
   - Recursive descent (..)
   - Wildcards and multi-select
   - Return arrays of matched items

2. **Filtering Capabilities**
   - Expression-based filtering (e.g., "severity == 'high'")
   - Comparison operators (==, !=, <, >, <=, >=)
   - Logical operators (&&, ||, !)
   - String operations (contains, starts_with, ends_with)
   - Null/undefined checking

3. **Sorting Options**
   - Sort by single field
   - Multi-field sorting with precedence
   - Ascending/descending order
   - Numeric vs string sorting
   - Handle missing fields in sort

4. **Data Limits**
   - Limit number of items processed
   - Skip/offset for pagination
   - Random sampling option
   - Top-N selection after sorting

5. **Data Transformation**
   - Flatten nested structures
   - Extract specific fields
   - Rename fields for consistency
   - Convert data types

### Non-Functional Requirements

1. **Performance**
   - Handle JSON files up to 100MB efficiently
   - Stream processing for large files
   - Lazy evaluation where possible

2. **Compatibility**
   - Support various JSON structures from different tools
   - Handle both arrays and objects as root
   - Graceful handling of malformed JSON

3. **Usability**
   - Clear error messages for invalid paths/filters
   - Examples in documentation
   - Validation of expressions at parse time

## Acceptance Criteria

- [ ] Can extract items using JSON path: `$.debt_items[*]`
- [ ] Supports array filtering: `$.items[?(@.priority > 5)]`
- [ ] Filter expressions work: `severity == 'critical' || priority > 8`
- [ ] Sorting works: `sort_by: "priority DESC, severity ASC"`
- [ ] Can limit items: `max_items: 50`
- [ ] Handles nested paths: `$.analysis.results..violations[*]`
- [ ] Provides clear errors for invalid expressions
- [ ] Performance test with 10MB JSON file shows <100ms processing
- [ ] Integration test with Debtmap output format
- [ ] Documentation includes common patterns and examples
- [ ] Unit tests cover all operators and edge cases

## Technical Details

### Implementation Approach

1. Use `jsonpath-rust` or similar crate for JSON path evaluation
2. Build a simple expression parser for filter syntax
3. Implement sorting with configurable comparators
4. Add streaming JSON parser for large files
5. Create a pipeline architecture for data processing

### Architecture Changes

```rust
// src/cook/execution/data_pipeline.rs
pub struct DataPipeline {
    json_path: Option<JsonPath>,
    filter: Option<FilterExpression>,
    sorter: Option<Sorter>,
    limit: Option<usize>,
    offset: Option<usize>,
}

impl DataPipeline {
    pub fn from_config(config: &MapReduceConfig) -> Result<Self>;
    pub fn process(&self, input: &Value) -> Result<Vec<Value>>;
    pub fn process_streaming<R: Read>(&self, reader: R) -> Result<Vec<Value>>;
}

// JSON Path evaluation
pub struct JsonPath {
    expression: String,
    compiled: jsonpath::Compiled,
}

impl JsonPath {
    pub fn compile(expr: &str) -> Result<Self>;
    pub fn select<'a>(&self, data: &'a Value) -> Vec<&'a Value>;
}

// Filter expression AST
pub enum FilterExpression {
    Comparison {
        field: String,
        op: ComparisonOp,
        value: Value,
    },
    Logical {
        op: LogicalOp,
        operands: Vec<FilterExpression>,
    },
    Function {
        name: String,
        args: Vec<String>,
    },
}

pub enum ComparisonOp {
    Equal, NotEqual, Less, Greater, LessEqual, GreaterEqual,
    Contains, StartsWith, EndsWith, Matches,
}

pub enum LogicalOp {
    And, Or, Not,
}

// Sorting configuration
pub struct Sorter {
    fields: Vec<SortField>,
}

pub struct SortField {
    path: String,
    order: SortOrder,
    null_position: NullPosition,
}

pub enum SortOrder {
    Ascending,
    Descending,
}

pub enum NullPosition {
    First,
    Last,
}
```

### Data Flow

```
1. Load JSON file or receive JSON data
2. Apply JSON path to extract items
3. Apply filter expression to selected items
4. Sort filtered items according to configuration
5. Apply limit/offset for final selection
6. Return processed work items
```

### APIs and Interfaces

```yaml
# Enhanced MapReduce configuration
map:
  input: "analysis_results.json"
  
  # JSON path to extract items
  json_path: "$.technical_debt.items[*]"
  
  # Filter expression
  filter: "severity in ['high', 'critical'] && complexity > 10"
  
  # Sorting configuration
  sort_by: 
    - field: "priority"
      order: "desc"
    - field: "complexity"
      order: "desc"
  
  # Limits
  max_items: 50
  offset: 0
  
  # Field mapping (optional)
  field_mapping:
    description: "$.details.summary"
    location: "$.file_info"
```

```rust
// Integration with MapReduceExecutor
impl MapReduceExecutor {
    async fn load_work_items(&self, config: &MapReduceConfig) -> Result<Vec<Value>> {
        // Load JSON file
        let data = self.load_json_file(&config.input).await?;
        
        // Create and apply pipeline
        let pipeline = DataPipeline::from_config(config)?;
        let items = pipeline.process(&data)?;
        
        Ok(items)
    }
}
```

## Dependencies

- **Prerequisites**: 
  - Spec 49 (MapReduce base)
  - Spec 50 (Variable interpolation for field access)
- **Affected Components**:
  - `src/cook/execution/mapreduce.rs` - Integration point
  - `src/config/mapreduce.rs` - Configuration parsing
- **External Dependencies**:
  - `jsonpath` or `jsonpath-rust` - JSON path evaluation
  - `pest` or similar - Expression parsing (optional)
  - `serde_json` - JSON manipulation

## Testing Strategy

- **Unit Tests**:
  - JSON path expressions with various patterns
  - Filter expression parsing and evaluation
  - Sorting with different data types
  - Edge cases (empty arrays, null values)
  - Performance with large datasets

- **Integration Tests**:
  - Debtmap output format processing
  - Complex nested JSON structures
  - Multi-stage filtering and sorting
  - Real-world tool outputs

- **Performance Tests**:
  - Large JSON file processing (10MB+)
  - Complex filter expressions
  - Sorting performance with 1000+ items

## Documentation Requirements

- **Code Documentation**:
  - JSON path syntax guide
  - Filter expression reference
  - Sorting configuration

- **User Documentation**:
  - Common patterns for different tools
  - Performance optimization tips
  - Troubleshooting guide

- **Examples**:
  - Debtmap integration
  - Security scanner output processing
  - Test results filtering

## Implementation Notes

### Phase 1: JSON Path (Day 1)
- Integrate jsonpath crate
- Basic path evaluation
- Array and object navigation

### Phase 2: Filtering (Day 2-3)
- Expression parser
- Comparison operators
- Logical operators
- Function support

### Phase 3: Sorting & Limits (Day 4)
- Multi-field sorting
- Null handling
- Limit/offset implementation

### Key Considerations

1. **Large Files**: Use streaming for files >10MB
2. **Type Safety**: Handle type mismatches gracefully
3. **Error Messages**: Provide helpful errors with examples
4. **Performance**: Cache compiled expressions
5. **Compatibility**: Test with real tool outputs

## Migration and Compatibility

- **Breaking Changes**: None - adds new optional features
- **Migration Path**: Existing workflows continue without filtering
- **Compatibility**: Works with any JSON input format
- **Rollback**: Can disable by omitting filter/sort configuration