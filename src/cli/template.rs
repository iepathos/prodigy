//! Template management commands
//!
//! This module provides CLI commands for managing workflow templates.

use crate::cook::workflow::composition::registry::{
    FileTemplateStorage, TemplateMetadata, TemplateRegistry,
};
use crate::cook::workflow::composition::ComposableWorkflow;
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::PathBuf;

/// Template manager for CLI operations
pub struct TemplateManager {
    registry: TemplateRegistry,
}

impl TemplateManager {
    /// Create a new template manager
    pub fn new() -> Result<Self> {
        let template_dir = get_template_directory()?;
        let storage = Box::new(FileTemplateStorage::new(template_dir));
        let registry = TemplateRegistry::with_storage(storage);

        Ok(Self { registry })
    }

    /// Register a new template
    pub async fn register_template(
        &self,
        path: PathBuf,
        name: Option<String>,
        description: Option<String>,
        version: String,
        tags: Vec<String>,
        author: Option<String>,
    ) -> Result<()> {
        // Load template file
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read template file: {}", path.display()))?;

        let template: ComposableWorkflow =
            serde_yaml::from_str(&content).with_context(|| "Failed to parse template YAML")?;

        // Determine template name
        let template_name = name.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("template")
                .to_string()
        });

        // Create metadata
        let metadata = TemplateMetadata {
            description,
            author,
            version,
            tags,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        // Register in registry
        self.registry
            .register_template_with_metadata(template_name.clone(), template, metadata)
            .await
            .with_context(|| format!("Failed to register template '{}'", template_name))?;

        println!("✅ Template '{}' registered successfully", template_name);

        Ok(())
    }

    /// List all registered templates
    pub async fn list_templates(&self, tag: Option<String>, long: bool) -> Result<()> {
        let templates = if let Some(tag) = tag {
            self.registry.search_by_tags(&[tag]).await?
        } else {
            self.registry.list().await?
        };

        if templates.is_empty() {
            println!("No templates found.");
            return Ok(());
        }

        println!("Available Templates:\n");

        if long {
            for template in templates {
                println!("📦 {}", template.name);
                if let Some(desc) = &template.description {
                    println!("   Description: {}", desc);
                }
                println!("   Version: {}", template.version);
                if !template.tags.is_empty() {
                    println!("   Tags: {}", template.tags.join(", "));
                }
                println!();
            }
        } else {
            for template in templates {
                let desc = template.description.as_deref().unwrap_or("No description");
                println!("  {} - {}", template.name, desc);
            }
        }

        Ok(())
    }

    /// Show detailed information about a template
    pub async fn show_template(&self, name: String) -> Result<()> {
        let entry = self
            .registry
            .get_with_metadata(&name)
            .await
            .with_context(|| format!("Template '{}' not found", name))?;

        println!("📦 Template: {}\n", entry.name);

        if let Some(desc) = &entry.metadata.description {
            println!("Description: {}", desc);
        }

        println!("Version: {}", entry.metadata.version);

        if let Some(author) = &entry.metadata.author {
            println!("Author: {}", author);
        }

        if !entry.metadata.tags.is_empty() {
            println!("Tags: {}", entry.metadata.tags.join(", "));
        }

        println!(
            "\nCreated: {}",
            entry.metadata.created_at.format("%Y-%m-%d")
        );
        println!("Updated: {}", entry.metadata.updated_at.format("%Y-%m-%d"));

        // Show parameters if defined
        if let Some(params) = &entry.template.parameters {
            println!("\nRequired Parameters:");
            for param in &params.required {
                println!(
                    "  - {} ({:?}): {}",
                    param.name, param.type_hint, param.description
                );
            }

            if !params.optional.is_empty() {
                println!("\nOptional Parameters:");
                for param in &params.optional {
                    println!(
                        "  - {} ({:?}): {}",
                        param.name, param.type_hint, param.description
                    );
                    if let Some(default) = &param.default {
                        println!("    Default: {}", default);
                    }
                }
            }
        }

        Ok(())
    }

    /// Delete a template
    pub async fn delete_template(&self, name: String, force: bool) -> Result<()> {
        if !force {
            print!("Delete template '{}'? [y/N] ", name);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }

        self.registry
            .delete(&name)
            .await
            .with_context(|| format!("Failed to delete template '{}'", name))?;

        println!("✅ Template '{}' deleted", name);

        Ok(())
    }

    /// Search for templates
    pub async fn search_templates(&self, query: String, by_tag: bool) -> Result<()> {
        let templates = if by_tag {
            self.registry
                .search_by_tags(std::slice::from_ref(&query))
                .await?
        } else {
            let all = self.registry.list().await?;
            all.into_iter()
                .filter(|t| {
                    t.name.contains(&query)
                        || t.description
                            .as_ref()
                            .map(|d| d.contains(&query))
                            .unwrap_or(false)
                })
                .collect()
        };

        if templates.is_empty() {
            println!("No templates found matching '{}'", query);
            return Ok(());
        }

        println!("Found {} template(s):\n", templates.len());

        for template in templates {
            let desc = template.description.as_deref().unwrap_or("No description");
            println!("  {} - {}", template.name, desc);
        }

        Ok(())
    }

    /// Validate a template file
    pub async fn validate_template(&self, path: PathBuf) -> Result<()> {
        // Load and parse template
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read template file: {}", path.display()))?;

        let template: ComposableWorkflow =
            serde_yaml::from_str(&content).with_context(|| "Failed to parse template YAML")?;

        println!("✅ Template file is valid\n");

        // Show validation details
        if template.uses_composition() {
            println!("Composition features used:");
            if template.imports.is_some() {
                println!("  ✓ Imports");
            }
            if template.extends.is_some() {
                println!("  ✓ Inheritance");
            }
            if template.template.is_some() {
                println!("  ✓ Template reference");
            }
            if template.workflows.is_some() {
                println!("  ✓ Sub-workflows");
            }
        }

        if let Some(params) = &template.parameters {
            println!("\nParameters:");
            println!("  Required: {}", params.required.len());
            println!("  Optional: {}", params.optional.len());
        }

        if let Some(defaults) = &template.defaults {
            println!("\nDefault values: {}", defaults.len());
        }

        Ok(())
    }

    /// Initialize template directory structure
    pub async fn init_template_directory(&self, path: PathBuf) -> Result<()> {
        // Create template directory
        tokio::fs::create_dir_all(&path)
            .await
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;

        println!("✅ Created template directory: {}", path.display());

        // Create example template
        let example_template = r#"# Example Refactoring Template
# This template provides a standard workflow for code refactoring

# Template parameters
parameters:
  required:
    - name: target
      type: string
      description: Target file or directory to refactor

  optional:
    - name: style
      type: string
      description: Refactoring style (functional, modular, etc.)
      default: "functional"

# Default values
defaults:
  timeout: 300
  verbose: false

# Workflow commands
commands:
  - claude: "/analyze ${target}"
    capture_output: true

  - claude: "/refactor ${target} --style ${style}"
    timeout: 300

  - shell: "cargo test"
    on_failure:
      claude: "/fix-tests '${shell.output}'"

  - shell: "cargo fmt && cargo clippy"
"#;

        let example_path = path.join("refactor-example.yml");
        tokio::fs::write(&example_path, example_template)
            .await
            .with_context(|| "Failed to write example template")?;

        println!("✅ Created example template: {}", example_path.display());

        // Create README
        let readme = r#"# Workflow Templates

This directory contains reusable workflow templates for Prodigy.

## Using Templates

Register a template:
```bash
prodigy template register refactor-example.yml --name refactor-base
```

Use in a workflow:
```yaml
template:
  name: refactor-base
  source: refactor-base
  with:
    target: src/main.rs
    style: functional
```

## Template Structure

Templates can include:
- Parameters (required and optional)
- Default values
- Commands
- Sub-workflows
- Imports and inheritance

See refactor-example.yml for a complete example.
"#;

        let readme_path = path.join("README.md");
        tokio::fs::write(&readme_path, readme)
            .await
            .with_context(|| "Failed to write README")?;

        println!("✅ Created README: {}", readme_path.display());

        Ok(())
    }
}

/// Get the template directory path
///
/// Checks for project-local templates first, then falls back to user data directory
fn get_template_directory() -> Result<PathBuf> {
    // Check for project-local templates first
    if PathBuf::from("templates").exists() {
        return Ok(PathBuf::from("templates"));
    }

    // Fall back to user data directory
    let dirs = directories::ProjectDirs::from("com", "prodigy", "prodigy")
        .ok_or_else(|| anyhow::anyhow!("Could not determine project directories"))?;

    let template_dir = dirs.data_dir().join("templates");

    // Create if it doesn't exist
    std::fs::create_dir_all(&template_dir)?;

    Ok(template_dir)
}
