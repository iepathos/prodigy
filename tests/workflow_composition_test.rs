//! Integration tests for workflow composition functionality

use anyhow::Result;
use prodigy::config::WorkflowConfig;
use prodigy::cook::workflow::{
    ComposableWorkflow, Parameter, ParameterDefinitions, ParameterType, SubWorkflow,
    TemplateRegistry, TemplateSource, TemplateStorage, WorkflowComposer, WorkflowImport,
    WorkflowTemplate,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_basic_workflow_composition() -> Result<()> {
    let registry = Arc::new(TemplateRegistry::new());
    let _composer = WorkflowComposer::new(registry);

    // Create a simple workflow
    let workflow = ComposableWorkflow::from_config(WorkflowConfig {
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
        merge: None,
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
        merge: None,
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
        merge: None,
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
        merge: None,
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
        merge: None,
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
        merge: None,
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
        merge: None,
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
        merge: None,
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
        merge: None,
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
        merge: None,
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
        merge: None,
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
        merge: None,
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
        merge: None,
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

// ========================================
// FileTemplateStorage::list Test Coverage
// ========================================

#[tokio::test]
async fn test_list_non_existent_directory() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let non_existent_path = PathBuf::from("/tmp/prodigy-test-non-existent-dir-12345");
    let storage = FileTemplateStorage::new(non_existent_path);

    let templates = storage.list().await?;

    assert_eq!(templates.len(), 0);
    Ok(())
}

#[tokio::test]
async fn test_list_empty_directory() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;
    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());

    let templates = storage.list().await?;

    assert_eq!(templates.len(), 0);
    Ok(())
}

#[tokio::test]
async fn test_list_no_yml_files() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;

    // Create non-YAML files
    std::fs::write(temp_dir.path().join("readme.txt"), "Documentation")?;
    std::fs::write(temp_dir.path().join("config.json"), "{}")?;
    std::fs::write(temp_dir.path().join("data.xml"), "<root></root>")?;

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    assert_eq!(templates.len(), 0);
    Ok(())
}

#[tokio::test]
async fn test_list_yml_files_only() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;

    // Create .yml files
    std::fs::write(temp_dir.path().join("template1.yml"), "commands: []")?;
    std::fs::write(temp_dir.path().join("template2.yml"), "commands: []")?;

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    assert_eq!(templates.len(), 2);
    Ok(())
}

#[tokio::test]
async fn test_list_skips_meta_files() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;

    // Create .yml template and its .meta.json file
    std::fs::write(temp_dir.path().join("template.yml"), "commands: []")?;
    std::fs::write(
        temp_dir.path().join("template.meta.yml"),
        "description: metadata",
    )?;

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    // Should only find template.yml, skip template.meta.yml
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "template");
    Ok(())
}

#[tokio::test]
async fn test_list_mixed_file_types() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;

    // Create mixed file types
    std::fs::write(temp_dir.path().join("template.yml"), "commands: []")?;
    std::fs::write(temp_dir.path().join("readme.txt"), "Documentation")?;
    std::fs::write(temp_dir.path().join("config.json"), "{}")?;
    std::fs::write(temp_dir.path().join("data.yaml"), "key: value")?; // .yaml extension

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    // Should only find .yml files
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "template");
    Ok(())
}

#[tokio::test]
async fn test_list_files_without_extensions() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;

    // Create files with and without extensions
    std::fs::write(temp_dir.path().join("template.yml"), "commands: []")?;
    std::fs::write(temp_dir.path().join("noextension"), "content")?;
    std::fs::write(temp_dir.path().join("README"), "docs")?;

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    // Should only find .yml files
    assert_eq!(templates.len(), 1);
    Ok(())
}

#[tokio::test]
async fn test_list_with_valid_metadata() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;

    // Create template with valid metadata
    std::fs::write(temp_dir.path().join("template.yml"), "commands: []")?;
    let metadata_content = r#"{
            "description": "Test template",
            "author": "Test Author",
            "version": "1.0.0",
            "tags": ["test", "example"],
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;
    std::fs::write(temp_dir.path().join("template.meta.json"), metadata_content)?;

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "template");
    assert_eq!(templates[0].description, Some("Test template".to_string()));
    assert_eq!(templates[0].version, "1.0.0");
    assert_eq!(templates[0].tags, vec!["test", "example"]);
    Ok(())
}

