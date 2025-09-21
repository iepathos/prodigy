//! Template registry for reusable workflow components

use super::ComposableWorkflow;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Registry for workflow templates
pub struct TemplateRegistry {
    templates: Arc<RwLock<HashMap<String, TemplateEntry>>>,
    storage: Box<dyn TemplateStorage>,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateRegistry {
    /// Create a new template registry with default file storage
    pub fn new() -> Self {
        Self::with_storage(Box::new(FileTemplateStorage::new(PathBuf::from(
            "templates",
        ))))
    }

    /// Create a new template registry with custom storage
    pub fn with_storage(storage: Box<dyn TemplateStorage>) -> Self {
        Self {
            templates: Arc::new(RwLock::new(HashMap::new())),
            storage,
        }
    }

    /// Register a new template
    pub async fn register_template(
        &self,
        name: String,
        template: ComposableWorkflow,
    ) -> Result<()> {
        // Validate template
        self.validate_template(&template)
            .with_context(|| format!("Template '{}' validation failed", name))?;

        let entry = TemplateEntry {
            name: name.clone(),
            template: template.clone(),
            metadata: TemplateMetadata {
                description: None,
                author: None,
                version: "1.0.0".to_string(),
                tags: Vec::new(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
        };

        // Store template
        self.storage
            .store(&name, &entry)
            .await
            .with_context(|| format!("Failed to store template '{}'", name))?;

        // Cache in memory
        self.templates.write().await.insert(name, entry);

        Ok(())
    }

    /// Register a template with metadata
    pub async fn register_template_with_metadata(
        &self,
        name: String,
        template: ComposableWorkflow,
        metadata: TemplateMetadata,
    ) -> Result<()> {
        // Validate template
        self.validate_template(&template)
            .with_context(|| format!("Template '{}' validation failed", name))?;

        let entry = TemplateEntry {
            name: name.clone(),
            template: template.clone(),
            metadata,
        };

        // Store template
        self.storage
            .store(&name, &entry)
            .await
            .with_context(|| format!("Failed to store template '{}'", name))?;

        // Cache in memory
        self.templates.write().await.insert(name, entry);

        Ok(())
    }

    /// Get a template by name
    pub async fn get(&self, name: &str) -> Result<ComposableWorkflow> {
        // Check cache
        {
            let templates = self.templates.read().await;
            if let Some(entry) = templates.get(name) {
                return Ok(entry.template.clone());
            }
        }

        // Load from storage
        let entry = self
            .storage
            .load(name)
            .await
            .with_context(|| format!("Template '{}' not found", name))?;

        let template = entry.template.clone();

        // Cache for future use
        self.templates.write().await.insert(name.to_string(), entry);

        Ok(template)
    }

    /// Get template with metadata
    pub async fn get_with_metadata(&self, name: &str) -> Result<TemplateEntry> {
        // Check cache
        {
            let templates = self.templates.read().await;
            if let Some(entry) = templates.get(name) {
                return Ok(entry.clone());
            }
        }

        // Load from storage
        let entry = self
            .storage
            .load(name)
            .await
            .with_context(|| format!("Template '{}' not found", name))?;

        // Cache for future use
        self.templates
            .write()
            .await
            .insert(name.to_string(), entry.clone());

        Ok(entry)
    }

    /// List all available templates
    pub async fn list(&self) -> Result<Vec<TemplateInfo>> {
        self.storage.list().await
    }

    /// Search templates by tags
    pub async fn search_by_tags(&self, tags: &[String]) -> Result<Vec<TemplateInfo>> {
        let all_templates = self.list().await?;

        Ok(all_templates
            .into_iter()
            .filter(|info| tags.iter().any(|tag| info.tags.contains(tag)))
            .collect())
    }

    /// Delete a template
    pub async fn delete(&self, name: &str) -> Result<()> {
        // Remove from storage
        self.storage
            .delete(name)
            .await
            .with_context(|| format!("Failed to delete template '{}'", name))?;

        // Remove from cache
        self.templates.write().await.remove(name);

        Ok(())
    }

    /// Validate a template
    fn validate_template(&self, template: &ComposableWorkflow) -> Result<()> {
        // Check for required parameters without defaults
        if let Some(params) = &template.parameters {
            for param in &params.required {
                if param.default.is_none() && param.validation.is_none() {
                    tracing::warn!(
                        "Template parameter '{}' has no default and no validation",
                        param.name
                    );
                }
            }
        }

        // Validate sub-workflow references
        if let Some(workflows) = &template.workflows {
            for (name, sub) in workflows {
                if !sub.source.exists() && !sub.source.to_str().unwrap_or("").starts_with("${") {
                    tracing::warn!(
                        "Template sub-workflow '{}' references non-existent source: {:?}",
                        name,
                        sub.source
                    );
                }
            }
        }

        Ok(())
    }

    /// Load all templates from storage
    pub async fn load_all(&self) -> Result<()> {
        let templates = self.storage.list().await?;

        for info in templates {
            if let Ok(entry) = self.storage.load(&info.name).await {
                self.templates
                    .write()
                    .await
                    .insert(info.name.clone(), entry);
            }
        }

        Ok(())
    }
}

/// Template storage interface
#[async_trait]
pub trait TemplateStorage: Send + Sync {
    /// Store a template
    async fn store(&self, name: &str, entry: &TemplateEntry) -> Result<()>;

    /// Load a template
    async fn load(&self, name: &str) -> Result<TemplateEntry>;

    /// List all templates
    async fn list(&self) -> Result<Vec<TemplateInfo>>;

    /// Delete a template
    async fn delete(&self, name: &str) -> Result<()>;

    /// Check if template exists
    async fn exists(&self, name: &str) -> Result<bool>;
}

/// File-based template storage
pub struct FileTemplateStorage {
    base_dir: PathBuf,
}

impl FileTemplateStorage {
    /// Create new file storage with base directory
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn template_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(format!("{}.yml", name))
    }

    fn metadata_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(format!("{}.meta.json", name))
    }
}

#[async_trait]
impl TemplateStorage for FileTemplateStorage {
    async fn store(&self, name: &str, entry: &TemplateEntry) -> Result<()> {
        // Ensure directory exists
        tokio::fs::create_dir_all(&self.base_dir)
            .await
            .with_context(|| format!("Failed to create template directory: {:?}", self.base_dir))?;

        // Store template YAML
        let template_path = self.template_path(name);
        let template_yaml =
            serde_yaml::to_string(&entry.template).context("Failed to serialize template")?;

        tokio::fs::write(&template_path, template_yaml)
            .await
            .with_context(|| format!("Failed to write template file: {:?}", template_path))?;

        // Store metadata JSON
        let metadata_path = self.metadata_path(name);
        let metadata_json = serde_json::to_string_pretty(&entry.metadata)
            .context("Failed to serialize metadata")?;

        tokio::fs::write(&metadata_path, metadata_json)
            .await
            .with_context(|| format!("Failed to write metadata file: {:?}", metadata_path))?;

        Ok(())
    }

