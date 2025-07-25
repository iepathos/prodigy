use super::{SpecMetadata, Specification};
use crate::{Error, Result};
use gray_matter::engine::YAML;
use gray_matter::Matter;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Deserialize, Default)]
struct SpecFrontmatter {
    id: Option<String>,
    name: Option<String>,
    objective: Option<String>,
    acceptance_criteria: Option<Vec<String>>,
    dependencies: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    priority: Option<u32>,
    estimated_hours: Option<f32>,
}

pub struct SpecParser {
    matter: Matter<YAML>,
}

impl SpecParser {
    pub fn new() -> Self {
        Self {
            matter: Matter::<YAML>::new(),
        }
    }

    pub async fn parse_file(&self, path: &Path) -> Result<Specification> {
        let content = fs::read_to_string(path).await?;
        self.parse_content(&content, path)
    }

    pub fn parse_content(&self, content: &str, path: &Path) -> Result<Specification> {
        let parsed = self.matter.parse(content);

        // Extract frontmatter directly from YAML string if present
        let frontmatter: SpecFrontmatter = if parsed.data.is_some() {
            // The gray_matter library includes the frontmatter in the original content
            // We can parse it directly from the YAML section
            if let Some(yaml_end) = content.find("---\n") {
                if let Some(yaml_start) = content[yaml_end + 4..].find("---\n") {
                    let yaml_content = &content[yaml_end + 4..yaml_end + 4 + yaml_start];
                    serde_yaml::from_str(yaml_content).unwrap_or_default()
                } else {
                    SpecFrontmatter::default()
                }
            } else {
                SpecFrontmatter::default()
            }
        } else {
            SpecFrontmatter::default()
        };

        let id = frontmatter.id.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        let name = frontmatter.name.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unnamed Specification")
                .to_string()
        });

        let mut spec = Specification::new(id, name, parsed.content);

        spec.metadata = SpecMetadata {
            objective: frontmatter.objective,
            acceptance_criteria: frontmatter.acceptance_criteria.unwrap_or_default(),
            tags: frontmatter.tags.unwrap_or_default(),
            priority: frontmatter.priority,
            estimated_hours: frontmatter.estimated_hours,
        };

        spec.dependencies = frontmatter.dependencies.unwrap_or_default();

        spec.validate()?;
        Ok(spec)
    }

    pub fn extract_sections(&self, content: &str) -> HashMap<String, String> {
        let mut sections = HashMap::new();
        let mut current_section = String::new();
        let mut current_content = String::new();

        for line in content.lines() {
            if line.starts_with("## ") {
                if !current_section.is_empty() {
                    sections.insert(current_section.clone(), current_content.trim().to_string());
                }
                current_section = line[3..].to_string();
                current_content = String::new();
            } else {
                current_content.push_str(line);
                current_content.push('\n');
            }
        }

        if !current_section.is_empty() {
            sections.insert(current_section, current_content.trim().to_string());
        }

        sections
    }
}

use std::collections::HashMap;
