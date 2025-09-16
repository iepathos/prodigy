use super::*;
use serde_json::json;
use std::fs::File;
use tempfile::TempDir;

#[tokio::test]
async fn test_arguments_provider() {
    let provider = arguments::ArgumentsInputProvider;
    let mut config = provider::InputConfig::new();
    config.set("args".to_string(), json!("arg1,arg2,key=value"));

    let inputs = provider.generate_inputs(&config).await.unwrap();
    assert_eq!(inputs.len(), 3);

    // Test first argument
    assert_eq!(inputs[0].id, "arg_0");
    assert_eq!(inputs[0].variables.get("arg").unwrap().to_string(), "arg1");

    // Test key=value parsing
    assert_eq!(inputs[2].id, "arg_2");
    assert_eq!(
        inputs[2].variables.get("arg_key").unwrap().to_string(),
        "key"
    );
    assert_eq!(
        inputs[2].variables.get("arg_value").unwrap().to_string(),
        "value"
    );
}

#[tokio::test]
async fn test_empty_input_source() {
    let processor = processor::InputProcessor::new();
    let config = config::InputConfig::default();

    let inputs = processor.process_inputs(&config).await.unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].id, "empty");
}

#[test]
fn test_variable_value_conversions() {
    use types::VariableValue;

    let str_val = VariableValue::String("test".to_string());
    assert_eq!(str_val.to_string(), "test");

    let num_val = VariableValue::Number(42);
    assert_eq!(num_val.as_number().unwrap(), 42);
    assert_eq!(num_val.to_string(), "42");

    let path_val = VariableValue::Path(std::path::PathBuf::from("/tmp/test"));
    assert_eq!(
        path_val.as_path().unwrap(),
        std::path::PathBuf::from("/tmp/test")
    );
}

#[test]
fn test_execution_input_variable_substitution() {
    use types::{ExecutionInput, InputType, VariableValue};

    let mut input = ExecutionInput::new(
        "test_input".to_string(),
        InputType::Arguments {
            separator: Some(",".to_string()),
        },
    );
    input.add_variable(
        "name".to_string(),
        VariableValue::String("test".to_string()),
    );
    input.add_variable("count".to_string(), VariableValue::Number(5));

    let template = "Processing {name} with count {count} (ID: {input_id})";
    let result = input.substitute_in_template(template).unwrap();
    assert_eq!(result, "Processing test with count 5 (ID: test_input)");
}

#[test]
fn test_execution_input_helper_functions() {
    use types::{ExecutionInput, InputType, VariableValue};

    let mut input = ExecutionInput::new("test".to_string(), InputType::Empty);
    input.add_variable(
        "path".to_string(),
        VariableValue::String("/path/to/file.txt".to_string()),
    );
    input.add_variable(
        "name".to_string(),
        VariableValue::String("Hello World".to_string()),
    );

    let template = "{path|basename} - {name|lowercase}";
    let result = input.substitute_in_template(template).unwrap();
    assert_eq!(result, "file.txt - hello world");
}

#[test]
fn test_legacy_adapter() {
    use crate::cook::command::CookCommand;
    use legacy_adapter::LegacyInputAdapter;

    let cmd = CookCommand {
        playbook: std::path::PathBuf::from("test.yml"),
        path: None,
        max_iterations: 1,
        worktree: false,
        map: vec!["*.rs".to_string()],
        args: vec!["arg1".to_string(), "arg2".to_string()],
        fail_fast: false,
        auto_accept: false,
        metrics: false,
        resume: None,
        verbosity: 0,
        quiet: false,
    };

    let config = LegacyInputAdapter::from_cook_command(&cmd).unwrap();

    // Should create a composite source with both args and file pattern
    match &config.sources[0] {
        config::InputSource::Composite { sources, .. } => {
            assert_eq!(sources.len(), 2);
        }
        _ => panic!("Expected composite source"),
    }
}

