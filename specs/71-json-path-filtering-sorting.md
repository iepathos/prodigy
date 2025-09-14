---
number: 71
title: JSON Path Filtering and Sorting for MapReduce
category: foundation
priority: high
status: draft
dependencies: [63]
created: 2025-01-14
---

# Specification 71: JSON Path Filtering and Sorting for MapReduce

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [63 - Conditional Execution]

## Context

The whitepaper shows sophisticated data filtering and sorting in MapReduce:
```yaml
map:
  input: "work-items.json"
  json_path: "$.items[*]"
  filter: "item.priority == 'high'"
  sort_by: "item.score DESC"
  max_items: 100
```

Currently, while JSON path extraction exists, the filtering and sorting capabilities are not implemented, limiting the ability to prioritize and focus on specific work items.

## Objective

Implement comprehensive JSON path filtering and sorting capabilities for MapReduce workflows, enabling sophisticated data selection, prioritization, and ordering of work items before processing.

## Requirements

### Functional Requirements
- JSONPath expression evaluation for data extraction
- Filter expressions on extracted items
- Multi-field sorting with direction control
- Support for complex filter conditions
- Numeric, string, and date comparisons
- Array and object field access
- Null-safe operations
- Limit and offset for pagination
- Filter/sort preview mode

### Non-Functional Requirements
- Efficient handling of large JSON files (100MB+)
- Streaming where possible to reduce memory
- Clear error messages for invalid expressions
- Performance metrics for filter/sort operations

## Acceptance Criteria

- [ ] `json_path: "$.items[*].data"` extracts nested data
- [ ] `filter: "score >= 80"` filters items by score
- [ ] `filter: "status == 'pending' && priority > 5"` complex filters
- [ ] `sort_by: "score DESC, name ASC"` multi-field sorting
- [ ] `max_items: 100` limits processed items
- [ ] `offset: 50` skips first 50 items
- [ ] Array access: `filter: "tags[0] == 'urgent'"`
- [ ] Null handling: `filter: "optional_field != null"`
- [ ] Date comparison: `filter: "created_at > '2024-01-01'"`
- [ ] Preview mode shows filtered/sorted items without execution

## Technical Details

### Implementation Approach

