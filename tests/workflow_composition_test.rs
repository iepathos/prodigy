//! Integration tests for workflow composition functionality

use anyhow::Result;
use prodigy::config::WorkflowConfig;
use prodigy::cook::workflow::{
    ComposableWorkflow, Parameter, ParameterDefinitions, ParameterType, SubWorkflow,
    TemplateRegistry, TemplateSource, WorkflowComposer, WorkflowImport, WorkflowTemplate,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_basic_workflow_composition() -> Result<()> {
    let registry = Arc::new(TemplateRegistry::new());
    let composer = WorkflowComposer::new(registry);

    // Create a simple workflow
    let workflow = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    assert!(!workflow.uses_composition());
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_parameters() -> Result<()> {
    let mut workflow = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    // Add parameter definitions
    workflow.parameters = Some(ParameterDefinitions {
        required: vec![Parameter {
            name: "target_file".to_string(),
            type_hint: ParameterType::String,
            description: "File to process".to_string(),
            default: None,
            validation: None,
        }],
        optional: vec![Parameter {
            name: "style".to_string(),
            type_hint: ParameterType::String,
            description: "Processing style".to_string(),
            default: Some(Value::String("functional".to_string())),
            validation: None,
        }],
    });

    // Test parameter validation
    let mut params = HashMap::new();
    params.insert(
        "target_file".to_string(),
        Value::String("test.js".to_string()),
    );

    assert!(workflow.validate_parameters(&params).is_ok());

    // Test missing required parameter
    let empty_params = HashMap::new();
    assert!(workflow.validate_parameters(&empty_params).is_err());

    Ok(())
}

#[tokio::test]
async fn test_workflow_with_imports() -> Result<()> {
    let mut workflow = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    // Add imports
    workflow.imports = Some(vec![
        WorkflowImport {
            path: PathBuf::from("./common/utilities.yml"),
            alias: Some("utils".to_string()),
            selective: vec![],
        },
        WorkflowImport {
            path: PathBuf::from("./common/validators.yml"),
            alias: None,
            selective: vec!["validate_output".to_string()],
        },
    ]);

    assert!(workflow.uses_composition());
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_template() -> Result<()> {
    let mut workflow = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    // Add template usage
    workflow.template = Some(WorkflowTemplate {
        name: "refactor-base".to_string(),
        source: TemplateSource::Registry("refactor-base".to_string()),
        with: Some(HashMap::from([(
            "style".to_string(),
            Value::String("modular".to_string()),
        )])),
        override_field: None,
    });

    assert!(workflow.uses_composition());
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_sub_workflows() -> Result<()> {
    let mut workflow = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    // Add sub-workflows
    let mut sub_workflows = HashMap::new();
    sub_workflows.insert(
        "process_files".to_string(),
        SubWorkflow {
            source: PathBuf::from("./workflows/process.yml"),
            parameters: HashMap::from([("parallel".to_string(), Value::Bool(true))]),
            inputs: Some(HashMap::from([(
                "files".to_string(),
                "file_list".to_string(),
            )])),
            outputs: Some(vec!["processed_count".to_string()]),
            parallel: false,
            continue_on_error: false,
            timeout: Some(std::time::Duration::from_secs(300)),
            working_dir: None,
        },
    );

    workflow.workflows = Some(sub_workflows);

    assert!(workflow.uses_composition());
    Ok(())
}

#[tokio::test]
async fn test_template_registry() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let registry = TemplateRegistry::with_storage(Box::new(
        prodigy::cook::workflow::composition::registry::FileTemplateStorage::new(
            temp_dir.path().to_path_buf(),
        ),
    ));

    // Create a template workflow
    let template = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    // Register template
    registry
        .register_template("test-template".to_string(), template.clone())
        .await?;

    // Retrieve template
    let retrieved = registry.get("test-template").await?;
    assert_eq!(
        retrieved.config.commands.len(),
        template.config.commands.len()
    );

    // List templates
    let templates = registry.list().await?;
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "test-template");

    // Delete template
    registry.delete("test-template").await?;

    // Verify deletion
    assert!(registry.get("test-template").await.is_err());

    Ok(())
}

#[tokio::test]
async fn test_parameter_type_validation() -> Result<()> {
    let mut workflow = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    workflow.parameters = Some(ParameterDefinitions {
        required: vec![
            Parameter {
                name: "count".to_string(),
                type_hint: ParameterType::Number,
                description: "Number of iterations".to_string(),
                default: None,
                validation: None,
            },
            Parameter {
                name: "enabled".to_string(),
                type_hint: ParameterType::Boolean,
                description: "Enable feature".to_string(),
                default: None,
                validation: None,
            },
        ],
        optional: vec![],
    });

    // Test correct types
    let mut params = HashMap::new();
    params.insert(
        "count".to_string(),
        Value::Number(serde_json::Number::from(5)),
    );
    params.insert("enabled".to_string(), Value::Bool(true));
    assert!(workflow.validate_parameters(&params).is_ok());

    // Test incorrect type
    let mut wrong_params = HashMap::new();
    wrong_params.insert("count".to_string(), Value::String("five".to_string()));
    wrong_params.insert("enabled".to_string(), Value::Bool(true));
    assert!(workflow.validate_parameters(&wrong_params).is_err());

    Ok(())
}

#[tokio::test]
async fn test_workflow_defaults() -> Result<()> {
    let mut workflow = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    // Add defaults
    workflow.defaults = Some(HashMap::from([
        (
            "timeout".to_string(),
            Value::Number(serde_json::Number::from(300)),
        ),
        (
            "retry_count".to_string(),
            Value::Number(serde_json::Number::from(3)),
        ),
        ("verbose".to_string(), Value::Bool(false)),
    ]));

    assert_eq!(workflow.defaults.as_ref().unwrap().len(), 3);
    Ok(())
}

#[tokio::test]
async fn test_workflow_inheritance() -> Result<()> {
    let mut workflow = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    // Set up inheritance
    workflow.extends = Some("base-workflow".to_string());

    assert!(workflow.uses_composition());
    Ok(())
}

#[tokio::test]
async fn test_required_parameters() -> Result<()> {
    let mut workflow = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    workflow.parameters = Some(ParameterDefinitions {
        required: vec![
            Parameter {
                name: "input".to_string(),
                type_hint: ParameterType::String,
                description: "Input file".to_string(),
                default: None,
                validation: None,
            },
            Parameter {
                name: "output".to_string(),
                type_hint: ParameterType::String,
                description: "Output file".to_string(),
                default: None,
                validation: None,
            },
        ],
        optional: vec![],
    });

    let required = workflow.required_parameters();
    assert_eq!(required.len(), 2);
    assert_eq!(required[0].name, "input");
    assert_eq!(required[1].name, "output");

    Ok(())
}

#[tokio::test]
async fn test_template_metadata() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::TemplateMetadata;

    let temp_dir = TempDir::new()?;
    let registry = TemplateRegistry::with_storage(Box::new(
        prodigy::cook::workflow::composition::registry::FileTemplateStorage::new(
            temp_dir.path().to_path_buf(),
        ),
    ));

    let template = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });

    let metadata = TemplateMetadata {
        description: Some("Test template for unit testing".to_string()),
        author: Some("Test Suite".to_string()),
        version: "2.0.0".to_string(),
        tags: vec!["test".to_string(), "example".to_string()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Register with metadata
    registry
        .register_template_with_metadata("meta-template".to_string(), template, metadata.clone())
        .await?;

    // Retrieve with metadata
    let entry = registry.get_with_metadata("meta-template").await?;
    assert_eq!(entry.metadata.version, "2.0.0");
    assert_eq!(entry.metadata.tags.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_template_search_by_tags() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::TemplateMetadata;

    let temp_dir = TempDir::new()?;
    let registry = TemplateRegistry::with_storage(Box::new(
        prodigy::cook::workflow::composition::registry::FileTemplateStorage::new(
            temp_dir.path().to_path_buf(),
        ),
    ));

    // Register templates with different tags
    let template1 = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });
    let metadata1 = TemplateMetadata {
        description: Some("Refactoring template".to_string()),
        author: None,
        version: "1.0.0".to_string(),
        tags: vec!["refactor".to_string(), "code-quality".to_string()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    registry
        .register_template_with_metadata("refactor-template".to_string(), template1, metadata1)
        .await?;

    let template2 = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
    });
    let metadata2 = TemplateMetadata {
        description: Some("Testing template".to_string()),
        author: None,
        version: "1.0.0".to_string(),
        tags: vec!["test".to_string(), "validation".to_string()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    registry
        .register_template_with_metadata("test-template".to_string(), template2, metadata2)
        .await?;

    // Search by tags
    let refactor_templates = registry.search_by_tags(&["refactor".to_string()]).await?;
    assert_eq!(refactor_templates.len(), 1);
    assert_eq!(refactor_templates[0].name, "refactor-template");

    let test_templates = registry.search_by_tags(&["test".to_string()]).await?;
    assert_eq!(test_templates.len(), 1);
    assert_eq!(test_templates[0].name, "test-template");

    Ok(())
}