#[tokio::test]
async fn test_input_processor_with_transformations() {
    use config::{InputConfig, InputSource, TransformationConfig};

    let processor = processor::InputProcessor::new();

    let mut transformation = TransformationConfig::default();
    transformation
        .variable_transformations
        .insert("arg".to_string(), "uppercase".to_string());

    let config = InputConfig {
        sources: vec![InputSource::Arguments {
            value: "hello".to_string(),
            separator: Some(",".to_string()),
            validation: None,
        }],
        validation: Default::default(),
        transformation,
        caching: Default::default(),
    };

    let inputs = processor.process_inputs(&config).await.unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].variables.get("arg").unwrap().to_string(), "HELLO");
}

#[tokio::test]
async fn test_composite_input_source() {
    use config::{InputConfig, InputSource, MergeStrategy};

    let processor = processor::InputProcessor::new();

    let config = InputConfig {
        sources: vec![InputSource::Composite {
            sources: vec![
                InputSource::Arguments {
                    value: "a,b".to_string(),
                    separator: Some(",".to_string()),
                    validation: None,
                },
                InputSource::Arguments {
                    value: "c,d".to_string(),
                    separator: Some(",".to_string()),
                    validation: None,
                },
            ],
            merge_strategy: MergeStrategy::Sequential,
        }],
        validation: Default::default(),
        transformation: Default::default(),
        caching: Default::default(),
    };

    let inputs = processor.process_inputs(&config).await.unwrap();
    assert_eq!(inputs.len(), 4);
}

#[tokio::test]
async fn test_interleaved_merge_strategy() {
    use config::{InputConfig, InputSource, MergeStrategy};

    let processor = processor::InputProcessor::new();

    let config = InputConfig {
        sources: vec![InputSource::Composite {
            sources: vec![
                InputSource::Arguments {
                    value: "a,b".to_string(),
                    separator: Some(",".to_string()),
                    validation: None,
                },
                InputSource::Arguments {
                    value: "1,2".to_string(),
                    separator: Some(",".to_string()),
                    validation: None,
                },
            ],
            merge_strategy: MergeStrategy::Interleaved,
        }],
        validation: Default::default(),
        transformation: Default::default(),
        caching: Default::default(),
    };

    let inputs = processor.process_inputs(&config).await.unwrap();
    assert_eq!(inputs.len(), 4);

    // Check interleaved order: a, 1, b, 2
    assert_eq!(inputs[0].variables.get("arg").unwrap().to_string(), "a");
    assert_eq!(inputs[1].variables.get("arg").unwrap().to_string(), "1");
    assert_eq!(inputs[2].variables.get("arg").unwrap().to_string(), "b");
    assert_eq!(inputs[3].variables.get("arg").unwrap().to_string(), "2");
}

// ========== Standard Input Provider Tests ==========

#[tokio::test]
async fn test_standard_input_format_detection() {
    let provider = standard_input::StandardInputProvider;
    let mut config = provider::InputConfig::new();

    // Test JSON format detection
    config.set("format".to_string(), json!("json"));
    let validation = provider.validate(&config).await.unwrap();
    assert!(validation
        .iter()
        .all(|v| v.severity != provider::ValidationSeverity::Error));

    // Test unsupported format
    config.set("format".to_string(), json!("invalid_format"));
    let validation = provider.validate(&config).await.unwrap();
    assert!(validation
        .iter()
        .any(|v| v.field == "format" && v.severity == provider::ValidationSeverity::Error));
}

#[tokio::test]
async fn test_standard_input_automation_mode() {
    let provider = standard_input::StandardInputProvider;
    let mut config = provider::InputConfig::new();

    // Set automation mode environment variable
    std::env::set_var("PRODIGY_AUTOMATION", "true");

    // Should fail without allow_in_automation
    let validation = provider.validate(&config).await.unwrap();
    assert!(validation
        .iter()
        .any(|v| v.field == "stdin" && v.severity == provider::ValidationSeverity::Error));

    // Should pass with allow_in_automation
    config.set("allow_in_automation".to_string(), json!(true));
    let validation = provider.validate(&config).await.unwrap();
    assert!(validation.iter().all(|v| v.field != "stdin"));

    // Clean up
    std::env::remove_var("PRODIGY_AUTOMATION");
}