1. **Enhanced Data Pipeline Configuration**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct DataPipelineConfig {
       /// Input source (file or command)
       pub input: String,

       /// JSONPath expression for extraction
       pub json_path: String,

       /// Filter expression
       #[serde(skip_serializing_if = "Option::is_none")]
       pub filter: Option<FilterExpression>,

       /// Sort specification
       #[serde(skip_serializing_if = "Option::is_none")]
       pub sort_by: Option<SortSpec>,

       /// Maximum items to process
       #[serde(skip_serializing_if = "Option::is_none")]
       pub max_items: Option<usize>,

       /// Number of items to skip
       #[serde(skip_serializing_if = "Option::is_none")]
       pub offset: Option<usize>,

       /// Distinct field for deduplication
       #[serde(skip_serializing_if = "Option::is_none")]
       pub distinct: Option<String>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct FilterExpression {
       pub expression: String,
       #[serde(skip)]
       pub compiled: Option<CompiledFilter>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct SortSpec {
       pub fields: Vec<SortField>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct SortField {
       pub path: String,
       pub direction: SortDirection,
       pub null_handling: NullHandling,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(rename_all = "UPPERCASE")]
   pub enum SortDirection {
       Asc,
       Desc,
   }
   ```

2. **Filter Expression Evaluator**:
   ```rust
   pub struct FilterEvaluator {
       parser: FilterParser,
       value_extractor: ValueExtractor,
   }

   impl FilterEvaluator {
       pub fn compile(&self, expression: &str) -> Result<CompiledFilter> {
           let ast = self.parser.parse(expression)?;
           Ok(CompiledFilter { ast })
       }

       pub fn evaluate(&self, filter: &CompiledFilter, item: &Value) -> Result<bool> {
           self.evaluate_node(&filter.ast, item)
       }

       fn evaluate_node(&self, node: &FilterNode, item: &Value) -> Result<bool> {
           match node {
               FilterNode::Comparison { left, op, right } => {
                   let left_val = self.extract_value(left, item)?;
                   let right_val = self.extract_value(right, item)?;
                   Ok(self.compare_values(&left_val, op, &right_val)?)
               }
               FilterNode::Logical { left, op, right } => {
                   match op {
                       LogicalOp::And => {
                           Ok(self.evaluate_node(left, item)?
                              && self.evaluate_node(right, item)?)
                       }
                       LogicalOp::Or => {
                           Ok(self.evaluate_node(left, item)?
                              || self.evaluate_node(right, item)?)
                       }
                       LogicalOp::Not => {
                           Ok(!self.evaluate_node(left, item)?)
                       }
                   }
               }
               FilterNode::Path(path) => {
                   // Evaluate path as boolean
                   let val = self.value_extractor.extract(path, item)?;
                   Ok(self.to_boolean(&val))
               }
           }
       }

       fn extract_value(&self, expr: &Expression, item: &Value) -> Result<Value> {
           match expr {
               Expression::Path(path) => {
                   self.value_extractor.extract(path, item)
               }
               Expression::Literal(val) => Ok(val.clone()),
               Expression::Function { name, args } => {
                   self.evaluate_function(name, args, item)
               }
           }
       }

       fn compare_values(&self, left: &Value, op: &ComparisonOp, right: &Value) -> Result<bool> {
           match (left, right) {
               (Value::Number(l), Value::Number(r)) => {
                   let l = l.as_f64().unwrap_or(0.0);
                   let r = r.as_f64().unwrap_or(0.0);
                   Ok(match op {
                       ComparisonOp::Eq => (l - r).abs() < f64::EPSILON,
                       ComparisonOp::Ne => (l - r).abs() >= f64::EPSILON,
                       ComparisonOp::Lt => l < r,
                       ComparisonOp::Le => l <= r,
                       ComparisonOp::Gt => l > r,
                       ComparisonOp::Ge => l >= r,
                       ComparisonOp::In => false, // Not applicable
                       ComparisonOp::Contains => false,
                   })
               }
               (Value::String(l), Value::String(r)) => {
                   Ok(match op {
                       ComparisonOp::Eq => l == r,
                       ComparisonOp::Ne => l != r,
                       ComparisonOp::Lt => l < r,
                       ComparisonOp::Le => l <= r,
                       ComparisonOp::Gt => l > r,
                       ComparisonOp::Ge => l >= r,
                       ComparisonOp::Contains => l.contains(r.as_str()),
                       ComparisonOp::In => false,
                   })
               }
               (Value::Array(arr), val) if matches!(op, ComparisonOp::Contains) => {
                   Ok(arr.iter().any(|v| v == val))
               }
               _ => Ok(false),
           }
       }
   }
   ```

3. **Sorting Engine**:
   ```rust
   pub struct SortingEngine {
       value_extractor: ValueExtractor,
   }

   impl SortingEngine {
       pub fn sort(&self, items: &mut [Value], spec: &SortSpec) -> Result<()> {
           items.sort_by(|a, b| {
               for field in &spec.fields {
                   let a_val = self.value_extractor
                       .extract(&field.path, a)
                       .unwrap_or(Value::Null);
                   let b_val = self.value_extractor
                       .extract(&field.path, b)
                       .unwrap_or(Value::Null);

                   let ordering = self.compare_values(&a_val, &b_val, &field.null_handling);

                   let ordering = match field.direction {
                       SortDirection::Asc => ordering,
                       SortDirection::Desc => ordering.reverse(),
                   };

                   if ordering != Ordering::Equal {
                       return ordering;
                   }
               }
               Ordering::Equal
           });

           Ok(())
       }

       fn compare_values(&self, a: &Value, b: &Value, null_handling: &NullHandling) -> Ordering {
           match (a, b) {
               (Value::Null, Value::Null) => Ordering::Equal,
               (Value::Null, _) => match null_handling {
                   NullHandling::First => Ordering::Less,
                   NullHandling::Last => Ordering::Greater,
               },
               (_, Value::Null) => match null_handling {
                   NullHandling::First => Ordering::Greater,
                   NullHandling::Last => Ordering::Less,
               },
               (Value::Number(a), Value::Number(b)) => {
                   a.as_f64().partial_cmp(&b.as_f64()).unwrap_or(Ordering::Equal)
               }
               (Value::String(a), Value::String(b)) => a.cmp(b),
               (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
               _ => Ordering::Equal,
           }
       }
   }
   ```

4. **Data Pipeline Processor**:
   ```rust
   impl DataPipeline {
       pub async fn process(&self, config: &DataPipelineConfig) -> Result<Vec<Value>> {
           // Load data
           let data = self.load_input(&config.input).await?;

           // Extract items using JSONPath
           let mut items = self.extract_items(&data, &config.json_path)?;

           info!("Extracted {} items from input", items.len());

           // Apply filter
           if let Some(filter) = &config.filter {
               let evaluator = FilterEvaluator::new();
               let compiled = evaluator.compile(&filter.expression)?;

               items = items.into_iter()
                   .filter(|item| {
                       evaluator.evaluate(&compiled, item)
                           .unwrap_or(false)
                   })
                   .collect();

               info!("Filtered to {} items", items.len());
           }

           // Apply sorting
           if let Some(sort_spec) = &config.sort_by {
               let engine = SortingEngine::new();
               engine.sort(&mut items, sort_spec)?;
               info!("Sorted {} items", items.len());
           }

           // Apply distinct
           if let Some(distinct_field) = &config.distinct {
               items = self.deduplicate(items, distinct_field)?;
               info!("Deduplicated to {} items", items.len());
           }

           // Apply offset
           if let Some(offset) = config.offset {
               items = items.into_iter().skip(offset).collect();
           }

           // Apply limit
           if let Some(max_items) = config.max_items {
               items.truncate(max_items);
               info!("Limited to {} items", items.len());
           }

           Ok(items)
       }
   }
   ```

### Architecture Changes
- Add `FilterEvaluator` component
- Add `SortingEngine` component
- Enhance `DataPipeline` with filter/sort
- Add expression parser
- Integrate with MapReduce executor

### Data Structures
```yaml
# Example MapReduce with filtering and sorting
map:
  input: "analysis.json"
  json_path: "$.items[*]"
  filter: "score >= 75 && status != 'completed'"
  sort_by: "priority DESC, score DESC, name ASC"
  max_items: 50
  offset: 0

  agent_template:
    commands:
      - claude: "/process-item '${item}'"

# Complex filtering example
map:
  input: "codebase-analysis.json"
  json_path: "$.files[*]"
  filter: |
    complexity > 10 &&
    lines_of_code > 100 &&
    (language == 'python' || language == 'javascript') &&
    last_modified > '2024-01-01'
  sort_by: "complexity DESC, lines_of_code DESC"
  distinct: "path"  # Deduplicate by file path
```

## Dependencies

- **Prerequisites**: [63 - Conditional Execution] for expression evaluation
- **Affected Components**:
  - `src/cook/execution/data_pipeline.rs` - Core pipeline
  - `src/cook/execution/mapreduce.rs` - Integration
  - `src/config/mapreduce.rs` - Configuration
- **External Dependencies**: `jsonpath_lib` for JSONPath

## Testing Strategy

- **Unit Tests**:
  - Filter expression parsing
  - Value comparison logic
  - Sorting algorithms
  - JSONPath extraction
- **Integration Tests**:
  - End-to-end pipeline processing
  - Complex filter expressions
  - Multi-field sorting
  - Large dataset handling
- **Performance Tests**:
  - Large JSON file processing
  - Complex filter performance
  - Memory usage monitoring
  - Streaming vs loading comparison

## Documentation Requirements

- **Code Documentation**: Document expression syntax
- **User Documentation**:
  - Filter expression guide
  - Sorting configuration
  - JSONPath syntax reference
  - Performance tuning tips
- **Architecture Updates**: Add data pipeline to flow

## Implementation Notes

- Support streaming for large files
- Cache compiled expressions
- Consider indexing for repeated queries
- Validate expressions at parse time
- Future: GraphQL-style queries

## Migration and Compatibility

- Existing MapReduce workflows continue to work
- Filtering/sorting are optional additions
- Clear error messages for invalid expressions
- Examples for common patterns