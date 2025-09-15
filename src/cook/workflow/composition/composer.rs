//! Workflow composer for building complex workflows from components

use super::{
    ComposableWorkflow, ComposedWorkflow, CompositionMetadata, DependencyInfo, DependencyType,
    ParameterDefinitions, TemplateRegistry, TemplateSource, WorkflowImport, WorkflowTemplate,
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Handles workflow composition from multiple sources
pub struct WorkflowComposer {
    loader: WorkflowLoader,
    template_registry: Arc<TemplateRegistry>,
    resolver: DependencyResolver,
}

impl WorkflowComposer {
    /// Create a new workflow composer
    pub fn new(template_registry: Arc<TemplateRegistry>) -> Self {
        Self {
            loader: WorkflowLoader::new(),
            template_registry,
            resolver: DependencyResolver::new(),
        }
    }

    /// Compose a workflow from source with parameters
    pub async fn compose(
        &self,
        source: &Path,
        params: HashMap<String, Value>,
    ) -> Result<ComposedWorkflow> {
        // Load base workflow
        let mut workflow = self
            .loader
            .load(source)
            .await
            .with_context(|| format!("Failed to load workflow from {:?}", source))?;

        let mut metadata = CompositionMetadata {
            sources: vec![source.to_path_buf()],
            templates: Vec::new(),
            parameters: params.clone(),
            composed_at: chrono::Utc::now(),
            dependencies: Vec::new(),
        };

        // Process imports
        if let Some(imports) = workflow.imports.clone() {
            self.process_imports(&mut workflow, &imports, &mut metadata)
                .await
                .context("Failed to process imports")?;
        }

        // Apply inheritance
        if let Some(base_name) = workflow.extends.clone() {
            self.apply_inheritance(&mut workflow, &base_name, &mut metadata)
                .await
                .context("Failed to apply inheritance")?;
        }

        // Apply template
        if let Some(template) = workflow.template.clone() {
            self.apply_template(&mut workflow, &template, &mut metadata)
                .await
                .context("Failed to apply template")?;
        }

        // Validate and apply parameters
        workflow
            .validate_parameters(&params)
            .context("Parameter validation failed")?;
        self.apply_parameters(&mut workflow, &params)?;

        // Resolve sub-workflows
        if let Some(sub_workflows) = &workflow.workflows {
            self.validate_sub_workflows(sub_workflows)
                .context("Sub-workflow validation failed")?;
        }

        // Apply defaults
        if let Some(defaults) = workflow.defaults.clone() {
            self.apply_defaults(&mut workflow, &defaults)?;
        }

        // Validate final composition
        self.validate_composition(&workflow, &metadata)
            .context("Composition validation failed")?;

        Ok(ComposedWorkflow { workflow, metadata })
    }

    async fn process_imports(
        &self,
        workflow: &mut ComposableWorkflow,
        imports: &[WorkflowImport],
        metadata: &mut CompositionMetadata,
    ) -> Result<()> {
        for import in imports {
            let imported = self
                .loader
                .load(&import.path)
                .await
                .with_context(|| format!("Failed to load import from {:?}", import.path))?;

            metadata.sources.push(import.path.clone());
            metadata.dependencies.push(DependencyInfo {
                source: import.path.clone(),
                dep_type: DependencyType::Import,
                resolved: import.path.display().to_string(),
            });

            if let Some(alias) = &import.alias {
                // Import with alias - store for reference
                tracing::debug!("Importing {:?} as alias '{}'", import.path, alias);
                // TODO: Implement aliased import storage
            } else if !import.selective.is_empty() {
                // Selective import
                self.import_selective(workflow, imported, &import.selective)?;
            } else {
                // Import all
                self.merge_workflows(workflow, imported)?;
            }
        }

        Ok(())
    }

    async fn apply_inheritance(
        &self,
        workflow: &mut ComposableWorkflow,
        base_name: &str,
        metadata: &mut CompositionMetadata,
    ) -> Result<()> {
        // Load base workflow
        let base_path = self.resolve_base_path(base_name)?;
        let base = self
            .loader
            .load(&base_path)
            .await
            .with_context(|| format!("Failed to load base workflow '{}'", base_name))?;

        metadata.dependencies.push(DependencyInfo {
            source: base_path.clone(),
            dep_type: DependencyType::Extends,
            resolved: base_name.to_string(),
        });

        // Merge base into current (current overrides base)
        self.merge_workflows_with_inheritance(workflow, base)?;

        Ok(())
    }

    async fn apply_template(
        &self,
        workflow: &mut ComposableWorkflow,
        template: &WorkflowTemplate,
        metadata: &mut CompositionMetadata,
    ) -> Result<()> {
        metadata.templates.push(template.name.clone());

        // Load template based on source
        let template_workflow = match &template.source {
            TemplateSource::File(path) => self.loader.load(path).await?,
            TemplateSource::Registry(name) => self
                .template_registry
                .get(name)
                .await
                .with_context(|| format!("Template '{}' not found in registry", name))?,
            TemplateSource::Url(url) => {
                anyhow::bail!("URL template sources not yet implemented: {}", url);
            }
        };

        // Apply parameters
        let mut instantiated = template_workflow;
        if let Some(params) = &template.with {
            self.apply_template_params(&mut instantiated, params)?;
        }

        // Apply overrides
        if let Some(overrides) = &template.override_field {
            self.apply_overrides(&mut instantiated, overrides)?;
        }

        // Merge with current workflow
        self.merge_workflows(workflow, instantiated)?;

        Ok(())
    }

    fn apply_parameters(
        &self,
        _workflow: &mut ComposableWorkflow,
        params: &HashMap<String, Value>,
    ) -> Result<()> {
        // Apply parameters throughout the workflow
        // This would involve variable substitution in commands
        tracing::debug!("Applying {} parameters to workflow", params.len());

        // TODO: Implement parameter substitution in commands

        Ok(())
    }

    fn apply_defaults(
        &self,
        _workflow: &mut ComposableWorkflow,
        defaults: &HashMap<String, Value>,
    ) -> Result<()> {
        // Apply default values where not already set
        tracing::debug!("Applying {} default values", defaults.len());

        // TODO: Implement default value application

        Ok(())
    }

    fn validate_composition(
        &self,
        workflow: &ComposableWorkflow,
        metadata: &CompositionMetadata,
    ) -> Result<()> {
        // Check for circular dependencies
        self.resolver.check_circular_deps(&metadata.dependencies)?;

        // Validate sub-workflow references
        if let Some(sub_workflows) = &workflow.workflows {
            self.validate_sub_workflows(sub_workflows)?;
        }

        Ok(())
    }

    fn merge_workflows(
        &self,
        target: &mut ComposableWorkflow,
        source: ComposableWorkflow,
    ) -> Result<()> {
        // Merge commands
        target.config.commands.extend(source.config.commands);

        // Merge parameters
        if let Some(source_params) = source.parameters {
            if target.parameters.is_none() {
                target.parameters = Some(ParameterDefinitions {
                    required: Vec::new(),
                    optional: Vec::new(),
                });
            }

            if let Some(target_params) = &mut target.parameters {
                target_params.required.extend(source_params.required);
                target_params.optional.extend(source_params.optional);
            }
        }

        // Merge defaults
        if let Some(source_defaults) = source.defaults {
            if target.defaults.is_none() {
                target.defaults = Some(HashMap::new());
            }

            if let Some(target_defaults) = &mut target.defaults {
                for (key, value) in source_defaults {
                    target_defaults.entry(key).or_insert(value);
                }
            }
        }

        // Merge sub-workflows
        if let Some(source_workflows) = source.workflows {
            if target.workflows.is_none() {
                target.workflows = Some(HashMap::new());
            }

            if let Some(target_workflows) = &mut target.workflows {
                target_workflows.extend(source_workflows);
            }
        }

        Ok(())
    }

    fn merge_workflows_with_inheritance(
        &self,
        child: &mut ComposableWorkflow,
        parent: ComposableWorkflow,
    ) -> Result<()> {
        // In inheritance, child overrides parent
        // Start with parent as base
        let mut merged = parent;

        // Override with child values
        if !child.config.commands.is_empty() {
            merged.config.commands = child.config.commands.clone();
        }

        if child.parameters.is_some() {
            merged.parameters = child.parameters.clone();
        }

        if child.defaults.is_some() {
            merged.defaults = child.defaults.clone();
        }

        if child.workflows.is_some() {
            merged.workflows = child.workflows.clone();
        }

        // Replace child with merged
        *child = merged;

        Ok(())
    }

    fn import_selective(
        &self,
        _target: &mut ComposableWorkflow,
        _source: ComposableWorkflow,
        selective: &[String],
    ) -> Result<()> {
        // Import only selected items
        for item in selective {
            tracing::debug!("Selectively importing '{}'", item);
            // TODO: Implement selective import based on item names
        }

        Ok(())
    }

    fn apply_template_params(
        &self,
        _template: &mut ComposableWorkflow,
        params: &HashMap<String, Value>,
    ) -> Result<()> {
        // Apply parameters to template
        tracing::debug!("Applying {} parameters to template", params.len());

        // TODO: Implement template parameter substitution

        Ok(())
    }

    fn apply_overrides(
        &self,
        _template: &mut ComposableWorkflow,
        overrides: &HashMap<String, Value>,
    ) -> Result<()> {
        // Apply overrides to template
        tracing::debug!("Applying {} overrides to template", overrides.len());

        // TODO: Implement template override application

        Ok(())
    }

    fn resolve_base_path(&self, base_name: &str) -> Result<PathBuf> {
        // Resolve base workflow path
        // Look in standard locations: ./bases/, ./templates/, ./workflows/
        let candidates = vec![
            PathBuf::from(format!("bases/{}.yml", base_name)),
            PathBuf::from(format!("templates/{}.yml", base_name)),
            PathBuf::from(format!("workflows/{}.yml", base_name)),
            PathBuf::from(format!("{}.yml", base_name)),
        ];

        for candidate in candidates {
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        anyhow::bail!("Base workflow '{}' not found", base_name);
    }

    fn validate_sub_workflows(
        &self,
        sub_workflows: &HashMap<String, super::SubWorkflow>,
    ) -> Result<()> {
        for (name, sub) in sub_workflows {
            if !sub.source.exists() {
                anyhow::bail!(
                    "Sub-workflow '{}' source does not exist: {:?}",
                    name,
                    sub.source
                );
            }
        }
        Ok(())
    }
}

/// Loads workflows from various sources
struct WorkflowLoader {
    cache: std::sync::Mutex<HashMap<PathBuf, ComposableWorkflow>>,
}

impl WorkflowLoader {
    fn new() -> Self {
        Self {
            cache: std::sync::Mutex::new(HashMap::new()),
        }
    }

    async fn load(&self, path: &Path) -> Result<ComposableWorkflow> {
        // Check cache
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(path) {
                return Ok(cached.clone());
            }
        }

        // Load from file
        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read workflow file: {:?}", path))?;

        let workflow: ComposableWorkflow = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse workflow YAML: {:?}", path))?;

        // Cache for future use
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(path.to_path_buf(), workflow.clone());
        }

        Ok(workflow)
    }
}