    async fn load(&self, name: &str) -> Result<TemplateEntry> {
        // Load template YAML
        let template_path = self.template_path(name);
        let template_content = tokio::fs::read_to_string(&template_path)
            .await
            .with_context(|| format!("Failed to read template file: {:?}", template_path))?;

        let template: ComposableWorkflow = serde_yaml::from_str(&template_content)
            .with_context(|| format!("Failed to parse template YAML: {:?}", template_path))?;

        // Load metadata if exists
        let metadata_path = self.metadata_path(name);
        let metadata = if metadata_path.exists() {
            let metadata_content = tokio::fs::read_to_string(&metadata_path)
                .await
                .with_context(|| format!("Failed to read metadata file: {:?}", metadata_path))?;

            serde_json::from_str(&metadata_content)
                .with_context(|| format!("Failed to parse metadata JSON: {:?}", metadata_path))?
        } else {
            TemplateMetadata::default()
        };

        Ok(TemplateEntry {
            name: name.to_string(),
            template,
            metadata,
        })
    }

    async fn list(&self) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        if !self.base_dir.exists() {
            return Ok(templates);
        }

        let mut entries = tokio::fs::read_dir(&self.base_dir)
            .await
            .with_context(|| format!("Failed to read template directory: {:?}", self.base_dir))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    // Skip metadata files
                    if stem.ends_with(".meta") {
                        continue;
                    }

                    // Try to load metadata
                    let metadata_path = self.metadata_path(stem);
                    let metadata = if metadata_path.exists() {
                        if let Ok(content) = tokio::fs::read_to_string(&metadata_path).await {
                            serde_json::from_str(&content).unwrap_or_default()
                        } else {
                            TemplateMetadata::default()
                        }
                    } else {
                        TemplateMetadata::default()
                    };

                    templates.push(TemplateInfo {
                        name: stem.to_string(),
                        description: metadata.description.clone(),
                        version: metadata.version.clone(),
                        tags: metadata.tags.clone(),
                    });
                }
            }
        }

        Ok(templates)
    }

    async fn delete(&self, name: &str) -> Result<()> {
        let template_path = self.template_path(name);
        if template_path.exists() {
            tokio::fs::remove_file(&template_path)
                .await
                .with_context(|| format!("Failed to delete template file: {:?}", template_path))?;
        }

        let metadata_path = self.metadata_path(name);
        if metadata_path.exists() {
            tokio::fs::remove_file(&metadata_path)
                .await
                .with_context(|| format!("Failed to delete metadata file: {:?}", metadata_path))?;
        }

        Ok(())
    }

    async fn exists(&self, name: &str) -> Result<bool> {
        Ok(self.template_path(name).exists())
    }
}

/// Template entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntry {
    /// Template name
    pub name: String,

    /// The template workflow
    pub template: ComposableWorkflow,

    /// Template metadata
    pub metadata: TemplateMetadata,
}

/// Template metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMetadata {
    /// Template description
    pub description: Option<String>,

    /// Template author
    pub author: Option<String>,

    /// Template version
    pub version: String,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Default for TemplateMetadata {
    fn default() -> Self {
        Self {
            description: None,
            author: None,
            version: "1.0.0".to_string(),
            tags: Vec::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

/// Template information for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    /// Template name
    pub name: String,

    /// Template description
    pub description: Option<String>,

    /// Template version
    pub version: String,

    /// Tags for categorization
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_template_registry() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage = Box::new(FileTemplateStorage::new(temp_dir.path().to_path_buf()));
        let registry = TemplateRegistry::with_storage(storage);

        let workflow = ComposableWorkflow::from_config(crate::config::WorkflowConfig {
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        });

        // Register template
        registry
            .register_template("test-template".to_string(), workflow.clone())
            .await
            .unwrap();

        // Retrieve template
        let retrieved = registry.get("test-template").await.unwrap();
        assert_eq!(
            retrieved.config.commands.len(),
            workflow.config.commands.len()
        );
    }

    #[test]
    fn test_template_metadata() {
        let metadata = TemplateMetadata {
            description: Some("Test template".to_string()),
            author: Some("Test Author".to_string()),
            version: "2.0.0".to_string(),
            tags: vec!["test".to_string(), "example".to_string()],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(metadata.version, "2.0.0");
        assert_eq!(metadata.tags.len(), 2);
    }
}