#[tokio::test]
async fn test_list_without_metadata() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;

    // Create template without metadata file
    std::fs::write(temp_dir.path().join("template.yml"), "commands: []")?;

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "template");
    assert_eq!(templates[0].description, None);
    assert_eq!(templates[0].version, "1.0.0"); // Default version
    assert!(templates[0].tags.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_list_with_invalid_metadata_json() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;

    // Create template with corrupted metadata JSON
    std::fs::write(temp_dir.path().join("template.yml"), "commands: []")?;
    std::fs::write(
        temp_dir.path().join("template.meta.json"),
        "{ invalid json syntax",
    )?;

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    // Should use default metadata when JSON is invalid
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "template");
    assert_eq!(templates[0].description, None);
    assert_eq!(templates[0].version, "1.0.0");
    Ok(())
}

#[tokio::test]
async fn test_list_with_unreadable_metadata() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new()?;

    // Create template with unreadable metadata file
    std::fs::write(temp_dir.path().join("template.yml"), "commands: []")?;
    let metadata_path = temp_dir.path().join("template.meta.json");
    std::fs::write(&metadata_path, r#"{"description": "Should not be read"}"#)?;

    // Make metadata file unreadable (Unix only)
    #[cfg(unix)]
    std::fs::set_permissions(&metadata_path, std::fs::Permissions::from_mode(0o000))?;

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    // Restore permissions for cleanup
    #[cfg(unix)]
    std::fs::set_permissions(&metadata_path, std::fs::Permissions::from_mode(0o644))?;

    // Should use default metadata when file is unreadable
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "template");
    assert_eq!(templates[0].description, None);
    Ok(())
}

#[tokio::test]
async fn test_list_multiple_templates() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;

    // Create multiple templates with various metadata states
    std::fs::write(temp_dir.path().join("template1.yml"), "commands: []")?;
    std::fs::write(
        temp_dir.path().join("template1.meta.json"),
        r#"{
            "description": "First template",
            "author": "Author1",
            "version": "1.0.0",
            "tags": ["test"],
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#,
    )?;

    std::fs::write(temp_dir.path().join("template2.yml"), "commands: []")?;
    // No metadata for template2

    std::fs::write(temp_dir.path().join("template3.yml"), "commands: []")?;
    std::fs::write(
        temp_dir.path().join("template3.meta.json"),
        r#"{
            "description": "Third template",
            "author": "Author3",
            "version": "2.0.0",
            "tags": ["prod", "critical"],
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#,
    )?;

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    assert_eq!(templates.len(), 3);

    // Find each template and verify
    let t1 = templates.iter().find(|t| t.name == "template1").unwrap();
    assert_eq!(t1.description, Some("First template".to_string()));
    assert_eq!(t1.version, "1.0.0");

    let t2 = templates.iter().find(|t| t.name == "template2").unwrap();
    assert_eq!(t2.description, None);
    assert_eq!(t2.version, "1.0.0"); // Default

    let t3 = templates.iter().find(|t| t.name == "template3").unwrap();
    assert_eq!(t3.description, Some("Third template".to_string()));
    assert_eq!(t3.version, "2.0.0");
    assert_eq!(t3.tags, vec!["prod", "critical"]);

    Ok(())
}

#[tokio::test]
async fn test_list_populates_template_info_correctly() -> Result<()> {
    use prodigy::cook::workflow::composition::registry::FileTemplateStorage;

    let temp_dir = TempDir::new()?;

    // Create template with all metadata fields populated
    std::fs::write(temp_dir.path().join("full-template.yml"), "commands: []")?;
    std::fs::write(
        temp_dir.path().join("full-template.meta.json"),
        r#"{
            "description": "Complete template description",
            "author": "Test Author",
            "version": "3.2.1",
            "tags": ["integration", "test", "complete"],
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-06-01T00:00:00Z"
        }"#,
    )?;

    let storage = FileTemplateStorage::new(temp_dir.path().to_path_buf());
    let templates = storage.list().await?;

    assert_eq!(templates.len(), 1);

    // Verify all TemplateInfo fields are correctly populated
    let template = &templates[0];
    assert_eq!(template.name, "full-template");
    assert_eq!(
        template.description,
        Some("Complete template description".to_string())
    );
    assert_eq!(template.version, "3.2.1");
    assert_eq!(template.tags, vec!["integration", "test", "complete"]);

    Ok(())
}
