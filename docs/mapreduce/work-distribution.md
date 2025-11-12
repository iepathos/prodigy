# Work Distribution

Work distribution is the process of extracting, filtering, sorting, and distributing work items to parallel agents in MapReduce workflows. The data pipeline provides powerful capabilities for selecting and organizing work items from various input sources.

## Overview

The work distribution system processes data through a multi-stage pipeline:

1. **Input Source** - Load data from JSON files or command output
2. **JSONPath Extraction** - Extract work items from nested structures
3. **Filtering** - Select items matching criteria
4. **Sorting** - Order items by priority or other fields
5. **Deduplication** - Remove duplicate items
6. **Pagination** - Apply offset and limit for testing or batching

Each stage is optional and can be configured independently to build the exact work distribution strategy you need.

```mermaid
flowchart LR
    Input[Input Source<br/>JSON file or command] --> JSONPath[JSONPath Extraction<br/>$.items[*]]
    JSONPath --> Filter[Filtering<br/>score >= 5]
    Filter --> Sort[Sorting<br/>priority DESC]
    Sort --> Dedup[Deduplication<br/>distinct: id]
    Dedup --> Offset[Offset<br/>skip first N]
    Offset --> Limit[Limit<br/>take M items]
    Limit --> Agents[Distribute to<br/>Parallel Agents]

    style Input fill:#e1f5ff
    style JSONPath fill:#fff3e0
    style Filter fill:#f3e5f5
    style Sort fill:#e8f5e9
    style Dedup fill:#fff3e0
    style Offset fill:#f3e5f5
    style Limit fill:#e1f5ff
    style Agents fill:#ffebee
```

**Figure**: Work distribution pipeline showing data flow from input source through transformation stages to parallel agents.

## Input Sources

Work items can be loaded from two types of input sources:

### JSON Files

The most common approach is to use a JSON file containing work items. Specify the file path using the `input` field in the map phase:

```yaml
# Source: workflows/mapreduce-example.yml
map:
  input: debt_items.json
  json_path: "$.debt_items[*]"
```

### Command Output

You can also generate work items dynamically using a command:

```yaml
# Source: workflows/documentation-drift-mapreduce.yml
setup:
  - shell: |
      cat > .prodigy/doc-areas.json << 'EOF'
      {
        "areas": [
          {"name": "README", "pattern": "README.md", "priority": "high"}
        ]
      }
      EOF

map:
  input: .prodigy/doc-areas.json
  json_path: "$.areas[*]"
```

!!! tip
    Generate work items in the setup phase and save them to a JSON file. This allows you to preview the items before processing and ensures consistent inputs if you need to resume the workflow.

## JSONPath Extraction

JSONPath expressions let you extract work items from complex nested JSON structures. Use the `json_path` field to specify an extraction pattern:

```yaml
# Source: workflows/mapreduce-example.yml
map:
  input: data.json
  json_path: "$.items[*]"
```

### Common JSONPath Patterns

!!! example "Common JSONPath Patterns"

    **Extract all array elements:**
    ```yaml
    json_path: "$.items[*]"
    ```

    **Extract from nested structure:**
    ```yaml
    json_path: "$.data.results[*]"
    ```

    **Extract specific field from each element:**
    ```yaml
    json_path: "$.items[*].name"
    ```

### How JSONPath Works

The JSONPath expression is applied to the input data and returns an array of matching items. Each item becomes a work item processed by an agent.

```json
{
  "items": [
    {"id": 1, "priority": 5},
    {"id": 2, "priority": 3}
  ]
}
```

With `json_path: "$.items[*]"`, two work items are created, one for each array element.

