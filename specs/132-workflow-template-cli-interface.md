---
number: 132
title: Workflow Template CLI Interface
category: foundation
priority: high
status: draft
dependencies: [131]
created: 2025-10-13
---

# Specification 132: Workflow Template CLI Interface

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 131 (Execution Layer)

## Context

The workflow template system has complete backend infrastructure but no way for users to interact with it. Users cannot register templates, list available templates, search by tags, or pass parameters to template-based workflows from the command line. This creates a significant usability gap where the system exists but is inaccessible.

## Objective

Create comprehensive CLI commands for managing workflow templates and using them in workflows, making the template system fully accessible to end users.

## Requirements

### Functional Requirements

1. **Template Management Commands**
   - `prodigy template register <path> [--name NAME]` - Register a template
   - `prodigy template list [--tag TAG]` - List available templates
   - `prodigy template show <name>` - Display template details
   - `prodigy template delete <name>` - Remove a template
   - `prodigy template validate <path>` - Validate template file

2. **Template Registry Commands**
   - `prodigy template search <query>` - Search templates by name/description
   - `prodigy template search --tag <tag>` - Search by tags
   - `prodigy template info <name>` - Show detailed template metadata

3. **Workflow Parameter Support**
   - Accept parameters via `--param KEY=VALUE` flags
   - Support parameter files via `--param-file <path>`
   - Validate parameters before workflow execution
   - Display required parameters when missing

4. **Template Initialization**
   - `prodigy template init` - Initialize template directory structure
   - Create example templates for reference
   - Set up recommended directory layout

### Non-Functional Requirements

1. **User Experience**
   - Clear, colorized output using existing UI framework
   - Helpful error messages with suggestions
   - Tab completion support for template names
   - Progress indicators for long operations

2. **Performance**
   - List operations complete in < 500ms
   - Template registration completes in < 100ms
   - Parameter parsing adds negligible overhead

3. **Validation**
   - Validate template structure before registration
   - Check for duplicate template names
   - Verify parameter types match definitions
   - Warn about missing metadata

## Acceptance Criteria

- [ ] `prodigy template register` successfully registers templates
- [ ] `prodigy template list` displays all templates with metadata
- [ ] `prodigy template show` displays detailed template information
- [ ] `prodigy template delete` removes templates correctly
- [ ] `prodigy template search` finds templates by query
- [ ] `prodigy template search --tag` filters by tags
- [ ] `prodigy run` accepts `--param` flags and passes to composer
- [ ] `prodigy run` accepts `--param-file` for bulk parameters
- [ ] `prodigy template validate` checks template correctness
- [ ] `prodigy template init` creates directory structure
- [ ] Help text is clear and includes examples
- [ ] Error messages provide actionable guidance
- [ ] CLI tests cover all new commands
- [ ] Documentation includes usage examples

## Technical Details

### Implementation Approach

#### 1. CLI Command Structure

Add new subcommand to `src/cli/args.rs`:

```rust
#[derive(Parser, Debug)]
pub enum Commands {
    // ... existing commands ...

    /// Manage workflow templates
    #[command(name = "template")]
    Template {
        #[command(subcommand)]
        action: TemplateCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum TemplateCommand {
    /// Register a new workflow template
    Register {
        /// Path to template file
        path: PathBuf,

        /// Template name (defaults to filename)
        #[arg(short = 'n', long)]
        name: Option<String>,

        /// Template description
        #[arg(short = 'd', long)]
        description: Option<String>,

        /// Template version
        #[arg(short = 'v', long, default_value = "1.0.0")]
        version: String,

        /// Template tags (comma-separated)
        #[arg(short = 't', long, value_delimiter = ',')]
        tags: Vec<String>,

        /// Template author
        #[arg(short = 'a', long)]
        author: Option<String>,
    },

    /// List all registered templates
    List {
        /// Filter by tag
        #[arg(short = 't', long)]
        tag: Option<String>,

        /// Show detailed information
        #[arg(short = 'l', long)]
        long: bool,
    },

    /// Show detailed information about a template
    Show {
        /// Template name
        name: String,
    },

    /// Delete a registered template
    Delete {
        /// Template name
        name: String,

        /// Skip confirmation prompt
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// Search for templates
    Search {
        /// Search query
        query: String,

        /// Search by tag instead of name
        #[arg(short = 't', long)]
        by_tag: bool,
    },

    /// Validate a template file
    Validate {
        /// Path to template file
        path: PathBuf,
    },

    /// Initialize template directory structure
    Init {
        /// Template directory path (defaults to ./templates)
        #[arg(default_value = "templates")]
        path: PathBuf,
    },
}
```

#### 2. Parameter Support in Run Command

Modify `Run` command in `src/cli/args.rs`:

```rust
#[command(name = "run")]
Run {
    // ... existing fields ...

    /// Template parameters (key=value)
    #[arg(long = "param", value_name = "KEY=VALUE")]
    params: Vec<String>,

    /// Parameter file (JSON or YAML)
    #[arg(long = "param-file")]
    param_file: Option<PathBuf>,
}
```

