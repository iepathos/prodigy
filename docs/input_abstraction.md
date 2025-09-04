# Input Abstraction System

The Input Abstraction System in Prodigy provides a flexible and extensible framework for handling various types of input data in workflows. This system enables workflows to process different data sources in a uniform way while maintaining type safety and validation.

## Overview

The input abstraction system is built around the concept of **Input Providers**, which transform various data sources into standardized **Execution Inputs** that can be processed by workflow steps.

## Architecture

```
┌─────────────────┐
│  Input Sources  │
└────────┬────────┘
         │
    ┌────▼─────┐
    │ Providers│
    └────┬─────┘
         │
  ┌──────▼──────┐
  │  Processor  │
  └──────┬──────┘
         │
   ┌─────▼──────┐
   │ Execution  │
   │   Inputs   │
   └────────────┘
```

## Input Types

### 1. Arguments
Processes command-line arguments or string parameters.

```yaml
input:
  type: arguments
  config:
    separator: ","  # Optional, default is comma
```

**Variables Available:**
- `arg`: The current argument value
- `arg_index`: Zero-based index
- `arg_count`: Total number of arguments
- `arg_key`: Key part (for key=value pairs)
- `arg_value`: Value part (for key=value pairs)

### 2. File Pattern
Processes files matching specified patterns.

```yaml
input:
  type: file_pattern
  config:
    patterns:
      - "*.rs"
      - "src/**/*.toml"
    recursive: true
```

**Variables Available:**
- `file_path`: Full path to the file
- `file_name`: File name without path
- `file_extension`: File extension
- `file_content`: File contents (if loaded)

### 3. Structured Data
Processes structured data in various formats (JSON, YAML, TOML, CSV, XML).

```yaml
input:
  type: structured_data
  config:
    file_path: "data.json"  # Or use 'data' for inline
    format: "json"  # auto, json, yaml, toml, csv, xml, text
```

**Variables Available:**
- `data`: The parsed data object
- `data_format`: Format of the data
- For CSV: `row`, `row_index`
- For text: `text`, `line_count`

### 4. Environment Variables
Processes system environment variables.

```yaml
input:
  type: environment
  config:
    prefix: "PRODIGY_"  # Optional prefix filter
    single_input: false  # Create one input per var
```

**Variables Available:**
- `env_key`: Variable name
- `env_value`: Variable value
- `env_key_stripped`: Name without prefix
- `env_value_number`: Value as number (if parseable)
- `env_value_bool`: Value as boolean (if parseable)
- `env_value_path`: Value as path (for PATH-like vars)

### 5. Standard Input
Processes data from standard input (stdin).

```yaml
input:
  type: standard_input
  config:
    format: "json"  # Format to expect
    process_lines: true  # Process line by line
```

**Variables Available:**
- `stdin_data`: Parsed data (for structured formats)
- `stdin_text`: Raw text content
- `stdin_lines`: Array of lines
- `stdin_format`: Format of the input

### 6. Generated Data
Generates synthetic data for testing or processing.

```yaml
input:
  type: generated
  config:
    generator: "sequence"  # Type of generator
    start: 1
    end: 100
    step: 1
```

**Generator Types:**
- `sequence`: Sequential numbers
- `random`: Random numbers
- `uuid`: UUID generation
- `timestamp`: Timestamps with intervals
- `range`: Floating-point ranges
- `grid`: 2D grid coordinates
- `fibonacci`: Fibonacci sequence
- `factorial`: Factorial sequence
- `prime`: Prime numbers

## Usage in Workflows

### Basic Example

```yaml
workflow:
  name: "Process Files"
  steps:
    - name: "Analyze Rust Files"
      input:
        type: file_pattern
        config:
          patterns: ["src/**/*.rs"]
      command: "cargo clippy --file {file_path}"
```

### Multiple Input Sources

```yaml
workflow:
  name: "Multi-Input Processing"
  steps:
    - name: "Process Data"
      input:
        type: composite
        sources:
          - type: arguments
            config:
              separator: ","
          - type: environment
            config:
              prefix: "APP_"
      command: "process --arg {arg} --env {env_key}={env_value}"
```

### MapReduce Pattern