#[tokio::test]
async fn test_standard_input_supported_formats() {
    let provider = standard_input::StandardInputProvider;
    let supported_formats = vec!["json", "yaml", "csv", "lines", "text", "auto"];

    for format in supported_formats {
        let mut config = provider::InputConfig::new();
        config.set("format".to_string(), json!(format));
        config.set("allow_in_automation".to_string(), json!(true));

        let validation = provider.validate(&config).await.unwrap();
        assert!(
            validation
                .iter()
                .all(|v| v.field != "format" || v.severity != provider::ValidationSeverity::Error),
            "Format {} should be supported",
            format
        );
    }
}

// ========== Structured Data Provider Tests ==========

#[tokio::test]
async fn test_structured_data_validation() {
    let provider = structured_data::StructuredDataInputProvider;
    let mut config = provider::InputConfig::new();

    // Test missing data source
    let validation = provider.validate(&config).await.unwrap();
    assert!(validation
        .iter()
        .any(|v| v.field == "source" && v.severity == provider::ValidationSeverity::Error));

    // Test with file_path
    config.set("file_path".to_string(), json!("/tmp/data.json"));
    let validation = provider.validate(&config).await.unwrap();
    assert!(validation.iter().all(|v| v.field != "source"));

    // Test with inline data
    let mut config2 = provider::InputConfig::new();
    config2.set("data".to_string(), json!({"key": "value"}));
    let validation = provider.validate(&config2).await.unwrap();
    assert!(validation.iter().all(|v| v.field != "source"));
}

#[tokio::test]
async fn test_structured_data_format_validation() {
    let provider = structured_data::StructuredDataInputProvider;
    let supported = vec!["json", "yaml", "toml", "csv", "xml", "text", "auto"];
    let unsupported = vec!["invalid", "unknown", "binary"];

    for format in supported {
        let mut config = provider::InputConfig::new();
        config.set("data".to_string(), json!("test"));
        config.set("format".to_string(), json!(format));
        let validation = provider.validate(&config).await.unwrap();
        assert!(
            validation
                .iter()
                .all(|v| v.field != "format" || v.severity != provider::ValidationSeverity::Error),
            "Format {} should be supported",
            format
        );
    }

    for format in unsupported {
        let mut config = provider::InputConfig::new();
        config.set("data".to_string(), json!("test"));
        config.set("format".to_string(), json!(format));
        let validation = provider.validate(&config).await.unwrap();
        assert!(
            validation
                .iter()
                .any(|v| v.field == "format" && v.severity == provider::ValidationSeverity::Error),
            "Format {} should not be supported",
            format
        );
    }
}

// ========== File Pattern Provider Tests ==========

#[tokio::test]
async fn test_file_pattern_validation() {
    let provider = file_pattern::FilePatternInputProvider::new();
    let config = provider::InputConfig::new();

    // Test warning when no patterns provided
    let validation = provider.validate(&config).await.unwrap();
    assert!(validation
        .iter()
        .any(|v| v.field == "patterns" && v.severity == provider::ValidationSeverity::Warning));

    // Test with patterns provided
    let mut config_with_patterns = provider::InputConfig::new();
    config_with_patterns.set("patterns".to_string(), json!(["*.rs", "**/*.txt"]));
    let validation = provider.validate(&config_with_patterns).await.unwrap();
    assert!(validation
        .iter()
        .all(|v| v.field != "patterns" || v.severity != provider::ValidationSeverity::Warning));
}

