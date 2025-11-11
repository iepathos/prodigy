# Work Distribution

Prodigy's MapReduce framework provides intelligent work item management and distribution for massive-scale parallel processing. Work items can be extracted from JSON files, filtered, sorted, paginated, and deduplicated.

## Overview

Work distribution features:
- **JSONPath extraction** - Extract items from complex JSON structures
- **Filtering** - Boolean expressions to select items
- **Sorting** - Order items by field values
- **Pagination** - Offset and limit for chunked processing
- **Deduplication** - Remove duplicates by field

## Input Sources

Work items can come from JSON files or command outputs:

```yaml
# Source: src/config/mapreduce.rs
map:
  input: "items.json"          # JSON file
  json_path: "$.items[*]"      # JSONPath to extract items
```

Or from command output:

```yaml
# Source: workflows/debtmap-reduce.yml
setup:
  - shell: "generate-items.sh"
    capture:
      output: items
      format: json

map:
  input: "${items}"
  json_path: "$[*]"
```

## JSONPath Extraction

Use JSONPath expressions to extract work items from complex structures:

```yaml
# Extract all items from nested structure
json_path: "$.data.items[*]"

# Extract specific fields
json_path: "$.users[*].{name: name, email: email}"

# Filter at extraction time
json_path: "$.items[?(@.score >= 5)]"
```

## Filtering Work Items

Filter items using boolean expressions:

```yaml
# Source: src/cook/execution/data_pipeline/filter.rs
map:
  input: "items.json"
  json_path: "$.items[*]"
  filter: "item.score >= 5 && item.status == 'pending'"
```

### Filter Expressions

Supported operators:
- `==`, `!=` - Equality
- `<`, `<=`, `>`, `>=` - Comparison
- `&&`, `||` - Logical AND/OR
- `!` - Logical NOT

Filter by multiple conditions:

```yaml
filter: "item.priority == 'high' || (item.score >= 8 && item.category == 'critical')"
```

## Sorting Work Items

Process items in a specific order:

```yaml
# Source: src/cook/execution/data_pipeline/sorter.rs
map:
  input: "items.json"
  json_path: "$.items[*]"
  sort_by: "item.priority DESC, item.score ASC"
```

Sort options:
- `ASC` - Ascending order
- `DESC` - Descending order
- Multiple fields separated by commas

## Pagination

Process large datasets in chunks:

```yaml
# Source: src/cook/execution/data_pipeline/mod.rs:177-191
map:
  input: "items.json"
  json_path: "$.items[*]"
  offset: 0        # Skip first N items
  max_items: 100   # Process up to N items
```

Use pagination for:
- Incremental processing
- Testing workflows on subsets
- Resource-constrained environments

## Deduplication

Remove duplicate items by field:

```yaml
# Source: src/config/mapreduce.rs:261
map:
  input: "items.json"
  json_path: "$.items[*]"
  distinct: "item.id"
```

Deduplication uses the first occurrence of each unique value.

!!! note "Null Value Handling"
    Items with null values for the distinct field are treated as having a 'null' key and are deduplicated together. This means only the first item with a null value will be kept.

## Processing Pipeline Order

When multiple work distribution features are used together, they are applied in a specific order:

1. **JSONPath extraction** - Extract items from input JSON
2. **Filtering** - Apply filter expressions to select items
3. **Sorting** - Order items by specified fields
4. **Deduplication** - Remove duplicates by distinct field
5. **Offset** - Skip first N items
6. **Limit** (max_items) - Truncate to maximum count

This order ensures predictable behavior when combining features.

!!! tip "Filter Expression Evaluation"
    When evaluating filter expressions, accessing non-existent fields returns false. This means items without the specified field will be filtered out. Always ensure your items have the expected structure, or use functions like `is_null()` to handle missing fields explicitly.

## Combining Distribution Features

All features can be combined for complex work distribution:

```yaml
# Source: workflows/debtmap-reduce.yml:79-81
map:
  input: "tasks.json"
  json_path: "$.tasks[*]"
  filter: "item.status == 'pending' && item.score >= 5"
  sort_by: "item.priority DESC"
  distinct: "item.task_id"
  offset: 0
  max_items: 50
  max_parallel: 10
```

This configuration:
1. Extracts tasks from JSON
2. Filters to pending tasks with score >= 5
3. Sorts by priority (highest first)
4. Removes duplicates by task ID
5. Processes first 50 items
6. Runs 10 agents in parallel

## Work Item Structure

Work items are available to agent templates as `${item}`:

```yaml
agent_template:
  - claude: "/process '${item.name}'"
  - shell: "echo Processing ${item.id}: ${item.description}"
```

Access nested fields:

```yaml
- claude: "/analyze ${item.metadata.category}"
- shell: "test -f ${item.config.path}"
```

## See Also

- [MapReduce Overview](overview.md) - MapReduce concepts and phases
- [Variables and Interpolation](../workflow-basics/variables.md) - Variable syntax
- [Checkpoint and Resume](checkpoint-resume.md) - Resume interrupted work
