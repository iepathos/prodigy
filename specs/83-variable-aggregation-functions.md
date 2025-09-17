---
number: 83
title: Variable Aggregation Functions
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-09-17
---

# Specification 83: Variable Aggregation Functions

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

Variable aggregation functions are essential for MapReduce workflows, particularly in the reduce phase where results from multiple map operations need to be combined. Currently, the implementation in `src/cook/execution/variables.rs` (line 577) returns placeholder values for all aggregate functions, making the reduce phase of MapReduce workflows completely non-functional.

Without working aggregation functions, users cannot:
- Calculate sums, averages, or counts from map results
- Find minimum or maximum values across distributed computations
- Concatenate or merge results from parallel operations
- Perform statistical analysis on distributed data
- Generate summary reports from MapReduce outputs

This effectively breaks the entire MapReduce paradigm, as the reduce phase cannot perform its intended data aggregation role.

## Objective

Implement all variable aggregation functions to enable proper data aggregation in MapReduce reduce phases and other workflow contexts requiring data summarization.

## Requirements

### Functional Requirements

1. **Core Aggregation Functions**
   - `sum()`: Calculate sum of numeric values
   - `count()`: Count number of items
   - `avg()`/`average()`: Calculate arithmetic mean
   - `min()`: Find minimum value
   - `max()`: Find maximum value
   - `concat()`: Concatenate strings or arrays
   - `merge()`: Merge objects/maps

2. **Statistical Functions**
   - `median()`: Calculate median value
   - `stddev()`: Calculate standard deviation
   - `variance()`: Calculate variance
   - `percentile(n)`: Calculate nth percentile

3. **Collection Functions**
   - `unique()`: Get unique values
   - `flatten()`: Flatten nested arrays
   - `group_by(key)`: Group items by key
   - `sort()`: Sort values
   - `reverse()`: Reverse order

4. **Type Handling**
   - Automatic type coercion where appropriate
   - Clear error messages for type mismatches
   - Support for mixed numeric types
   - Handle null/undefined values gracefully

### Non-Functional Requirements

1. **Performance**
   - Efficient algorithms for large datasets
   - Stream processing for memory efficiency
   - Lazy evaluation where possible

2. **Accuracy**
   - Numerical stability for floating-point operations
   - Proper handling of edge cases (empty sets, single values)
   - Consistent rounding behavior

3. **Extensibility**
   - Plugin architecture for custom aggregations
   - Composable function design
   - Clear interfaces for extensions

## Acceptance Criteria

- [ ] All core aggregation functions return correct results
- [ ] Statistical functions calculate accurate values
- [ ] Collection functions properly manipulate data structures
- [ ] Type coercion works as expected
- [ ] Null/undefined values are handled gracefully
- [ ] Large datasets (10,000+ items) are processed efficiently
- [ ] Memory usage remains bounded for streaming operations
- [ ] All existing variable tests pass
- [ ] New comprehensive tests for each aggregation function
- [ ] Performance benchmarks meet targets (<100ms for 10k items)

## Technical Details

### Implementation Approach

1. **Replace Placeholder Implementation**
   ```rust
   // Current (line 577)
   // For now, return a placeholder

   // New implementation
   pub fn evaluate_aggregate(
       &self,
       function: &str,
       values: &[Value],
   ) -> Result<Value> {
       match function {
           "sum" => self.calculate_sum(values),
           "count" => Ok(Value::Number(values.len() as f64)),
           "avg" | "average" => self.calculate_average(values),
           "min" => self.find_minimum(values),
           "max" => self.find_maximum(values),
           "concat" => self.concatenate_values(values),
           "merge" => self.merge_objects(values),
           "median" => self.calculate_median(values),
           "unique" => self.get_unique_values(values),
           _ => Err(anyhow!("Unknown aggregate function: {}", function)),
       }
   }
   ```