!!! note
    If no JSONPath is specified, the entire input is treated as either an array (if it's a JSON array) or a single work item (for other JSON types).

## Filtering

Filters let you selectively process work items based on boolean expressions. Use the `filter` field to specify selection criteria:

```yaml
# Source: workflows/mapreduce-example.yml
map:
  filter: "severity == 'high' || severity == 'critical'"
```

### Filter Syntax

Filters support comparison operators, logical operators, and nested field access:

**Comparison operators:**
```yaml
# Equality
filter: "status == 'active'"
filter: "status = 'active'"  # (1)!

# Inequality
filter: "status != 'archived'"

# Numeric comparison
filter: "priority > 5"
filter: "priority >= 5"  # (2)!
filter: "priority < 10"
filter: "priority <= 10"

1. Single `=` also works for equality checks
2. Inclusive comparison - items with priority of 5 will be included
```

**Logical operators:**
```yaml
# AND
filter: "severity == 'high' && priority > 5"
filter: "severity == 'high' AND priority > 5"  # (1)!

# OR
filter: "severity == 'high' || severity == 'critical'"
filter: "severity == 'high' OR severity == 'critical'"  # (2)!

# NOT
filter: "!(status == 'archived')"
filter: "!is_null(optional_field)"

1. Word-based `AND` operator is also supported
2. Word-based `OR` operator is also supported
```

**Nested field access:**
```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:298-300
filter: "unified_score.final_score >= 5"
filter: "location.coordinates.lat > 40.0"
```

**IN operator:**
```yaml
filter: "severity in ['high', 'critical']"
filter: "status in ['active', 'pending']"
```

### Filter Functions

Advanced filtering with built-in functions:

```yaml
# Null checks
filter: "is_null(optional_field)"
filter: "is_not_null(required_field)"

# Type checks
filter: "is_number(score)"
filter: "is_string(name)"
filter: "is_bool(active)"
filter: "is_array(tags)"
filter: "is_object(metadata)"

# String operations
filter: "contains(name, 'test')"
filter: "starts_with(path, '/usr')"
filter: "ends_with(filename, '.rs')"

# Length checks
filter: "length(tags) == 3"

# Regex matching
filter: "matches(email, '^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$')"
```

### Complex Filter Examples

**Combine multiple conditions:**
```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:793-795
filter: "unified_score.final_score >= 5 && debt_type.category == 'complexity'"
```

**Filter with type safety:**
```yaml
filter: "is_number(score) && score > 50"
```

**Pattern matching on file paths:**
```yaml
filter: "matches(path, '\\.rs$')"  # Only Rust files
```

!!! warning
    When a field doesn't exist, most comparisons evaluate to `false`. Use `is_null()` or `is_not_null()` functions for explicit null checks if field presence is important.

## Sorting

Sort work items to control processing order. Use the `sort_by` field to specify one or more sort fields:

```yaml
# Source: workflows/mapreduce-example.yml
map:
  sort_by: "priority DESC"
```

### Sort Syntax

**Single field ascending:**
```yaml
sort_by: "created_at ASC"
```

**Single field descending:**
```yaml
sort_by: "priority DESC"
```

**Multiple fields:**
```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:1498
sort_by: "severity DESC, priority ASC"
```

**Nested fields:**
```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:1559
sort_by: "unified_score.final_score DESC"
```

### Null Handling

By default, null values are sorted last regardless of sort direction (NULLS LAST). You can control this behavior explicitly:

```yaml
# Nulls come last (default)
sort_by: "score DESC NULLS LAST"

# Nulls come first
sort_by: "score DESC NULLS FIRST"
```

**Example with mixed types:**
```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:1523-1526
sort_by: "File.score DESC NULLS LAST, Function.unified_score.final_score DESC NULLS LAST"
```

This sorts items where `File.score` exists first (by score descending), then items where `Function.unified_score.final_score` exists (by score descending). Items missing both fields come last.

!!! tip
    Use `NULLS LAST` (default) to prioritize items with values. Use `NULLS FIRST` when you want to handle missing data items first.

## Pagination

Control the number of items processed using offset and limit:

### Limit (max_items)

Limit the total number of work items:

```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:298-300
map:
  json_path: "$.items[*]"
  filter: "unified_score.final_score >= 5"
  sort_by: "unified_score.final_score DESC"
  max_items: 3  # Process only top 3 items
```

### Offset

Skip the first N items:

```yaml
# Source: src/config/mapreduce.rs:84-91
map:
  json_path: "$.items[*]"
  offset: 10      # Skip first 10 items
  max_items: 20   # Then take next 20
```

### Use Cases

!!! tip "Testing Workflows First"
    Always test MapReduce workflows with a small subset before running on the full dataset:
    ```yaml
    map:
      max_items: 5  # Process only 5 items during development
    ```

**Batched processing:**
```yaml
# Batch 1: items 0-99
map:
  offset: 0
  max_items: 100

# Batch 2: items 100-199
map:
  offset: 100
  max_items: 100
```

**Top-N processing:**
```yaml
# Process only the 10 highest priority items
map:
  sort_by: "priority DESC"
  max_items: 10
```

## Deduplication

Remove duplicate work items based on a field value using the `distinct` field:

```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:361-363
map:
  json_path: "$.items[*]"
  distinct: "id"  # Keep only first occurrence of each unique ID
```

### How Deduplication Works

The deduplication process:

1. Extracts the field value from each item
2. Serializes the value to a string for comparison
3. Keeps only the first item with each unique value
4. Discards subsequent items with the same value

**Example:**
```json
[
  {"id": 1, "value": "a"},
  {"id": 2, "value": "b"},
  {"id": 1, "value": "c"},  // Duplicate id=1, discarded
  {"id": 3, "value": "d"}
]
```

With `distinct: "id"`, only 3 items remain (ids: 1, 2, 3).

### Nested Field Deduplication

You can deduplicate based on nested fields:

```yaml
distinct: "location.file"  # Unique by file path
distinct: "user.email"     # Unique by email address
```

### Null Value Handling

Items with null values in the distinct field are treated as having the value `"null"`. This means:
- Only one item with a null distinct value will be kept
- Items with explicit `null` and missing fields are treated the same

!!! note
    The correct field name is `distinct`, not `deduplicate_by`. The deduplication happens after filtering and sorting but before offset and limit.

## Processing Pipeline Order

Understanding the order of operations is important for building effective work distribution strategies:

1. **JSONPath Extraction** - Extract items from input source
2. **Filtering** - Apply filter expression to select items
3. **Sorting** - Order items by sort fields
4. **Deduplication** - Remove duplicates based on distinct field
5. **Offset** - Skip first N items
6. **Limit (max_items)** - Take only first N remaining items

!!! note "Optimization Tip"
    Place expensive filtering early in the pipeline to reduce the number of items for subsequent operations. Sort only after filtering to minimize sort cost.

```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:127-201
map:
  input: data.json
  json_path: "$.items[*]"           # (1)!
  filter: "score >= 50"             # (2)!
  sort_by: "score DESC"             # (3)!
  distinct: "category"              # (4)!
  offset: 5                         # (5)!
  max_items: 10                     # (6)!

1. Extract all items from `$.items[*]` array
2. Keep only items where `score >= 50`
3. Sort remaining items by score (highest first)
4. Remove duplicates by category (keeps first of each)
5. Skip the first 5 items
6. Take the next 10 items for processing
```

This pipeline demonstrates the complete data transformation flow from extraction to final work item distribution.

## Complete Examples

### High-Priority Debt Items

Process technical debt items with high scores, sorted by priority:

```yaml
# Source: workflows/mapreduce-example.yml
name: parallel-debt-elimination
mode: mapreduce

setup:
  - shell: "debtmap analyze . --output debt_items.json"  # (1)!

map:
  input: debt_items.json  # (2)!
  json_path: "$.debt_items[*]"  # (3)!
  filter: "severity == 'high' || severity == 'critical'"  # (4)!
  sort_by: "priority DESC"  # (5)!
  max_parallel: 10  # (6)!

  agent_template:
    - claude: "/fix-issue ${item.description}"

1. Generate work items in setup phase - ensures reproducible input
2. Use JSON file output from setup phase
3. Extract debt items from the array
4. Process only high and critical severity items
5. Process highest priority items first
6. Run up to 10 agents concurrently
```

### Top Scoring Items with Deduplication

Process the top 3 unique high-scoring items:

```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:294-355
map:
  input: analysis.json
  json_path: "$.items[*]"  # (1)!
  filter: "unified_score.final_score >= 5"  # (2)!
  sort_by: "unified_score.final_score DESC"  # (3)!
  distinct: "location.file"  # (4)!
  max_items: 3  # (5)!

1. Extract all items from analysis results
2. Only process items with score >= 5
3. Sort by score, highest first
4. Keep only one item per file (deduplication)
5. Process only the top 3 unique items
```

### Documentation Areas by Priority

Process high-priority documentation areas first:

```yaml
# Source: workflows/documentation-drift-mapreduce.yml
setup:
  - shell: |
      cat > .prodigy/doc-areas.json << 'EOF'
      {
        "areas": [
          {"name": "README", "priority": 1},
          {"name": "API", "priority": 2},
          {"name": "Examples", "priority": 3}
        ]
      }
      EOF

map:
  input: .prodigy/doc-areas.json
  json_path: "$.areas[*]"
  sort_by: "priority ASC"  # Process priority 1 first
  max_parallel: 4
```

### Batched Processing with Filters

Process work items in batches with filtering:

```yaml
map:
  input: large-dataset.json
  json_path: "$.tasks[*]"
  filter: "status == 'pending' && assigned_to == null"
  sort_by: "created_at ASC"
  offset: 0       # Start from beginning
  max_items: 50   # Process 50 at a time
  max_parallel: 10
```

## Integration with Map Phase

All work distribution fields are configured within the `map` phase configuration block:

```yaml
# Source: src/config/mapreduce.rs:49
map:
  # Input source
  input: <path-to-json-file>  # (1)!

  # Work distribution pipeline
  json_path: <jsonpath-expression>  # (2)!
  filter: <filter-expression>  # (3)!
  sort_by: <sort-specification>  # (4)!
  distinct: <field-name>  # (5)!
  offset: <number>  # (6)!
  max_items: <number>  # (7)!

  # Parallelization
  max_parallel: <number>  # (8)!

  # Agent template
  agent_template:
    - claude: "/process-item ${item}"

1. Path to JSON file containing work items
2. JSONPath expression to extract items (e.g., `$.items[*]`)
3. Filter expression to select items (e.g., `score >= 5`)
4. Sort specification (e.g., `priority DESC`)
5. Field name for deduplication (e.g., `id`)
6. Number of items to skip from the start
7. Maximum number of items to process
8. Number of concurrent agents to run
```

These fields work together to control how work items are selected and distributed to parallel agents.

## Troubleshooting

### Common Issues

**JSONPath returns no items:**
- Verify the input JSON structure matches your path
- Test JSONPath expressions using online tools
- Check for typos in field names

**Filter excludes all items:**
- Test filter expressions on sample data
- Check for correct field names and types
- Verify nested field paths are accurate

**Sorting doesn't work as expected:**
- Ensure sort field exists in all items
- Use `NULLS LAST` to handle missing values
- Check field types (strings sort alphabetically, numbers numerically)

**Deduplication removes too many items:**
- Verify the distinct field has the granularity you expect
- Remember that null values are treated as identical
- Check if nested field paths are correct

### Debugging Tips

**Preview filtered items:**
```yaml
setup:
  - shell: |
      jq '.items[] | select(.score >= 5)' data.json > filtered-preview.json
```

**Count items at each stage:**
```yaml
setup:
  - shell: |
      echo "Total items: $(jq '.items | length' data.json)"
      echo "After filter: $(jq '[.items[] | select(.score >= 5)] | length' data.json)"
```

**Validate JSONPath:**
```yaml
setup:
  - shell: |
      jq '$.items[*]' data.json | jq 'length'
```

## See Also

- [MapReduce Overview](index.md) - Introduction to MapReduce workflows
- [Setup Phase (Advanced)](setup-phase-advanced.md) - Generating work items dynamically
- [Checkpoint and Resume](checkpoint-and-resume.md) - Resuming interrupted workflows
- [Dead Letter Queue (DLQ)](dead-letter-queue-dlq.md) - Handling failed work items
