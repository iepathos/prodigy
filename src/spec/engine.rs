use super::{SpecParser, SpecStatus, Specification};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct SpecificationEngine {
    parser: SpecParser,
    specifications: HashMap<String, Specification>,
    spec_dir: PathBuf,
}

impl SpecificationEngine {
    pub fn new(spec_dir: PathBuf) -> Self {
        Self {
            parser: SpecParser::new(),
            specifications: HashMap::new(),
            spec_dir,
        }
    }

    pub async fn load_specifications(&mut self) -> Result<()> {
        if !self.spec_dir.exists() {
            return Err(Error::Spec(format!(
                "Specification directory not found: {}",
                self.spec_dir.display()
            )));
        }

        for entry in WalkDir::new(&self.spec_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                match self.parser.parse_file(path).await {
                    Ok(spec) => {
                        self.specifications.insert(spec.id.clone(), spec);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse specification {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_specification(&self, id: &str) -> Option<&Specification> {
        self.specifications.get(id)
    }

    pub fn get_ready_specifications(&self) -> Vec<&Specification> {
        let completed: Vec<String> = self
            .specifications
            .values()
            .filter(|s| s.status == SpecStatus::Completed)
            .map(|s| s.id.clone())
            .collect();

        self.specifications
            .values()
            .filter(|s| s.status == SpecStatus::Pending && s.is_ready(&completed))
            .collect()
    }

    pub fn update_status(&mut self, id: &str, status: SpecStatus) -> Result<()> {
        match self.specifications.get_mut(id) {
            Some(spec) => {
                spec.status = status;
                Ok(())
            }
            None => Err(Error::Spec(format!("Specification '{id}' not found"))),
        }
    }

    pub fn get_dependency_graph(&self) -> HashMap<String, Vec<String>> {
        self.specifications
            .iter()
            .map(|(id, spec)| (id.clone(), spec.dependencies.clone()))
            .collect()
    }

    pub fn topological_sort(&self) -> Result<Vec<String>> {
        let mut graph = self.get_dependency_graph();
        let mut sorted = Vec::new();
        let mut no_deps = Vec::new();

        for (id, deps) in &graph {
            if deps.is_empty() {
                no_deps.push(id.clone());
            }
        }

        while let Some(node) = no_deps.pop() {
            sorted.push(node.clone());

            let mut to_update = Vec::new();
            for (id, deps) in &graph {
                if deps.contains(&node) {
                    to_update.push(id.clone());
                }
            }

            for id in to_update {
                if let Some(deps) = graph.get_mut(&id) {
                    deps.retain(|d| d != &node);
                    if deps.is_empty() {
                        no_deps.push(id);
                    }
                }
            }
        }

        if sorted.len() != self.specifications.len() {
            return Err(Error::Spec("Circular dependency detected".to_string()));
        }

        Ok(sorted)
    }

    pub async fn render_template(
        &self,
        template: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String> {
        let mut result = template.to_string();
        for (key, value) in variables {
            result = result.replace(&format!("{{{{{key}}}}}"), value);
        }
        Ok(result)
    }

    pub async fn list_specs(&self, _project_path: &Path) -> Result<Vec<SpecSummary>> {
        let specs: Vec<SpecSummary> = self
            .specifications
            .values()
            .map(|spec| SpecSummary {
                id: spec.id.clone(),
                name: spec.name.clone(),
                status: spec.status.status_string(),
                priority: spec.metadata.priority.unwrap_or(0),
            })
            .collect();

        Ok(specs)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecSummary {
    pub id: String,
    pub name: String,
    pub status: String,
    pub priority: u32,
}

impl SpecStatus {
    pub fn status_string(&self) -> String {
        match self {
            SpecStatus::Pending => "pending".to_string(),
            SpecStatus::InProgress => "in_progress".to_string(),
            SpecStatus::Completed => "completed".to_string(),
            SpecStatus::Failed => "failed".to_string(),
            SpecStatus::Blocked => "blocked".to_string(),
        }
    }
}