/// Resolves and validates workflow dependencies
struct DependencyResolver;

impl DependencyResolver {
    fn new() -> Self {
        Self
    }

    fn check_circular_deps(&self, dependencies: &[DependencyInfo]) -> Result<()> {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        // Build dependency graph
        for dep in dependencies {
            let from = dep.source.display().to_string();
            let to = dep.resolved.clone();

            graph.entry(from).or_default().push(to);
        }

        // Check for cycles using DFS
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node in graph.keys() {
            if !visited.contains(node)
                && Self::has_cycle(&graph, node, &mut visited, &mut rec_stack)?
            {
                anyhow::bail!("Circular dependency detected in workflow composition");
            }
        }

        Ok(())
    }

    fn has_cycle(
        graph: &HashMap<String, Vec<String>>,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> Result<bool> {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());

        if let Some(neighbors) = graph.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    if Self::has_cycle(graph, neighbor, visited, rec_stack)? {
                        return Ok(true);
                    }
                } else if rec_stack.contains(neighbor) {
                    return Ok(true);
                }
            }
        }

        rec_stack.remove(node);
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_composer_creation() {
        let registry = Arc::new(TemplateRegistry::new());
        let _composer = WorkflowComposer::new(registry);

        // Basic test to ensure composer can be created
        // Composer created successfully
    }

    #[test]
    fn test_dependency_resolver() {
        let resolver = DependencyResolver::new();

        let deps = vec![
            DependencyInfo {
                source: PathBuf::from("a.yml"),
                dep_type: DependencyType::Import,
                resolved: "b.yml".to_string(),
            },
            DependencyInfo {
                source: PathBuf::from("b.yml"),
                dep_type: DependencyType::Import,
                resolved: "c.yml".to_string(),
            },
        ];

        // Should not detect cycle in linear dependencies
        assert!(resolver.check_circular_deps(&deps).is_ok());

        let circular_deps = vec![
            DependencyInfo {
                source: PathBuf::from("a.yml"),
                dep_type: DependencyType::Import,
                resolved: "b.yml".to_string(),
            },
            DependencyInfo {
                source: PathBuf::from("b.yml"),
                dep_type: DependencyType::Import,
                resolved: "a.yml".to_string(),
            },
        ];

        // Should detect circular dependency
        assert!(resolver.check_circular_deps(&circular_deps).is_err());
    }
}
