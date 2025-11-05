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
    aliases: std::sync::RwLock<HashMap<String, ComposableWorkflow>>,
}

impl WorkflowComposer {
    /// Create a new workflow composer
    pub fn new(template_registry: Arc<TemplateRegistry>) -> Self {
        Self {
            loader: WorkflowLoader::new(),
            template_registry,
            resolver: DependencyResolver::new(),
            aliases: std::sync::RwLock::new(HashMap::new()),
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
                let mut aliases = self.aliases.write().unwrap();
                aliases.insert(alias.clone(), imported);
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
        workflow: &mut ComposableWorkflow,
        params: &HashMap<String, Value>,
    ) -> Result<()> {
        // Apply parameters throughout the workflow
        // This would involve variable substitution in commands
        tracing::debug!("Applying {} parameters to workflow", params.len());

        // Substitute parameters in all commands
        for command in &mut workflow.config.commands {
            substitute_parameters_in_command(command, params)
                .context("Failed to substitute parameters in command")?;
        }

        Ok(())
    }

    fn apply_defaults(
        &self,
        workflow: &mut ComposableWorkflow,
        defaults: &HashMap<String, Value>,
    ) -> Result<()> {
        // Apply default values where not already set
        tracing::debug!("Applying {} default values", defaults.len());

        // Apply defaults to environment variables if not already set
        if workflow.config.env.is_none() {
            workflow.config.env = Some(HashMap::new());
        }

        if let Some(env) = &mut workflow.config.env {
            for (key, value) in defaults {
                // Only apply default if key doesn't exist
                if !env.contains_key(key) {
                    let value_str = match value {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => value.to_string(),
                    };
                    env.insert(key.clone(), value_str);
                }
            }
        }

        // Apply parameter defaults if defined
        if let Some(params) = &mut workflow.parameters {
            for param in params.optional.iter_mut() {
                if param.default.is_none() {
                    if let Some(default_value) = defaults.get(&param.name) {
                        param.default = Some(default_value.clone());
                    }
                }
            }
        }

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
        target: &mut ComposableWorkflow,
        source: ComposableWorkflow,
        selective: &[String],
    ) -> Result<()> {
        // Import only selected items
        for item in selective {
            tracing::debug!("Selectively importing '{}'", item);

            // Check if it's a command by index (e.g., "0", "1", "2")
            if let Ok(index) = item.parse::<usize>() {
                if let Some(command) = source.config.commands.get(index) {
                    target.config.commands.push(command.clone());
                    continue;
                }
            }

            // Check if it's a sub-workflow name
            if let Some(workflows) = &source.workflows {
                if let Some(workflow) = workflows.get(item) {
                    if target.workflows.is_none() {
                        target.workflows = Some(HashMap::new());
                    }
                    target
                        .workflows
                        .as_mut()
                        .unwrap()
                        .insert(item.clone(), workflow.clone());
                    continue;
                }
            }

            // Check if it's a parameter
            if let Some(params) = &source.parameters {
                let found_required = params.required.iter().find(|p| p.name == *item);

                let found_optional = params.optional.iter().find(|p| p.name == *item);

                if found_required.is_some() || found_optional.is_some() {
                    if target.parameters.is_none() {
                        target.parameters = Some(ParameterDefinitions {
                            required: Vec::new(),
                            optional: Vec::new(),
                        });
                    }

                    if let Some(target_params) = &mut target.parameters {
                        if let Some(param) = found_required {
                            target_params.required.push(param.clone());
                        } else if let Some(param) = found_optional {
                            target_params.optional.push(param.clone());
                        }
                    }
                    continue;
                }
            }

            anyhow::bail!("Item '{}' not found in source workflow", item);
        }

        Ok(())
    }

    fn apply_template_params(
        &self,
        template: &mut ComposableWorkflow,
        params: &HashMap<String, Value>,
    ) -> Result<()> {
        // Apply parameters to template
        tracing::debug!("Applying {} parameters to template", params.len());

        use regex::Regex;
        let param_regex =
            Regex::new(r"\$\{([^}]+)\}").context("Failed to create parameter regex")?;

        // Apply parameters to commands
        for command in &mut template.config.commands {
            substitute_parameters_in_command(command, params)?;
        }

        // Apply parameters to environment variables
        if let Some(env) = &mut template.config.env {
            for (_key, value) in env.iter_mut() {
                *value = self.substitute_params_in_string(&param_regex, value, params)?;
            }
        }

        // Apply parameters to merge workflow
        if let Some(merge) = &mut template.config.merge {
            for step in &mut merge.commands {
                self.substitute_params_in_workflow_step(&param_regex, step, params)?;
            }
        }

        Ok(())
    }

    fn substitute_params_in_string(
        &self,
        param_regex: &regex::Regex,
        text: &str,
        params: &HashMap<String, Value>,
    ) -> Result<String> {
        let mut result = text.to_string();

        for cap in param_regex.captures_iter(text) {
            let param_expr = &cap[1];

            // Support nested property access: ${item.location.file}
            let value = self.resolve_param_expression(param_expr, params)?;

            let placeholder = format!("${{{}}}", param_expr);
            result = result.replace(&placeholder, &value);
        }

        Ok(result)
    }

    fn resolve_param_expression(
        &self,
        expr: &str,
        params: &HashMap<String, Value>,
    ) -> Result<String> {
        // Split on dots for nested access
        let parts: Vec<&str> = expr.split('.').collect();

        let mut current = params
            .get(parts[0])
            .with_context(|| format!("Parameter '{}' not found", parts[0]))?;

        // Navigate nested structure
        for part in &parts[1..] {
            current = current.get(part).with_context(|| {
                format!("Property '{}' not found in parameter '{}'", part, expr)
            })?;
        }

        // Convert to string
        Ok(match current {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => current.to_string(),
        })
    }

    fn substitute_params_in_workflow_step(
        &self,
        param_regex: &regex::Regex,
        step: &mut crate::cook::workflow::WorkflowStep,
        params: &HashMap<String, Value>,
    ) -> Result<()> {
        // Substitute in claude commands
        if let Some(cmd) = &mut step.claude {
            *cmd = self.substitute_params_in_string(param_regex, cmd, params)?;
        }

        // Substitute in shell commands
        if let Some(cmd) = &mut step.shell {
            *cmd = self.substitute_params_in_string(param_regex, cmd, params)?;
        }

        // Substitute in goal_seek configuration
        if let Some(goal_seek) = &mut step.goal_seek {
            goal_seek.goal =
                self.substitute_params_in_string(param_regex, &goal_seek.goal, params)?;
            goal_seek.validate =
                self.substitute_params_in_string(param_regex, &goal_seek.validate, params)?;
            if let Some(claude) = &mut goal_seek.claude {
                *claude = self.substitute_params_in_string(param_regex, claude, params)?;
            }
            if let Some(shell) = &mut goal_seek.shell {
                *shell = self.substitute_params_in_string(param_regex, shell, params)?;
            }
        }

        // Substitute in legacy command field
        if let Some(cmd) = &mut step.command {
            *cmd = self.substitute_params_in_string(param_regex, cmd, params)?;
        }

        Ok(())
    }

    fn apply_overrides(
        &self,
        template: &mut ComposableWorkflow,
        overrides: &HashMap<String, Value>,
    ) -> Result<()> {
        // Apply overrides to template
        tracing::debug!("Applying {} overrides to template", overrides.len());

        for (key, value) in overrides {
            match key.as_str() {
                "commands" => {
                    // Override commands array
                    if let Value::Array(commands) = value {
                        template.config.commands = self.parse_commands(commands)?;
                    }
                }

                "env" => {
                    // Override environment variables
                    if let Value::Object(env) = value {
                        let env_map: HashMap<String, String> = env
                            .iter()
                            .map(|(k, v)| (k.clone(), v.to_string()))
                            .collect();
                        template.config.env = Some(env_map);
                    }
                }

                "merge" => {
                    // Override merge workflow
                    if let Value::Object(merge) = value {
                        template.config.merge = Some(self.parse_merge_config(merge)?);
                    }
                }

                // Support dot notation for nested overrides
                key if key.contains('.') => {
                    self.apply_nested_override(template, key, value)?;
                }

                _ => {
                    tracing::warn!("Unknown override key: {}", key);
                }
            }
        }

        Ok(())
    }

    fn parse_commands(&self, commands: &[Value]) -> Result<Vec<crate::config::WorkflowCommand>> {
        commands
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).context("Failed to parse command from override")
            })
            .collect()
    }

    fn parse_merge_config(
        &self,
        merge: &serde_json::Map<String, Value>,
    ) -> Result<crate::config::mapreduce::MergeWorkflow> {
        serde_json::from_value(Value::Object(merge.clone()))
            .context("Failed to parse merge config from override")
    }

    fn apply_nested_override(
        &self,
        _template: &mut ComposableWorkflow,
        path: &str,
        _value: &Value,
    ) -> Result<()> {
        // Support paths like "commands[0].timeout"
        // This is a simplified implementation - full path navigation
        // would require more complex logic
        tracing::debug!("Applying nested override at path: {}", path);

        // Note: Full implementation of nested path navigation would be complex
        // and is left for future enhancement if needed

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

/// Substitute parameters in a workflow command
fn substitute_parameters_in_command(
    command: &mut crate::config::WorkflowCommand,
    params: &HashMap<String, Value>,
) -> Result<()> {
    use crate::config::WorkflowCommand;
    use regex::Regex;

    // Create regex for ${param} pattern matching
    let param_regex = Regex::new(r"\$\{([^}]+)\}").context("Failed to create parameter regex")?;

    match command {
        WorkflowCommand::Simple(ref mut cmd) => {
            *cmd = substitute_params(&param_regex, cmd, params)?;
        }
        WorkflowCommand::Structured(ref mut boxed_cmd) => {
            // Handle structured command fields
            substitute_params_in_structured(&param_regex, boxed_cmd, params)?;
        }
        WorkflowCommand::WorkflowStep(ref mut boxed_step) => {
            // Handle workflow step command fields
            if let Some(ref mut claude) = boxed_step.claude {
                *claude = substitute_params(&param_regex, claude, params)?;
            }
            if let Some(ref mut shell) = boxed_step.shell {
                *shell = substitute_params(&param_regex, shell, params)?;
            }
            // Handle other fields that might contain parameters
            if let Some(ref mut id) = boxed_step.id {
                *id = substitute_params(&param_regex, id, params)?;
            }
        }
        WorkflowCommand::SimpleObject(ref mut simple) => {
            simple.name = substitute_params(&param_regex, &simple.name, params)?;
            if let Some(ref mut args) = simple.args {
                for arg in args {
                    *arg = substitute_params(&param_regex, arg, params)?;
                }
            }
        }
    }

    Ok(())
}

/// Substitute parameters in structured command
fn substitute_params_in_structured(
    regex: &regex::Regex,
    cmd: &mut crate::config::Command,
    params: &HashMap<String, Value>,
) -> Result<()> {
    // Substitute in command name
    cmd.name = substitute_params(regex, &cmd.name, params)?;

    // Substitute in args (args is a Vec, not Option<Vec>)
    for arg in &mut cmd.args {
        if let Some(name) = arg.as_literal_mut() {
            *name = substitute_params(regex, name, params)?;
        }
    }

    Ok(())
}

/// Helper to access CommandArg literal value mutably
trait CommandArgExt {
    fn as_literal_mut(&mut self) -> Option<&mut String>;
}

impl CommandArgExt for crate::config::CommandArg {
    fn as_literal_mut(&mut self) -> Option<&mut String> {
        match self {
            crate::config::CommandArg::Literal(ref mut s) => Some(s),
            crate::config::CommandArg::Variable(_) => None,
        }
    }
}

/// Substitute parameter references in a string
fn substitute_params(
    regex: &regex::Regex,
    text: &str,
    params: &HashMap<String, Value>,
) -> Result<String> {
    let mut result = text.to_string();
    let mut errors = Vec::new();

    for cap in regex.captures_iter(text) {
        let param_name = &cap[1];
        match params.get(param_name) {
            Some(value) => {
                let value_str = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Array(_) | Value::Object(_) => {
                        // Complex types are serialized as JSON
                        serde_json::to_string(value).with_context(|| {
                            format!("Failed to serialize parameter '{}'", param_name)
                        })?
                    }
                    Value::Null => String::new(),
                };

                result = result.replace(&format!("${{{}}}", param_name), &value_str);
            }
            None => {
                errors.push(format!("Parameter '{}' not found", param_name));
            }
        }
    }

    if !errors.is_empty() {
        anyhow::bail!("Parameter substitution errors: {}", errors.join(", "));
    }

    Ok(result)
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

    #[test]
    fn test_substitute_params() {
        use regex::Regex;

        let mut params = HashMap::new();
        params.insert("target".to_string(), Value::String("app.js".to_string()));
        params.insert(
            "count".to_string(),
            Value::Number(serde_json::Number::from(42)),
        );
        params.insert("enabled".to_string(), Value::Bool(true));

        let regex = Regex::new(r"\$\{([^}]+)\}").unwrap();

        // Test simple string substitution
        let result = substitute_params(&regex, "Process ${target}", &params);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Process app.js");

        // Test number substitution
        let result = substitute_params(&regex, "Count: ${count}", &params);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Count: 42");

        // Test boolean substitution
        let result = substitute_params(&regex, "Enabled: ${enabled}", &params);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Enabled: true");

        // Test multiple substitutions
        let result =
            substitute_params(&regex, "${target} has ${count} items (${enabled})", &params);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "app.js has 42 items (true)");

        // Test missing parameter
        let result = substitute_params(&regex, "Missing: ${missing}", &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_substitute_parameters_in_command() {
        use crate::config::WorkflowCommand;

        let mut params = HashMap::new();
        params.insert("file".to_string(), Value::String("main.rs".to_string()));

        // Test with Simple command
        let mut cmd = WorkflowCommand::Simple("claude: /refactor ${file}".to_string());
        let result = substitute_parameters_in_command(&mut cmd, &params);
        assert!(result.is_ok());
        match cmd {
            WorkflowCommand::Simple(s) => assert_eq!(s, "claude: /refactor main.rs"),
            _ => panic!("Expected Simple command"),
        }
    }
}
