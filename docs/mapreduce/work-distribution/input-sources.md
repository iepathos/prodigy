# Input Sources

Work items can be loaded from two types of input sources and extracted using JSONPath expressions.

!!! info "Automatic Input Detection"
    Prodigy automatically detects the input type based on file existence:

    1. If the path points to an existing `.json` file → treated as JSON file input
    2. If the path points to any other existing file → also treated as JSON file input
    3. Otherwise → treated as a command to execute

    This allows flexibility in file naming while defaulting to command execution for non-existent paths.

## JSON Files

The most common approach is to use a JSON file containing work items. Specify the file path using the `input` field in the map phase:

```yaml
# Source: workflows/mapreduce-example.yml
map:
  input: debt_items.json
  json_path: "$.debt_items[*]"
```

!!! note "File Extension Flexibility"
    While `.json` files are preferred, any readable file with valid JSON content works as an input source. Prodigy checks for file existence first, then parses the content as JSON regardless of extension.

## Command Output

You can also generate work items dynamically using a command. When the input doesn't resolve to an existing file, Prodigy executes it as a shell command.

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

### Command Output Parsing

<!-- Source: src/cook/execution/input_source.rs:94-127 -->

When using command output as input, Prodigy parses the output using this logic:

1. **Full JSON array** - If the entire output is a valid JSON array, it's returned as-is
2. **Single JSON value** - If the output is a single JSON object or value, it's wrapped in an array
3. **Line-based parsing** - If output isn't valid JSON, each line is processed individually:
     - Lines that parse as JSON become work items directly
     - Non-JSON lines become `{"item": "line content"}`

```yaml
# Example: Command producing JSON array
map:
  input: "cat items.json"  # If items.json doesn't exist, runs as command
  json_path: "$[*]"

# Example: Command producing line-based output
map:
  input: "find . -name '*.rs' -type f"
  # Each line becomes: {"item": "path/to/file.rs"}
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

### Recursive Descent

<!-- Source: src/cook/execution/data_pipeline/json_path.rs:46-51, 172-191 -->

The recursive descent operator (`..`) searches through all nested levels to find matching fields. This is useful when you need to find values regardless of where they appear in the structure.

```yaml
# Find all "name" fields at any depth
json_path: "$..name"
```

!!! example "Recursive Descent Example"

    Given this nested structure:

    ```json
    {
      "project": {
        "name": "main",
        "modules": [
          {"name": "core", "files": [{"name": "lib.rs"}]},
          {"name": "utils", "files": [{"name": "helpers.rs"}]}
        ]
      }
    }
    ```

    With `json_path: "$..name"`, you get: `["main", "core", "lib.rs", "utils", "helpers.rs"]`

### Filter Expressions

<!-- Source: src/cook/execution/data_pipeline/json_path.rs:62-68, 194-275 -->

Filter expressions allow conditional selection of array elements. Use `[?(@.field operator value)]` syntax to filter items based on field values.

**Supported operators:**

| Operator | Description |
|----------|-------------|
| `==` or `=` | Equal to (strings or numbers) |
| `!=` | Not equal to |
| `>` | Greater than (numbers) |
| `<` | Less than (numbers) |
| `>=` | Greater than or equal |
| `<=` | Less than or equal |

```yaml
# Select items where priority is greater than 5
json_path: "$.items[?(@.priority > 5)]"

# Select items where status equals "pending"
json_path: "$.tasks[?(@.status == 'pending')]"

# Select items where count is less than or equal to 10
json_path: "$.data[?(@.count <= 10)]"
```

!!! example "Filter Expression Example"

    Given this input:

    ```json
    {
      "items": [
        {"id": 1, "priority": 3, "status": "done"},
        {"id": 2, "priority": 8, "status": "pending"},
        {"id": 3, "priority": 5, "status": "pending"}
      ]
    }
    ```

    - `$.items[?(@.priority > 5)]` → `[{"id": 2, "priority": 8, "status": "pending"}]`
    - `$.items[?(@.status == 'pending')]` → items with id 2 and 3

!!! note
    Filter expressions use whitespace-separated syntax: `@.field operator value`. String values can be quoted with single or double quotes.

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
