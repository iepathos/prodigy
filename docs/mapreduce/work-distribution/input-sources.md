# Input Sources

Work items can be loaded from two types of input sources and extracted using JSONPath expressions.

## JSON Files

The most common approach is to use a JSON file containing work items. Specify the file path using the `input` field in the map phase:

```yaml
# Source: workflows/mapreduce-example.yml
map:
  input: debt_items.json
  json_path: "$.debt_items[*]"
```

## Command Output

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
