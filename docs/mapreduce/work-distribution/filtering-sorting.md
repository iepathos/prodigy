# Filtering & Sorting

Filter, sort, and deduplicate work items to control which items are processed and in what order.

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
```

1. Single `=` also works for equality checks
2. Inclusive comparison - items with priority of 5 will be included

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
```

1. Word-based `AND` operator is also supported
2. Word-based `OR` operator is also supported

**Nested field access:**
```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:298-300
filter: "unified_score.final_score >= 5"
filter: "location.coordinates.lat > 40.0"
```

**Array index access:**
```yaml
# Source: src/cook/execution/data_pipeline/filter.rs:451-461
filter: "tags[0] == 'important'"         # First element of tags array
filter: "results[2].score > 80"          # Third result's score
filter: "matrix[0][1] == 42"             # Nested array access
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

## Processing Order

Filtering, sorting, and deduplication are part of a larger data processing pipeline. The complete order is:

1. JSONPath Extraction → 2. **Filtering** → 3. **Sorting** → 4. **Deduplication** → 5. Offset → 6. Limit

See [Pagination](pagination.md#processing-pipeline-order) for the full pipeline details and examples showing how these operations combine.

## Related Topics

- [Pagination](pagination.md) - Control output size with offset and limit
- [Input Sources](input-sources.md) - Define where work items come from
- [Examples](examples.md) - Complete workflow examples with filtering and sorting