#### 3. Command Implementations

Create `src/cli/template.rs`:

```rust
use crate::cook::workflow::{
    TemplateRegistry, TemplateMetadata, ComposableWorkflow,
    FileTemplateStorage,
};
use anyhow::{Context, Result};
use std::path::PathBuf;

pub struct TemplateManager {
    registry: TemplateRegistry,
}

impl TemplateManager {
    pub fn new() -> Result<Self> {
        let template_dir = get_template_directory()?;
        let storage = Box::new(FileTemplateStorage::new(template_dir));
        let registry = TemplateRegistry::with_storage(storage);

        Ok(Self { registry })
    }

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

        let template: ComposableWorkflow = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse template YAML")?;

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
                let desc = template.description
                    .as_deref()
                    .unwrap_or("No description");
                println!("  {} - {}", template.name, desc);
            }
        }

        Ok(())
    }

    pub async fn show_template(&self, name: String) -> Result<()> {
        let entry = self.registry
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

        println!("\nCreated: {}", entry.metadata.created_at.format("%Y-%m-%d"));
        println!("Updated: {}", entry.metadata.updated_at.format("%Y-%m-%d"));

        // Show parameters if defined
        if let Some(params) = &entry.template.parameters {
            println!("\nRequired Parameters:");
            for param in &params.required {
                println!("  - {} ({}): {}", param.name, format!("{:?}", param.type_hint), param.description);
            }

            if !params.optional.is_empty() {
                println!("\nOptional Parameters:");
                for param in &params.optional {
                    println!("  - {} ({}): {}", param.name, format!("{:?}", param.type_hint), param.description);
                    if let Some(default) = &param.default {
                        println!("    Default: {}", default);
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn delete_template(&self, name: String, force: bool) -> Result<()> {
        if !force {
            print!("Delete template '{}'? [y/N] ", name);
            use std::io::{self, Write};
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

    pub async fn search_templates(&self, query: String, by_tag: bool) -> Result<()> {
        let templates = if by_tag {
            self.registry.search_by_tags(&[query.clone()]).await?
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

    pub async fn validate_template(&self, path: PathBuf) -> Result<()> {
        // Load and parse template
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read template file: {}", path.display()))?;

        let template: ComposableWorkflow = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse template YAML")?;

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
```

#### 4. Parameter Parsing

Create `src/cli/params.rs`:

```rust
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub fn parse_cli_params(params: Vec<String>) -> Result<HashMap<String, Value>> {
    let mut result = HashMap::new();

    for param in params {
        let parts: Vec<&str> = param.splitn(2, '=').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid parameter format: '{}'. Expected KEY=VALUE", param);
        }

        let key = parts[0].to_string();
        let value = parse_param_value(parts[1])?;

        result.insert(key, value);
    }

    Ok(result)
}

pub async fn load_param_file(path: &Path) -> Result<HashMap<String, Value>> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read parameter file: {}", path.display()))?;

    // Try JSON first
    if let Ok(params) = serde_json::from_str::<HashMap<String, Value>>(&content) {
        return Ok(params);
    }

    // Try YAML
    serde_yaml::from_str::<HashMap<String, Value>>(&content)
        .with_context(|| "Failed to parse parameter file as JSON or YAML")
}

fn parse_param_value(value: &str) -> Result<Value> {
    // Try to parse as number
    if let Ok(num) = value.parse::<i64>() {
        return Ok(Value::Number(num.into()));
    }

    // Try to parse as float
    if let Ok(num) = value.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(num) {
            return Ok(Value::Number(num));
        }
    }

    // Try to parse as boolean
    if let Ok(b) = value.parse::<bool>() {
        return Ok(Value::Bool(b));
    }

    // Default to string
    Ok(Value::String(value.to_string()))
}

pub fn merge_params(
    cli_params: HashMap<String, Value>,
    file_params: HashMap<String, Value>,
) -> HashMap<String, Value> {
    let mut result = file_params;

    // CLI params override file params
    for (key, value) in cli_params {
        result.insert(key, value);
    }

    result
}
```

#### 5. Command Dispatcher

Add to `src/cli/mod.rs`:

```rust
pub async fn execute_command(command: Commands, verbose: bool) -> Result<()> {
    match command {
        // ... existing commands ...

        Commands::Template { action } => {
            execute_template_command(action).await
        }
    }
}

async fn execute_template_command(action: TemplateCommand) -> Result<()> {
    let manager = TemplateManager::new()?;

    match action {
        TemplateCommand::Register {
            path,
            name,
            description,
            version,
            tags,
            author,
        } => {
            manager
                .register_template(path, name, description, version, tags, author)
                .await
        }

        TemplateCommand::List { tag, long } => {
            manager.list_templates(tag, long).await
        }

        TemplateCommand::Show { name } => {
            manager.show_template(name).await
        }

        TemplateCommand::Delete { name, force } => {
            manager.delete_template(name, force).await
        }

        TemplateCommand::Search { query, by_tag } => {
            manager.search_templates(query, by_tag).await
        }

        TemplateCommand::Validate { path } => {
            manager.validate_template(path).await
        }

        TemplateCommand::Init { path } => {
            manager.init_template_directory(path).await
        }
    }
}
```

