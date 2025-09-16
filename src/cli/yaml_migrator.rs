//! YAML workflow migrator to convert from nested to simplified syntax

use anyhow::{Context, Result};
use serde_yaml::{Mapping, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub struct YamlMigrator {
    create_backup: bool,
}

#[derive(Debug)]
pub struct MigrationResult {
    pub file: PathBuf,
    pub was_migrated: bool,
    pub error: Option<String>,
}

impl YamlMigrator {
    pub fn new(create_backup: bool) -> Self {
        Self { create_backup }
    }

    /// Migrate a single YAML file
    pub fn migrate_file(&self, path: &Path, dry_run: bool) -> Result<MigrationResult> {
        // Read the file
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Parse YAML
        let mut yaml: Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML: {}", path.display()))?;

        // Check if it's a MapReduce workflow
        let mut was_migrated = false;
        if let Value::Mapping(ref mut root) = yaml {
            if let Some(Value::String(mode)) = root.get("mode") {
                if mode == "mapreduce" {
                    was_migrated = self.migrate_mapreduce_workflow(root)?;
                }
            }
        }

        // Check if it's a regular workflow (array of steps)
        if let Value::Sequence(_) = yaml {
            // Regular workflows already use simplified syntax
            was_migrated = false;
        }

        if was_migrated && !dry_run {
            // Create backup if requested
            if self.create_backup {
                let backup_path = path.with_extension("yml.bak");
                fs::copy(path, &backup_path).with_context(|| {
                    format!("Failed to create backup: {}", backup_path.display())
                })?;
            }

            // Write migrated content
            let migrated_content = serde_yaml::to_string(&yaml)?;
            fs::write(path, migrated_content)
                .with_context(|| format!("Failed to write migrated file: {}", path.display()))?;
        }

        Ok(MigrationResult {
            file: path.to_path_buf(),
            was_migrated,
            error: None,
        })
    }

    /// Migrate all YAML files in a directory
    pub fn migrate_directory(&self, dir: &Path, dry_run: bool) -> Result<Vec<MigrationResult>> {
        let mut results = Vec::new();

        // Find all .yml and .yaml files
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yml")
                || path.extension().and_then(|s| s.to_str()) == Some("yaml")
            {
                match self.migrate_file(&path, dry_run) {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        results.push(MigrationResult {
                            file: path.clone(),
                            was_migrated: false,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Migrate a MapReduce workflow from nested to simplified syntax
    fn migrate_mapreduce_workflow(&self, workflow: &mut Mapping) -> Result<bool> {
        let mut was_migrated = false;

        // Migrate map.agent_template.commands -> map.agent_template
        if let Some(Value::Mapping(map)) = workflow.get_mut("map") {
            // Check if agent_template has nested commands
            let needs_migration =
                if let Some(Value::Mapping(agent_template)) = map.get("agent_template") {
                    agent_template.contains_key("commands")
                } else {
                    false
                };

            if needs_migration {
                // Extract and migrate the commands
                if let Some(Value::Mapping(mut agent_template)) = map.remove("agent_template") {
                    if let Some(commands) = agent_template.remove("commands") {
                        // Put commands directly as agent_template
                        map.insert("agent_template".into(), commands);
                        was_migrated = true;
                    }
                }
            }

            // Remove deprecated parameters
            if map.remove("timeout_per_agent").is_some() {
                was_migrated = true;
            }
            if map.remove("retry_on_failure").is_some() {
                was_migrated = true;
            }
        }

        // Migrate reduce.commands -> reduce (direct array)
        if let Some(Value::Mapping(ref mut reduce)) = workflow.get_mut("reduce") {
            if let Some(commands) = reduce.remove("commands") {
                // Replace the reduce mapping with the commands array directly
                workflow.insert("reduce".into(), commands);
                was_migrated = true;
            }
        }

        // Migrate on_failure sections to remove deprecated parameters
        let mut workflow_value = Value::Mapping(workflow.clone());
        self.migrate_on_failure_recursive(&mut workflow_value)?;
        if let Value::Mapping(updated) = workflow_value {
            *workflow = updated;
            was_migrated = true;
        }

        Ok(was_migrated)
    }

    /// Recursively migrate on_failure sections
    fn migrate_on_failure_recursive(&self, value: &mut Value) -> Result<()> {
        Self::migrate_on_failure_recursive_impl(value)
    }

    fn migrate_on_failure_recursive_impl(value: &mut Value) -> Result<()> {
        match value {
            Value::Mapping(map) => {
                // Check for on_failure
                if let Some(Value::Mapping(ref mut on_failure)) = map.get_mut("on_failure") {
                    // Remove deprecated parameters
                    on_failure.remove("max_attempts");
                    on_failure.remove("fail_workflow");
                }

                // Recurse into all values
                for (_key, val) in map.iter_mut() {
                    Self::migrate_on_failure_recursive_impl(val)?;
                }
            }
            Value::Sequence(seq) => {
                // Recurse into all items
                for item in seq.iter_mut() {
                    Self::migrate_on_failure_recursive_impl(item)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
