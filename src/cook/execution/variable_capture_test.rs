//! Tests for variable capture engine

use super::variable_capture::*;
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn test_simple_capture() {
    let mut config = HashMap::new();
    config.insert("MY_VAR".to_string(), CaptureConfig::Simple(0));

    let mut engine = VariableCaptureEngine::new(config);

    let result = CommandResult {
        stdout: "test output\n".to_string(),
        stderr: String::new(),
        success: true,
        exit_code: Some(0),
    };

    engine.capture_from_command(0, &result).await.unwrap();

    let captured = engine.get_variable_value("MY_VAR").unwrap();
    assert_eq!(captured, &json!("test output\n"));
}

#[tokio::test]
async fn test_capture_with_pattern() {
    let mut config = HashMap::new();
    config.insert("VERSION".to_string(), CaptureConfig::Detailed {
        command_index: 0,
        source: CaptureSource::Stdout,
        pattern: Some(r"version (\d+\.\d+\.\d+)".to_string()),
        json_path: None,
        max_size: 1024,
        default: None,
        multiline: MultilineHandling::Preserve,
    });

    let mut engine = VariableCaptureEngine::new(config);

    let result = CommandResult {
        stdout: "Application version 1.2.3 running".to_string(),
        stderr: String::new(),
        success: true,
        exit_code: Some(0),
    };

    engine.capture_from_command(0, &result).await.unwrap();

    let captured = engine.get_variable_value("VERSION").unwrap();
    assert_eq!(captured, &json!("1.2.3"));
}

#[tokio::test]
async fn test_capture_with_json_path() {
    let mut config = HashMap::new();
    config.insert("API_KEY".to_string(), CaptureConfig::Detailed {
        command_index: 0,
        source: CaptureSource::Stdout,
        pattern: None,
        json_path: Some("credentials.api_key".to_string()),
        max_size: 1024,
        default: None,
        multiline: MultilineHandling::Preserve,
    });

    let mut engine = VariableCaptureEngine::new(config);

    let result = CommandResult {
        stdout: r#"{"credentials": {"api_key": "sk-12345", "secret": "hidden"}}"#.to_string(),
        stderr: String::new(),
        success: true,
        exit_code: Some(0),
    };

    engine.capture_from_command(0, &result).await.unwrap();

    let captured = engine.get_variable_value("API_KEY").unwrap();
    assert_eq!(captured, &json!("sk-12345"));
}

#[tokio::test]
async fn test_capture_with_multiline_handling() {
    let mut config = HashMap::new();

    // Test FirstLine
    config.insert("FIRST".to_string(), CaptureConfig::Detailed {
        command_index: 0,
        source: CaptureSource::Stdout,
        pattern: None,
        json_path: None,
        max_size: 1024,
        default: None,
        multiline: MultilineHandling::FirstLine,
    });

    // Test LastLine
    config.insert("LAST".to_string(), CaptureConfig::Detailed {
        command_index: 0,
        source: CaptureSource::Stdout,
        pattern: None,
        json_path: None,
        max_size: 1024,
        default: None,
        multiline: MultilineHandling::LastLine,
    });

    // Test Join
    config.insert("JOINED".to_string(), CaptureConfig::Detailed {
        command_index: 0,
        source: CaptureSource::Stdout,
        pattern: None,
        json_path: None,
        max_size: 1024,
        default: None,
        multiline: MultilineHandling::Join,
    });

    // Test Array
    config.insert("ARRAY".to_string(), CaptureConfig::Detailed {
        command_index: 0,
        source: CaptureSource::Stdout,
        pattern: None,
        json_path: None,
        max_size: 1024,
        default: None,
        multiline: MultilineHandling::Array,
    });

    let mut engine = VariableCaptureEngine::new(config);

    let result = CommandResult {
        stdout: "line1\nline2\nline3".to_string(),
        stderr: String::new(),
        success: true,
        exit_code: Some(0),
    };

    engine.capture_from_command(0, &result).await.unwrap();

    assert_eq!(engine.get_variable_value("FIRST").unwrap(), &json!("line1"));
    assert_eq!(engine.get_variable_value("LAST").unwrap(), &json!("line3"));
    assert_eq!(engine.get_variable_value("JOINED").unwrap(), &json!("line1 line2 line3"));
    assert_eq!(engine.get_variable_value("ARRAY").unwrap(), &json!(["line1", "line2", "line3"]));
}

#[tokio::test]
async fn test_capture_with_default_on_failure() {
    let mut config = HashMap::new();
    config.insert("SAFE_VAR".to_string(), CaptureConfig::Detailed {
        command_index: 0,
        source: CaptureSource::Stdout,
        pattern: Some(r"missing_pattern".to_string()),
        json_path: None,
        max_size: 1024,
        default: Some("default_value".to_string()),
        multiline: MultilineHandling::Preserve,
    });

    let mut engine = VariableCaptureEngine::new(config);

    let result = CommandResult {
        stdout: "This text does not match the pattern".to_string(),
        stderr: String::new(),
        success: true,
        exit_code: Some(0),
    };

    engine.capture_from_command(0, &result).await.unwrap();

    let captured = engine.get_variable_value("SAFE_VAR").unwrap();
    assert_eq!(captured, &json!("default_value"));
}

