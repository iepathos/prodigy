use super::{get_global_mmm_dir, Project, ProjectManager};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub variables: Vec<TemplateVariable>,
    pub structure: Vec<TemplateItem>,
    pub config: TemplateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default: Option<String>,
    pub choices: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateItem {
    pub path: String,
    pub files: Vec<TemplateFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFile {
    pub name: String,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    pub default_command: String,
    pub phases: Vec<String>,
}

pub struct TemplateManager {
    templates_dir: PathBuf,
}

impl TemplateManager {
    pub async fn new() -> Result<Self> {
        let global_dir = get_global_mmm_dir()?;
        let templates_dir = global_dir.join("templates");
        fs::create_dir_all(&templates_dir).await?;

        let manager = Self { templates_dir };
        manager.ensure_builtin_templates().await?;

        Ok(manager)
    }

    async fn ensure_builtin_templates(&self) -> Result<()> {
        // Create built-in templates if they don't exist
        let builtin_templates = vec![
            self.web_app_template(),
            self.cli_tool_template(),
            self.library_template(),
            self.api_service_template(),
        ];

        for template in builtin_templates {
            let template_path = self.templates_dir.join(format!("{}.yaml", template.name));
            if !template_path.exists() {
                let yaml =
                    serde_yaml::to_string(&template).map_err(|e| Error::Config(e.to_string()))?;
                fs::write(&template_path, yaml).await?;
            }
        }

        Ok(())
    }

    pub async fn list_templates(&self) -> Result<Vec<Template>> {
        let mut templates = Vec::new();

        let mut entries = fs::read_dir(&self.templates_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let content = fs::read_to_string(&path).await?;
                if let Ok(template) = serde_yaml::from_str::<Template>(&content) {
                    templates.push(template);
                }
            }
        }

        Ok(templates)
    }

    pub async fn create_from_template(
        &self,
        project_name: &str,
        project_path: &Path,
        template_name: &str,
    ) -> Result<()> {
        let template_path = self.templates_dir.join(format!("{}.yaml", template_name));
        if !template_path.exists() {
            return Err(Error::Project(format!(
                "Template '{}' not found",
                template_name
            )));
        }

        let content = fs::read_to_string(&template_path).await?;
        let template: Template =
            serde_yaml::from_str(&content).map_err(|e| Error::Config(e.to_string()))?;

        // Create project structure from template
        let mut project_manager = ProjectManager::new().await?;
        let _project = project_manager
            .create_project(project_name, project_path)
            .await?;

        // Apply template structure
        for item in &template.structure {
            let item_path = project_path.join(&item.path);
            fs::create_dir_all(&item_path).await?;

            for file in &item.files {
                let file_path = item_path.join(&file.name);
                if let Some(content) = &file.content {
                    fs::write(&file_path, content).await?;
                }
            }
        }

        // Update project configuration with template defaults
        let config_path = project_path.join(".mmm").join("config.toml");
        let mut config = toml::from_str::<toml::Value>(&fs::read_to_string(&config_path).await?)
            .map_err(|e| Error::Config(e.to_string()))?;

        if let toml::Value::Table(ref mut table) = config {
            table.insert(
                "default_command".to_string(),
                toml::Value::String(template.config.default_command),
            );
            table.insert(
                "phases".to_string(),
                toml::Value::Array(
                    template
                        .config
                        .phases
                        .into_iter()
                        .map(toml::Value::String)
                        .collect(),
                ),
            );
        }

        let config_content =
            toml::to_string_pretty(&config).map_err(|e| Error::Config(e.to_string()))?;
        fs::write(&config_path, config_content).await?;

        Ok(())
    }

    pub async fn create_from_project(
        &self,
        _template_name: &str,
        _project_name: &str,
    ) -> Result<()> {
        // TODO: Implement creating template from existing project
        Ok(())
    }

    pub async fn install_from_url(&self, _url: &str) -> Result<String> {
        // TODO: Implement downloading and installing template from URL
        Ok("template".to_string())
    }

    pub async fn remove_template(&self, name: &str) -> Result<()> {
        let template_path = self.templates_dir.join(format!("{}.yaml", name));
        if template_path.exists() {
            fs::remove_file(&template_path).await?;
        }
        Ok(())
    }

    // Built-in template definitions
    fn web_app_template(&self) -> Template {
        Template {
            name: "web-app".to_string(),
            description: "Full-stack web application template".to_string(),
            version: "1.0.0".to_string(),
            author: "mmm-community".to_string(),
            variables: vec![
                TemplateVariable {
                    name: "project_name".to_string(),
                    description: "Name of the project".to_string(),
                    required: true,
                    default: None,
                    choices: None,
                },
                TemplateVariable {
                    name: "framework".to_string(),
                    description: "Web framework to use".to_string(),
                    required: false,
                    default: Some("react".to_string()),
                    choices: Some(vec!["react".to_string(), "vue".to_string(), "angular".to_string(), "svelte".to_string()]),
                },
            ],
            structure: vec![
                TemplateItem {
                    path: "specs".to_string(),
                    files: vec![
                        TemplateFile {
                            name: "authentication.md".to_string(),
                            content: Some("# Authentication Specification\n\n## Objective\nImplement user authentication...".to_string()),
                        },
                        TemplateFile {
                            name: "database-schema.md".to_string(),
                            content: Some("# Database Schema Specification\n\n## Objective\nDefine database structure...".to_string()),
                        },
                    ],
                },
            ],
            config: TemplateConfig {
                default_command: "/implement-spec".to_string(),
                phases: vec!["planning".to_string(), "implementation".to_string(), "testing".to_string(), "review".to_string()],
            },
        }
    }

    fn cli_tool_template(&self) -> Template {
        Template {
            name: "cli-tool".to_string(),
            description: "Command-line application template".to_string(),
            version: "1.0.0".to_string(),
            author: "mmm-community".to_string(),
            variables: vec![],
            structure: vec![
                TemplateItem {
                    path: "specs".to_string(),
                    files: vec![
                        TemplateFile {
                            name: "cli-interface.md".to_string(),
                            content: Some("# CLI Interface Specification\n\n## Objective\nDefine command-line interface...".to_string()),
                        },
                    ],
                },
            ],
            config: TemplateConfig {
                default_command: "/implement-spec".to_string(),
                phases: vec!["design".to_string(), "implementation".to_string(), "testing".to_string()],
            },
        }
    }

    fn library_template(&self) -> Template {
        Template {
            name: "library".to_string(),
            description: "Reusable library/package template".to_string(),
            version: "1.0.0".to_string(),
            author: "mmm-community".to_string(),
            variables: vec![],
            structure: vec![TemplateItem {
                path: "specs".to_string(),
                files: vec![TemplateFile {
                    name: "api-design.md".to_string(),
                    content: Some(
                        "# API Design Specification\n\n## Objective\nDefine public API..."
                            .to_string(),
                    ),
                }],
            }],
            config: TemplateConfig {
                default_command: "/implement-spec".to_string(),
                phases: vec![
                    "design".to_string(),
                    "implementation".to_string(),
                    "documentation".to_string(),
                ],
            },
        }
    }

    fn api_service_template(&self) -> Template {
        Template {
            name: "api-service".to_string(),
            description: "REST/GraphQL API service template".to_string(),
            version: "1.0.0".to_string(),
            author: "mmm-community".to_string(),
            variables: vec![],
            structure: vec![TemplateItem {
                path: "specs".to_string(),
                files: vec![TemplateFile {
                    name: "api-endpoints.md".to_string(),
                    content: Some(
                        "# API Endpoints Specification\n\n## Objective\nDefine API endpoints..."
                            .to_string(),
                    ),
                }],
            }],
            config: TemplateConfig {
                default_command: "/implement-spec".to_string(),
                phases: vec![
                    "design".to_string(),
                    "implementation".to_string(),
                    "testing".to_string(),
                    "deployment".to_string(),
                ],
            },
        }
    }
}