```yaml
workflow:
  name: "MapReduce Example"
  steps:
    - name: "Map Phase"
      input:
        type: file_pattern
        config:
          patterns: ["data/*.json"]
      map:
        command: "transform {file_path}"
        output: "{file_name|basename}.out"
    - name: "Reduce Phase"
      reduce:
        command: "aggregate {outputs}"
```

## Variable Substitution

The input system supports variable substitution in commands with helper functions:

- `{variable}`: Basic substitution
- `{file_path|basename}`: Extract filename
- `{file_path|dirname}`: Extract directory
- `{text|uppercase}`: Convert to uppercase
- `{text|lowercase}`: Convert to lowercase
- `{text|trim}`: Remove whitespace

## Validation

Each input provider includes validation to ensure:
- Required configuration parameters are present
- Data formats are valid
- File paths exist (when applicable)
- Environment restrictions are respected

## Extending the System

### Creating Custom Providers

To create a custom input provider:

1. Implement the `InputProvider` trait:

```rust
#[async_trait]
impl InputProvider for MyCustomProvider {
    fn input_type(&self) -> InputType {
        // Return your input type
    }
    
    async fn validate(&self, config: &InputConfig) -> Result<Vec<ValidationIssue>> {
        // Validate configuration
    }
    
    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        // Generate execution inputs
    }
    
    fn available_variables(&self, config: &InputConfig) -> Result<Vec<VariableDefinition>> {
        // Define available variables
    }
    
    fn supports(&self, config: &InputConfig) -> bool {
        // Check if this provider can handle the config
    }
}
```

2. Register the provider in the processor.

## Migration Guide

### From Legacy System

The new input abstraction system replaces the previous ad-hoc input handling with a unified approach:

**Before:**
```yaml
steps:
  - command: "process {}"
    args: "file1.txt,file2.txt"
```

**After:**
```yaml
steps:
  - command: "process {arg}"
    input:
      type: arguments
      config:
        args: "file1.txt,file2.txt"
        separator: ","
```

### Benefits of Migration

1. **Type Safety**: Variables are typed and validated
2. **Consistency**: Uniform handling across input types
3. **Flexibility**: Easy to switch between input sources
4. **Testability**: Generated inputs for testing
5. **Extensibility**: Simple to add new input types

## Best Practices

1. **Use appropriate input types**: Choose the input type that best matches your data source
2. **Validate early**: Use provider validation to catch issues before execution
3. **Leverage variables**: Use the full set of available variables for flexibility
4. **Consider performance**: For large datasets, use streaming or batching
5. **Document configurations**: Clearly document required configuration parameters

## Error Handling

The input system provides detailed error messages for common issues:

- Missing configuration parameters
- Invalid data formats
- File not found errors
- Parse errors for structured data
- Environment variable access issues

Each error includes context about what was expected and suggestions for resolution.

## Performance Considerations

- **Lazy Loading**: File contents are loaded only when needed
- **Streaming**: Large files can be processed in chunks
- **Caching**: Parsed data is cached within a workflow step
- **Parallel Processing**: Multiple inputs can be processed concurrently

## Security

The input system includes security features:

- **Path Validation**: File paths are validated and sanitized
- **Environment Filtering**: Only specified environment variables are accessible
- **Input Sanitization**: User inputs are sanitized before use
- **Resource Limits**: Configurable limits on input size and count

## Debugging

Enable debug logging to see input processing details:

```bash
RUST_LOG=debug prodigy cook --workflow my-workflow.yml
```

This will show:
- Input provider selection
- Configuration validation
- Generated execution inputs
- Variable substitution

## Examples

### Processing CSV Data

```yaml
input:
  type: structured_data
  config:
    file_path: "data.csv"
    format: "csv"
command: "process-row --id {row.id} --name {row.name}"
```

### Using Environment Variables

```yaml
input:
  type: environment
  config:
    prefix: "CI_"
    single_input: true
command: "deploy --env {env.CI_ENVIRONMENT}"
```

### Generating Test Data

```yaml
input:
  type: generated
  config:
    generator: "uuid"
    count: 100
command: "create-record --id {uuid}"
```

## Conclusion

The Input Abstraction System provides a powerful, flexible, and extensible framework for handling diverse input sources in Prodigy workflows. By standardizing input processing, it simplifies workflow creation while maintaining the flexibility to handle complex data processing scenarios.