2. **Implement Core Functions**
   ```rust
   fn calculate_sum(&self, values: &[Value]) -> Result<Value> {
       let sum = values.iter()
           .filter_map(|v| v.as_number())
           .sum::<f64>();
       Ok(Value::Number(sum))
   }

   fn calculate_average(&self, values: &[Value]) -> Result<Value> {
       let numbers: Vec<f64> = values.iter()
           .filter_map(|v| v.as_number())
           .collect();

       if numbers.is_empty() {
           return Ok(Value::Null);
       }

       let avg = numbers.iter().sum::<f64>() / numbers.len() as f64;
       Ok(Value::Number(avg))
   }

   fn concatenate_values(&self, values: &[Value]) -> Result<Value> {
       let strings: Vec<String> = values.iter()
           .map(|v| v.to_string())
           .collect();
       Ok(Value::String(strings.join("")))
   }
   ```

3. **Streaming Implementation for Large Datasets**
   ```rust
   pub struct StreamingAggregator {
       function: AggregateFunction,
       state: AggregateState,
   }

   impl StreamingAggregator {
       pub fn update(&mut self, value: &Value) {
           match self.function {
               AggregateFunction::Sum => {
                   if let Some(n) = value.as_number() {
                       self.state.sum += n;
                       self.state.count += 1;
                   }
               }
               // ... other functions
           }
       }

       pub fn finalize(&self) -> Value {
           match self.function {
               AggregateFunction::Average => {
                   Value::Number(self.state.sum / self.state.count as f64)
               }
               // ... other functions
           }
       }
   }
   ```

### Architecture Changes

- Add `AggregateFunction` enum for type-safe function selection
- Implement streaming aggregator for memory efficiency
- Add aggregate function registry for extensibility
- Integrate with expression evaluator for complex expressions

### Data Structures

```rust
pub enum AggregateFunction {
    Sum,
    Count,
    Average,
    Min,
    Max,
    Concat,
    Merge,
    Median,
    StdDev,
    Variance,
    Percentile(f64),
    Unique,
    GroupBy(String),
}

pub struct AggregateState {
    pub sum: f64,
    pub count: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub values: Vec<Value>, // For functions requiring all values
}

pub struct AggregateResult {
    pub value: Value,
    pub metadata: AggregateMetadata,
}

pub struct AggregateMetadata {
    pub items_processed: usize,
    pub nulls_skipped: usize,
    pub type_mismatches: usize,
}
```

### APIs and Interfaces

```rust
pub trait AggregateEvaluator {
    fn evaluate(
        &self,
        function: &str,
        values: &[Value],
        options: &AggregateOptions,
    ) -> Result<AggregateResult>;

    fn evaluate_streaming(
        &self,
        function: &str,
        values: impl Iterator<Item = Value>,
    ) -> Result<AggregateResult>;

    fn register_custom(
        &mut self,
        name: &str,
        function: Box<dyn CustomAggregate>,
    ) -> Result<()>;
}

pub trait CustomAggregate {
    fn aggregate(&self, values: &[Value]) -> Result<Value>;
    fn supports_streaming(&self) -> bool;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - Variable evaluation system
  - MapReduce reduce phase
  - Expression evaluator
  - Workflow executor
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test each aggregation function with various inputs
  - Edge cases (empty arrays, single values, nulls)
  - Type coercion scenarios
  - Large dataset handling

- **Integration Tests**:
  - MapReduce workflows using aggregations
  - Complex expressions with multiple aggregations
  - Performance tests with large datasets

- **Property-Based Tests**:
  - Commutative and associative properties
  - Numerical stability tests
  - Consistency across different input orders

- **User Acceptance**:
  - Real-world MapReduce scenarios
  - Statistical analysis workflows
  - Data summarization use cases

## Documentation Requirements

- **Code Documentation**:
  - Document each aggregation function
  - Include examples and edge cases
  - Performance characteristics

- **User Documentation**:
  - Aggregation function reference
  - Usage in MapReduce workflows
  - Best practices and patterns
  - Performance optimization guide

- **Architecture Updates**:
  - Update ARCHITECTURE.md with aggregation system
  - Document streaming architecture
  - Include extension points

## Implementation Notes

- Start with core functions (sum, count, avg) before statistical functions
- Ensure numerical stability for floating-point operations
- Consider using established statistical libraries for complex functions
- Implement streaming versions for memory efficiency
- Add detailed error messages for debugging
- Consider adding SQL-like aggregate syntax support
- Plan for future support of windowed aggregations

## Migration and Compatibility

- No breaking changes to existing variable system
- Graceful handling of workflows using placeholder aggregations
- Consider deprecation path for old syntax if needed
- Document migration guide for existing workflows