#[tokio::test]
async fn test_file_pattern_glob_expansion() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test file structure
    let test_files = vec![
        "file1.txt",
        "file2.txt",
        "src/main.rs",
        "src/lib.rs",
        "tests/test.rs",
    ];

    for file in &test_files {
        let file_path = temp_path.join(file);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        File::create(file_path).unwrap();
    }

    let provider = file_pattern::FilePatternInputProvider::new();
    let mut config = provider::InputConfig::new();

    // Set the base path for glob patterns
    std::env::set_current_dir(temp_path).unwrap();

    // Test txt files pattern
    config.set("patterns".to_string(), json!(["*.txt"]));
    let inputs = provider.generate_inputs(&config).await.unwrap();
    assert_eq!(inputs.len(), 2, "Should find 2 .txt files");

    // Test recursive Rust files
    config.set("patterns".to_string(), json!(["**/*.rs"]));
    let inputs = provider.generate_inputs(&config).await.unwrap();
    assert_eq!(inputs.len(), 3, "Should find 3 .rs files");
}

// ========== Environment Provider Tests ==========

// NOTE: Environment and Generated providers require additional modules
// that may not be available in all configurations

// ========== Input Processor Error Handling Tests ==========

#[tokio::test]
async fn test_processor_validation_with_rules() {
    use config::{CustomValidationRule, InputConfig, InputSource, ValidationConfig};

    let processor = processor::InputProcessor::new();

    let mut validation = ValidationConfig::default();
    validation.custom_rules.push(CustomValidationRule {
        name: "arg_required".to_string(),
        expression: "arg.is_present".to_string(),
        error_message: "Argument is required".to_string(),
    });
    validation.strict = true;

    let config = InputConfig {
        sources: vec![InputSource::Empty],
        validation,
        transformation: Default::default(),
        caching: Default::default(),
    };

    // Empty source should not have 'arg' field, so validation might fail if strict
    let result = processor.process_inputs(&config).await;
    // Note: This depends on how validation is implemented
    assert!(result.is_ok(), "Should process even with validation issues");
}

#[test]
fn test_variable_value_type_conversions() {
    use types::VariableValue;

    // Test string to number conversion
    let str_num = VariableValue::String("123".to_string());
    assert_eq!(str_num.as_number().unwrap(), 123);

    // Test invalid number conversion
    let str_invalid = VariableValue::String("not_a_number".to_string());
    assert!(str_invalid.as_number().is_err());

    // Test boolean conversions
    let bool_true = VariableValue::Boolean(true);
    assert_eq!(bool_true.to_string(), "true");

    // Test array conversions
    let array = VariableValue::Array(vec![
        VariableValue::String("a".to_string()),
        VariableValue::String("b".to_string()),
    ]);
    // Check string representation of array
    assert!(array.to_string().contains("a"));
    assert!(array.to_string().contains("b"));
}

#[test]
fn test_execution_input_metadata() {
    use types::{ExecutionInput, InputType};

    let mut input = ExecutionInput::new("test".to_string(), InputType::Empty);

    // Test metadata addition using actual fields
    input.metadata.source = "/path/to/file".to_string();
    input.metadata.size_bytes = Some(42);
    input.metadata.checksum = Some("abc123".to_string());
    input
        .metadata
        .custom_fields
        .insert("line_number".to_string(), json!(10));

    assert_eq!(input.metadata.source, "/path/to/file");
    assert_eq!(input.metadata.size_bytes, Some(42));
    assert_eq!(input.metadata.checksum, Some("abc123".to_string()));
    assert_eq!(
        input.metadata.custom_fields.get("line_number"),
        Some(&json!(10))
    );
}

