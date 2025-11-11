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
map:
  input: "items.json"          # JSON file
  json_path: "$.items[*]"      # JSONPath to extract items
```

Or from command output:

```yaml
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
map:
  input: "items.json"
  json_path: "$.items[*]"
  deduplicate_by: "item.id"
```

Deduplication uses the first occurrence of each unique value.

## Combining Distribution Features

All features can be combined for complex work distribution:

```yaml
map:
  input: "tasks.json"
  json_path: "$.tasks[*]"
  filter: "item.status == 'pending' && item.score >= 5"
  sort_by: "item.priority DESC"
  deduplicate_by: "item.task_id"
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