### Architecture Changes

1. **New Modules**:
   - `src/cli/template.rs` - Template management commands
   - `src/cli/params.rs` - Parameter parsing utilities

2. **Modified Modules**:
   - `src/cli/args.rs` - Add Template command enum
   - `src/cli/mod.rs` - Add template command dispatcher
   - `src/cook/command.rs` - Add params/param_file fields

### User Experience Design

#### Output Formatting

Use colorized, structured output:

```
$ prodigy template list

Available Templates:

  📦 refactor-base - Standard code refactoring workflow
  📦 test-suite - Comprehensive testing workflow
  📦 ci-pipeline - CI/CD integration workflow
```

```
$ prodigy template show refactor-base

📦 Template: refactor-base

Description: Standard code refactoring workflow
Version: 1.0.0
Author: Prodigy Team
Tags: refactor, code-quality

Created: 2025-10-13
Updated: 2025-10-13

Required Parameters:
  - target (string): Target file or directory to refactor

Optional Parameters:
  - style (string): Refactoring style (functional, modular, etc.)
    Default: "functional"
```

## Dependencies

### Prerequisites
- Spec 131: Workflow Template Execution Layer

### Affected Components
- `src/cli/args.rs` - CLI argument parsing
- `src/cli/mod.rs` - Command execution
- `src/cook/command.rs` - Cook command structure

### External Dependencies
- `clap` - Already used for CLI parsing
- `serde_json` - For parameter file parsing

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_parse_cli_params() {
    let params = vec![
        "target=src/main.rs".to_string(),
        "timeout=300".to_string(),
        "verbose=true".to_string(),
    ];

    let result = parse_cli_params(params).unwrap();

    assert_eq!(result.get("target"), Some(&Value::String("src/main.rs".to_string())));
    assert_eq!(result.get("timeout"), Some(&Value::Number(300.into())));
    assert_eq!(result.get("verbose"), Some(&Value::Bool(true)));
}

#[tokio::test]
async fn test_load_param_file_json() {
    let json_content = r#"{"target": "app.js", "style": "functional"}"#;
    // Write to temp file and test
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_template_register_list_delete() {
    // Register a template
    let output = run_command(&["template", "register", "test.yml", "--name", "test"]).await;
    assert!(output.contains("registered successfully"));

    // List templates
    let output = run_command(&["template", "list"]).await;
    assert!(output.contains("test"));

    // Delete template
    let output = run_command(&["template", "delete", "test", "--force"]).await;
    assert!(output.contains("deleted"));
}

#[tokio::test]
async fn test_run_with_parameters() {
    // Create template workflow
    // Run with --param flags
    // Verify parameters were applied
}
```

### CLI Tests

```bash
# Manual testing script
#!/bin/bash

# Initialize template directory
prodigy template init

# Register a template
prodigy template register templates/refactor-example.yml

# List templates
prodigy template list

# Show template details
prodigy template show refactor-base

# Run workflow with parameters
prodigy run workflow.yml --param target=src/main.rs --param style=functional

# Delete template
prodigy template delete refactor-base
```

## Documentation Requirements

### User Guide Sections

1. **Template Management**
   - Registering templates
   - Listing and searching
   - Template metadata
   - Directory structure

2. **Using Templates**
   - Template syntax in workflows
   - Passing parameters
   - Parameter files
   - Parameter precedence

3. **Template Development**
   - Creating templates
   - Parameter definitions
   - Best practices
   - Testing templates

### Help Text Examples

```
$ prodigy template --help

Manage workflow templates

Usage: prodigy template <COMMAND>

Commands:
  register  Register a new workflow template
  list      List all registered templates
  show      Show detailed information about a template
  delete    Delete a registered template
  search    Search for templates
  validate  Validate a template file
  init      Initialize template directory structure

Options:
  -h, --help  Print help
```

## Implementation Notes

### Design Decisions

1. **Template Directory Location**
   - Check project-local `./templates` first
   - Fall back to user data directory
   - Allow override via environment variable

2. **Parameter Precedence**
   - CLI params > param file > defaults
   - Document clearly in help text

3. **Error Handling**
   - Validate template structure before registration
   - Clear error messages with suggestions
   - Fail fast on invalid parameters

### Future Enhancements

1. Template versioning support
2. Remote template repositories
3. Template dependencies
4. Template namespacing
5. Template auto-update checks

## Migration and Compatibility

### Breaking Changes
- None (purely additive)

### Deprecations
- None

### Migration Path
- No migration needed
- New features opt-in
- Existing workflows unaffected

## Success Metrics

1. CLI commands execute successfully
2. Help text is comprehensive
3. Error messages are actionable
4. Parameter parsing handles edge cases
5. Template operations feel intuitive
6. Documentation is clear with examples