#[tokio::test]
async fn test_capture_stderr() {
    let mut config = HashMap::new();
    config.insert("ERROR_MSG".to_string(), CaptureConfig::Detailed {
        command_index: 0,
        source: CaptureSource::Stderr,
        pattern: None,
        json_path: None,
        max_size: 1024,
        default: None,
        multiline: MultilineHandling::Preserve,
    });

    let mut engine = VariableCaptureEngine::new(config);

    let result = CommandResult {
        stdout: "normal output".to_string(),
        stderr: "error output".to_string(),
        success: false,
        exit_code: Some(1),
    };

    engine.capture_from_command(0, &result).await.unwrap();

    let captured = engine.get_variable_value("ERROR_MSG").unwrap();
    assert_eq!(captured, &json!("error output"));
}

#[tokio::test]
async fn test_capture_both_outputs() {
    let mut config = HashMap::new();
    config.insert("ALL_OUTPUT".to_string(), CaptureConfig::Detailed {
        command_index: 0,
        source: CaptureSource::Both,
        pattern: None,
        json_path: None,
        max_size: 1024,
        default: None,
        multiline: MultilineHandling::Preserve,
    });

    let mut engine = VariableCaptureEngine::new(config);

    let result = CommandResult {
        stdout: "stdout content".to_string(),
        stderr: "stderr content".to_string(),
        success: true,
        exit_code: Some(0),
    };

    engine.capture_from_command(0, &result).await.unwrap();

    let captured = engine.get_variable_value("ALL_OUTPUT").unwrap();
    assert!(captured.as_str().unwrap().contains("stdout content"));
    assert!(captured.as_str().unwrap().contains("stderr content"));
}

#[tokio::test]
async fn test_size_limit() {
    let mut config = HashMap::new();
    config.insert("LIMITED".to_string(), CaptureConfig::Detailed {
        command_index: 0,
        source: CaptureSource::Stdout,
        pattern: None,
        json_path: None,
        max_size: 10,
        default: None,
        multiline: MultilineHandling::Preserve,
    });

    let mut engine = VariableCaptureEngine::new(config);

    let result = CommandResult {
        stdout: "This is a very long output that will be truncated".to_string(),
        stderr: String::new(),
        success: true,
        exit_code: Some(0),
    };

    engine.capture_from_command(0, &result).await.unwrap();

    let captured = engine.get_variable_value("LIMITED").unwrap();
    assert!(captured.as_str().unwrap().len() <= 10);
}

#[tokio::test]
async fn test_multiple_captures_from_different_commands() {
    let mut config = HashMap::new();
    config.insert("CMD0_OUTPUT".to_string(), CaptureConfig::Simple(0));
    config.insert("CMD1_OUTPUT".to_string(), CaptureConfig::Simple(1));
    config.insert("CMD2_OUTPUT".to_string(), CaptureConfig::Simple(2));

    let mut engine = VariableCaptureEngine::new(config);

    // Command 0
    engine.capture_from_command(0, &CommandResult {
        stdout: "output0".to_string(),
        stderr: String::new(),
        success: true,
        exit_code: Some(0),
    }).await.unwrap();

    // Command 1
    engine.capture_from_command(1, &CommandResult {
        stdout: "output1".to_string(),
        stderr: String::new(),
        success: true,
        exit_code: Some(0),
    }).await.unwrap();

    // Command 2
    engine.capture_from_command(2, &CommandResult {
        stdout: "output2".to_string(),
        stderr: String::new(),
        success: true,
        exit_code: Some(0),
    }).await.unwrap();

    assert_eq!(engine.get_variable_value("CMD0_OUTPUT").unwrap(), &json!("output0"));
    assert_eq!(engine.get_variable_value("CMD1_OUTPUT").unwrap(), &json!("output1"));
    assert_eq!(engine.get_variable_value("CMD2_OUTPUT").unwrap(), &json!("output2"));
}

#[test]
fn test_capture_config_serialization() {
    // Test that CaptureConfig can be serialized and deserialized correctly
    let config = CaptureConfig::Detailed {
        command_index: 1,
        source: CaptureSource::Stdout,
        pattern: Some(r"\d+".to_string()),
        json_path: Some("$.data".to_string()),
        max_size: 2048,
        default: Some("fallback".to_string()),
        multiline: MultilineHandling::Join,
    };

    let serialized = serde_json::to_string(&config).unwrap();
    let deserialized: CaptureConfig = serde_json::from_str(&serialized).unwrap();

    match deserialized {
        CaptureConfig::Detailed { command_index, .. } => {
            assert_eq!(command_index, 1);
        }
        _ => panic!("Expected detailed config"),
    }
}

#[test]
fn test_json_path_extraction() {
    // Test the extract_json_path helper function
    let data = json!({
        "users": [
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"}
        ],
        "metadata": {
            "version": "1.0",
            "count": 2
        }
    });

    assert_eq!(
        super::variable_capture::extract_json_path(&data, "metadata.version"),
        Some(json!("1.0"))
    );
    assert_eq!(
        super::variable_capture::extract_json_path(&data, "users[0].name"),
        Some(json!("Alice"))
    );
    assert_eq!(
        super::variable_capture::extract_json_path(&data, "users.1.id"),
        Some(json!(2))
    );
    assert_eq!(
        super::variable_capture::extract_json_path(&data, "missing.path"),
        None
    );
}