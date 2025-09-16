---
number: 67
title: Input Modules Testing
category: testing
priority: medium
status: draft
dependencies: []
created: 2025-09-16
---

# Specification 67: Input Modules Testing

**Category**: testing
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The input handling modules are completely untested with 0% coverage across 713 lines of code. These modules handle critical functionality including stdin processing, structured data parsing, dynamic input generation, file pattern matching, and environment variable handling. Adding tests would contribute ~1.4% to overall coverage while ensuring data input reliability.

## Objective

Achieve 50%+ test coverage for all input modules by implementing tests for data parsing, validation, error handling, and various input source types.

## Requirements

### Functional Requirements
- Test standard input reading and parsing
- Test structured data formats (JSON, YAML, TOML, CSV)
- Test dynamic input generation from commands
- Test file pattern matching with globs
- Test argument parsing and validation
- Test environment variable interpolation
- Test input validation and error reporting
- Test format auto-detection

### Non-Functional Requirements
- Tests must mock stdin for determinism
- Tests must handle malformed input gracefully
- Tests must verify error messages
- Tests must complete within 10 seconds total

## Acceptance Criteria

- [ ] All input modules have at least 50% coverage
- [ ] Standard input module handles all formats
- [ ] Structured data parsing is fully tested
- [ ] File pattern matching works correctly
- [ ] Input validation catches common errors
- [ ] Environment variable handling is verified
- [ ] Format detection works reliably
- [ ] All tests pass in CI environment

## Technical Details

### Implementation Approach

#### Modules to Test

1. **standard_input.rs**
   - Stdin reading (sync and async)
   - Format detection
   - Data parsing
   - Automation mode handling
   - Simulated input for testing

2. **structured_data.rs**
   - JSON parsing and validation
   - YAML parsing with anchors
   - TOML parsing
   - CSV processing
   - Schema validation
   - Type coercion

3. **generated.rs**
   - Command execution for input
   - Output capture
   - Dynamic data generation
   - Template processing
   - Error handling

4. **file_pattern.rs**
   - Glob pattern expansion
   - Recursive directory traversal
   - Pattern filtering
   - Path normalization
   - Symlink handling

5. **arguments.rs**
   - Argument parsing
   - Type validation
   - Default values
   - Required vs optional
   - Value transformation

6. **environment.rs**
   - Environment variable reading
   - Variable interpolation
   - Default values
   - Type conversion
   - Precedence rules

### Test Structure

```rust
// tests/input/mod.rs
mod standard_input_tests;
mod structured_data_tests;
mod generated_input_tests;
mod file_pattern_tests;
mod argument_tests;
mod environment_tests;

// Mock utilities
pub struct MockStdin {
    content: String,
    read_position: usize,
}

impl MockStdin {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
            read_position: 0,
        }
    }
}

pub struct MockCommandExecutor {
    outputs: HashMap<String, String>,
}
```

### Key Test Scenarios

```rust
#[tokio::test]
async fn test_stdin_format_detection() {
    let test_cases = vec![
        (r#"{"key": "value"}"#, DataFormat::Json),
        ("key: value\n", DataFormat::Yaml),
        ("[section]\nkey = value", DataFormat::Toml),
        ("col1,col2\nval1,val2", DataFormat::Csv),
        ("plain text", DataFormat::PlainText),
    ];

    for (input, expected_format) in test_cases {
        let provider = StandardInputProvider::new();
        let detected = provider.detect_format(input).await.unwrap();
        assert_eq!(detected, expected_format);
    }
}

#[test]
fn test_json_parsing_with_validation() {
    let input = r#"{
        "name": "test",
        "count": 42,
        "items": ["a", "b", "c"]
    }"#;

    let schema = r#"{
        "type": "object",
        "required": ["name", "count"],
        "properties": {
            "name": {"type": "string"},
            "count": {"type": "number", "minimum": 0}
        }
    }"#;

    let result = parse_json_with_schema(input, schema).unwrap();
    assert_eq!(result["name"], "test");
    assert_eq!(result["count"], 42);
}

#[test]
fn test_glob_pattern_matching() {
    let temp_dir = create_test_directory_structure();

    let patterns = vec![
        ("*.txt", vec!["file1.txt", "file2.txt"]),
        ("**/*.rs", vec!["src/main.rs", "src/lib.rs", "tests/test.rs"]),
        ("src/**/mod.rs", vec!["src/mod.rs", "src/subdir/mod.rs"]),
    ];

    for (pattern, expected_files) in patterns {
        let matches = expand_glob(temp_dir.path(), pattern).unwrap();
        assert_eq!(matches.len(), expected_files.len());
    }
}

#[test]
fn test_malformed_input_handling() {
    let malformed_inputs = vec![
        ("{'invalid': json}", DataFormat::Json),
        ("- invalid\nyaml: :", DataFormat::Yaml),
        ("[section\nunclosed", DataFormat::Toml),
    ];

    for (input, format) in malformed_inputs {
        let result = parse_structured_data(input, format);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("parse"));
    }
}

#[test]
fn test_environment_variable_interpolation() {
    std::env::set_var("TEST_VAR", "test_value");
    std::env::set_var("TEST_NUM", "42");

    let template = "Path: ${TEST_VAR}/file, Count: ${TEST_NUM}";
    let result = interpolate_env_vars(template).unwrap();

    assert_eq!(result, "Path: test_value/file, Count: 42");
}
```

### Error Handling Tests

```rust
#[test]
fn test_missing_required_arguments() {
    let args = ArgumentParser::new()
        .required("name")
        .optional("age");

    let input = HashMap::new();
    let result = args.parse(input);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("required"));
}

#[test]
fn test_type_validation_errors() {
    let input = r#"{"age": "not-a-number"}"#;
    let schema = r#"{"properties": {"age": {"type": "number"}}}"#;

    let result = validate_json(input, schema);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("type"));
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: All input provider modules
- **External Dependencies**: serde for parsing, glob for patterns

## Testing Strategy

- **Unit Tests**: Individual parsing functions
- **Integration Tests**: Complete input pipelines
- **Error Tests**: Malformed and invalid inputs
- **Mock Tests**: Stdin and command execution
- **Property Tests**: Fuzzing for parser robustness

## Documentation Requirements

- **Test Cases**: Document input/output examples
- **Error Messages**: Catalog validation errors
- **Format Specs**: Document supported formats

## Implementation Notes

### Mock Strategy

```rust
// Mock stdin for testing
impl Read for MockStdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let remaining = &self.content[self.read_position..];
        let to_read = std::cmp::min(buf.len(), remaining.len());
        buf[..to_read].copy_from_slice(&remaining.as_bytes()[..to_read]);
        self.read_position += to_read;
        Ok(to_read)
    }
}

// Test data generators
pub fn generate_test_json(size: usize) -> String {
    // Generate valid JSON of specified complexity
}

pub fn generate_test_csv(rows: usize, cols: usize) -> String {
    // Generate CSV data
}
```

### Format Detection Logic

```rust
fn detect_format(content: &str) -> DataFormat {
    let trimmed = content.trim();

    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        DataFormat::Json
    } else if trimmed.contains(':') && !trimmed.contains(',') {
        DataFormat::Yaml
    } else if trimmed.contains('[') && trimmed.contains(']') {
        DataFormat::Toml
    } else if trimmed.lines().any(|l| l.contains(',')) {
        DataFormat::Csv
    } else {
        DataFormat::PlainText
    }
}
```

## Migration and Compatibility

Tests are additive only; no changes to input handling logic required.