#[test]
fn test_input_transformation_helpers() {
    use types::{ExecutionInput, InputType, VariableValue};

    let mut input = ExecutionInput::new("test".to_string(), InputType::Empty);
    input.add_variable(
        "mixed_case".to_string(),
        VariableValue::String("HeLLo WoRLd".to_string()),
    );
    input.add_variable(
        "file_path".to_string(),
        VariableValue::String("/path/to/some/file.txt".to_string()),
    );

    // Test case transformations
    let template = "{mixed_case|uppercase} - {mixed_case|lowercase}";
    let result = input.substitute_in_template(template).unwrap();
    assert_eq!(result, "HELLO WORLD - hello world");

    // Test path manipulations
    let path_template = "{file_path|basename} in {file_path|dirname}";
    let result = input.substitute_in_template(path_template).unwrap();
    assert_eq!(result, "file.txt in /path/to/some");
}

// ========== Additional Input Type Tests ==========

#[test]
fn test_input_type_serialization() {
    use types::InputType;

    let empty = InputType::Empty;
    let json_str = serde_json::to_string(&empty).unwrap();
    assert!(json_str.contains("empty"));

    let args = InputType::Arguments {
        separator: Some(",".to_string()),
    };
    let json_str = serde_json::to_string(&args).unwrap();
    assert!(json_str.contains("arguments"));
}

#[test]
fn test_data_format_enum() {
    use types::DataFormat;

    let formats = vec![
        DataFormat::Json,
        DataFormat::Yaml,
        DataFormat::Toml,
        DataFormat::Csv,
        DataFormat::Xml,
        DataFormat::PlainText,
        DataFormat::Auto,
    ];

    for format in formats {
        // Test that each format can be serialized and deserialized
        let json_str = serde_json::to_string(&format).unwrap();
        let deserialized: DataFormat = serde_json::from_str(&json_str).unwrap();
        assert_eq!(json_str, serde_json::to_string(&deserialized).unwrap());
    }
}

#[test]
fn test_validation_rule_types() {
    use types::ValidationRule;

    let rules = vec![
        ValidationRule::FileExists,
        ValidationRule::Range {
            min: Some(0),
            max: Some(100),
        },
        ValidationRule::Pattern {
            regex: "^[a-z]+$".to_string(),
        },
        ValidationRule::OneOf {
            values: vec!["option1".to_string(), "option2".to_string()],
        },
        ValidationRule::Custom {
            validator: "custom_validator".to_string(),
        },
    ];

    for rule in rules {
        let json_str = serde_json::to_string(&rule).unwrap();
        let deserialized: ValidationRule = serde_json::from_str(&json_str).unwrap();
        assert_eq!(json_str, serde_json::to_string(&deserialized).unwrap());
    }
}

#[test]
fn test_variable_definition_complete() {
    use types::{ValidationRule, VariableDefinition, VariableType};

    let var_def = VariableDefinition {
        name: "test_var".to_string(),
        var_type: VariableType::Number,
        description: "Test variable".to_string(),
        required: true,
        default_value: Some("42".to_string()),
        validation_rules: vec![ValidationRule::Range {
            min: Some(0),
            max: Some(100),
        }],
    };

    assert_eq!(var_def.name, "test_var");
    assert!(var_def.required);
    assert_eq!(var_def.default_value, Some("42".to_string()));
    assert_eq!(var_def.validation_rules.len(), 1);
}

#[tokio::test]
async fn test_file_pattern_with_exclusions() {
    use std::fs::{create_dir_all, File};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test file structure with files to exclude
    let test_files = vec![
        "src/main.rs",
        "src/test.rs",
        "src/lib.rs",
        "target/debug/main",
        "target/release/main",
        "tests/integration_test.rs",
        ".git/config",
        "README.md",
    ];

    for file in &test_files {
        let file_path = temp_path.join(file);
        if let Some(parent) = file_path.parent() {
            create_dir_all(parent).unwrap();
        }
        File::create(file_path).unwrap();
    }

    let provider = file_pattern::FilePatternInputProvider::new();
    let mut config = provider::InputConfig::new();

    // Set the base path for glob patterns
    std::env::set_current_dir(temp_path).unwrap();

    // Test pattern that should exclude target directory
    config.set("patterns".to_string(), json!(["src/**/*.rs"]));
    let inputs = provider.generate_inputs(&config).await.unwrap();
    assert_eq!(inputs.len(), 3, "Should find 3 .rs files in src/");

    // Verify no target files are included
    for input in &inputs {
        if let Some(file_var) = input.variables.get("file") {
            assert!(!file_var.to_string().contains("target"));
        }
    }
}

#[test]
fn test_variable_value_complex_types() {
    use std::collections::HashMap;
    use types::VariableValue;

    // Test nested object
    let mut obj = HashMap::new();
    obj.insert(
        "key1".to_string(),
        VariableValue::String("value1".to_string()),
    );
    obj.insert("key2".to_string(), VariableValue::Number(42));

    let obj_value = VariableValue::Object(obj);
    let obj_str = obj_value.to_string();
    assert!(obj_str.contains("key1"));
    assert!(obj_str.contains("value1"));
    assert!(obj_str.contains("key2"));

    // Test nested array
    let nested_array = VariableValue::Array(vec![
        VariableValue::Number(1),
        VariableValue::Array(vec![
            VariableValue::String("nested".to_string()),
            VariableValue::Boolean(true),
        ]),
        VariableValue::Null,
    ]);
    let array_str = nested_array.to_string();
    assert!(array_str.contains("1"));
    assert!(array_str.contains("nested"));
    assert!(array_str.contains("true"));
    assert!(array_str.contains("null"));
}

#[test]
fn test_variable_value_path_conversion() {
    use std::path::PathBuf;
    use types::VariableValue;

    // Test direct path value
    let path = PathBuf::from("/tmp/test");
    let path_value = VariableValue::Path(path.clone());
    assert_eq!(path_value.as_path().unwrap(), path);

    // Test string to path conversion
    let str_value = VariableValue::String("/tmp/test".to_string());
    assert_eq!(str_value.as_path().unwrap(), PathBuf::from("/tmp/test"));

    // Test invalid conversion
    let num_value = VariableValue::Number(42);
    assert!(num_value.as_path().is_err());
}

#[test]
fn test_execution_input_dependencies() {
    use types::{ExecutionInput, InputType};

    let mut input = ExecutionInput::new("test".to_string(), InputType::Empty);

    // Add dependencies
    input.dependencies.push("dep1".to_string());
    input.dependencies.push("dep2".to_string());

    assert_eq!(input.dependencies.len(), 2);
    assert_eq!(input.dependencies[0], "dep1");
    assert_eq!(input.dependencies[1], "dep2");
}

#[tokio::test]
async fn test_processor_empty_source_handling() {
    use config::{InputConfig, InputSource};

    let processor = processor::InputProcessor::new();

    // Test with empty source (should generate single empty input)
    let config = InputConfig {
        sources: vec![InputSource::Empty],
        validation: Default::default(),
        transformation: Default::default(),
        caching: Default::default(),
    };

    let inputs = processor.process_inputs(&config).await.unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].id, "empty");
}

#[tokio::test]
async fn test_arguments_with_key_value_pairs() {
    let provider = arguments::ArgumentsInputProvider;
    let mut config = provider::InputConfig::new();

    // Test key=value parsing in more detail
    config.set("args".to_string(), json!("name=John,age=30,active=true"));

    let inputs = provider.generate_inputs(&config).await.unwrap();
    assert_eq!(inputs.len(), 3);

    // Check first key-value pair
    assert!(inputs[0].variables.contains_key("arg_key"));
    assert!(inputs[0].variables.contains_key("arg_value"));
    assert_eq!(
        inputs[0].variables.get("arg_key").unwrap().to_string(),
        "name"
    );
    assert_eq!(
        inputs[0].variables.get("arg_value").unwrap().to_string(),
        "John"
    );
}
