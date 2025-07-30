//! Architecture extraction and pattern detection

use super::*;
use crate::context::dependencies::ArchitecturalLayer;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Trait for architecture extraction
#[async_trait::async_trait]
pub trait ArchitectureExtractor: Send + Sync {
    /// Extract architecture information from the project
    async fn extract_architecture(&self, project_path: &Path) -> Result<ArchitectureInfo>;

    /// Update architecture based on changed files
    async fn update_architecture(
        &self,
        project_path: &Path,
        current: &ArchitectureInfo,
        changed_files: &[PathBuf],
    ) -> Result<ArchitectureInfo>;
}

/// Basic architecture extractor implementation
pub struct BasicArchitectureExtractor;

impl Default for BasicArchitectureExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicArchitectureExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Wrapper for extract_components for tests
    pub fn detect_components(&self, project_path: &Path) -> HashMap<String, ComponentInfo> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.extract_components(project_path))
            .unwrap_or_default()
    }

    /// Check violations with dependency graph (for tests)
    pub fn check_violations_graph(
        &self,
        graph: &crate::context::dependencies::DependencyGraph,
    ) -> Vec<ArchitectureViolation> {
        let mut violations = Vec::new();

        // Check for god components (too many dependencies)
        let mut dep_counts: HashMap<String, usize> = HashMap::new();
        for edge in &graph.edges {
            *dep_counts.entry(edge.from.clone()).or_insert(0) += 1;
        }

        for (component, count) in dep_counts {
            if count > 10 {
                violations.push(ArchitectureViolation {
                    rule: "Avoid god components".to_string(),
                    location: component.clone(),
                    severity: ViolationSeverity::High,
                    description: format!(
                        "{component} has {count} dependencies, consider splitting"
                    ),
                });
            }
        }

        violations
    }

    /// Detect architectural patterns from project structure
    fn detect_patterns(&self, project_path: &Path) -> Vec<String> {
        let mut patterns = Vec::new();

        // Check for common architectural patterns
        if project_path.join("src/controllers").exists() || project_path.join("src/views").exists()
        {
            patterns.push("MVC".to_string());
        }

        if project_path.join("src/handlers").exists() || project_path.join("src/routes").exists() {
            patterns.push("REST API".to_string());
        }

        if project_path.join("src/domain").exists()
            && project_path.join("src/infrastructure").exists()
        {
            patterns.push("Domain-Driven Design".to_string());
        }

        if project_path.join("src/components").exists() {
            patterns.push("Component-Based".to_string());
        }

        if project_path.join("src/services").exists() {
            patterns.push("Service-Oriented".to_string());
        }

        if patterns.is_empty() {
            patterns.push("Modular".to_string());
        }

        patterns
    }

    /// Extract components from project structure
    async fn extract_components(
        &self,
        project_path: &Path,
    ) -> Result<HashMap<String, ComponentInfo>> {
        let mut components = HashMap::new();
        let src_path = project_path.join("src");

        if !src_path.exists() {
            return Ok(components);
        }

        // Find top-level modules/components
        for entry in std::fs::read_dir(&src_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Determine component responsibility based on name
                let responsibility = match name.as_str() {
                    "api" | "handlers" | "routes" => "API endpoints and request handling",
                    "models" | "domain" | "entities" => "Business entities and domain logic",
                    "services" | "business" => "Business logic and services",
                    "db" | "database" | "persistence" => "Data persistence and storage",
                    "utils" | "helpers" | "common" => "Shared utilities and helpers",
                    "config" => "Configuration management",
                    "auth" | "security" => "Authentication and authorization",
                    "middleware" => "Request/response middleware",
                    _ => "Module functionality",
                }
                .to_string();

                // Find interfaces (public functions/types)
                let interfaces = self.extract_interfaces(&path).await?;

                // Find dependencies
                let dependencies = self.find_component_dependencies(&path).await?;

                components.insert(
                    name.clone(),
                    ComponentInfo {
                        name: name.clone(),
                        responsibility,
                        interfaces,
                        dependencies,
                    },
                );
            }
        }

        Ok(components)
    }

    /// Extract public interfaces from a component
    async fn extract_interfaces(&self, component_path: &Path) -> Result<Vec<String>> {
        let mut interfaces = Vec::new();

        // Look for mod.rs or lib.rs
        let mod_file = component_path.join("mod.rs");
        let lib_file = component_path.join("lib.rs");

        let file_to_read = if mod_file.exists() {
            mod_file
        } else if lib_file.exists() {
            lib_file
        } else {
            return Ok(interfaces);
        };

        let content = tokio::fs::read_to_string(&file_to_read).await?;

        // Parse public items
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("pub fn") || line.starts_with("pub async fn") {
                if let Some(name) = line.split_whitespace().nth(2) {
                    interfaces.push(format!("fn {}", name.trim_end_matches('(')));
                }
            } else if line.starts_with("pub struct") {
                if let Some(name) = line.split_whitespace().nth(2) {
                    interfaces.push(format!("struct {}", name.trim_end_matches(['{', ';'])));
                }
            } else if line.starts_with("pub trait") {
                if let Some(name) = line.split_whitespace().nth(2) {
                    interfaces.push(format!("trait {}", name.trim_end_matches('{')));
                }
            }
        }

        Ok(interfaces)
    }

    /// Find dependencies of a component
    async fn find_component_dependencies(&self, component_path: &Path) -> Result<Vec<String>> {
        use walkdir::WalkDir;

        let mut dependencies = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for entry in WalkDir::new(component_path)
            .max_depth(3)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        {
            let content = tokio::fs::read_to_string(entry.path()).await?;

            // Extract crate-level dependencies
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("use ") {
                    if let Some(dep) = line.strip_prefix("use ").and_then(|s| s.split("::").next())
                    {
                        let dep = dep.trim();
                        if dep != "crate"
                            && dep != "super"
                            && dep != "self"
                            && dep != "std"
                            && seen.insert(dep.to_string())
                        {
                            dependencies.push(dep.to_string());
                        }
                    }
                }
            }
        }

        Ok(dependencies)
    }

    /// Check for architecture violations
    fn check_violations(
        &self,
        components: &HashMap<String, ComponentInfo>,
    ) -> Vec<ArchitectureViolation> {
        let mut violations = Vec::new();

        // Check for circular dependencies between components
        for (name, component) in components {
            for dep in &component.dependencies {
                if let Some(dep_component) = components.get(dep) {
                    if dep_component.dependencies.contains(name) {
                        violations.push(ArchitectureViolation {
                            rule: "No circular dependencies between components".to_string(),
                            location: name.clone(),
                            severity: ViolationSeverity::High,
                            description: format!("Circular dependency between {name} and {dep}"),
                        });
                    }
                }
            }
        }

        // Check for common anti-patterns
        for (name, component) in components {
            // Check for god components (too many dependencies)
            if component.dependencies.len() > 10 {
                violations.push(ArchitectureViolation {
                    rule: "Avoid god components".to_string(),
                    location: name.clone(),
                    severity: ViolationSeverity::Medium,
                    description: format!(
                        "{} has {} dependencies, consider splitting",
                        name,
                        component.dependencies.len()
                    ),
                });
            }

            // Check for anemic components (no interfaces)
            if component.interfaces.is_empty() && name != "utils" && name != "common" {
                violations.push(ArchitectureViolation {
                    rule: "Components should expose interfaces".to_string(),
                    location: name.clone(),
                    severity: ViolationSeverity::Low,
                    description: format!("{name} has no public interfaces"),
                });
            }
        }

        violations
    }
}

#[async_trait::async_trait]
impl ArchitectureExtractor for BasicArchitectureExtractor {
    async fn extract_architecture(&self, project_path: &Path) -> Result<ArchitectureInfo> {
        let patterns = self.detect_patterns(project_path);
        let components = self.extract_components(project_path).await?;
        let violations = self.check_violations(&components);

        // Create architectural layers based on components
        let mut layers = Vec::new();

        // Presentation layer
        let presentation_components: Vec<_> = components
            .keys()
            .filter(|name| {
                name.contains("api")
                    || name.contains("handlers")
                    || name.contains("routes")
                    || name.contains("controllers")
                    || name.contains("views")
            })
            .cloned()
            .collect();

        if !presentation_components.is_empty() {
            layers.push(ArchitecturalLayer {
                name: "Presentation".to_string(),
                level: 0,
                modules: presentation_components,
            });
        }

        // Business layer
        let business_components: Vec<_> = components
            .keys()
            .filter(|name| {
                name.contains("services")
                    || name.contains("business")
                    || name.contains("domain")
                    || name.contains("use_cases")
            })
            .cloned()
            .collect();

        if !business_components.is_empty() {
            layers.push(ArchitecturalLayer {
                name: "Business".to_string(),
                level: 1,
                modules: business_components,
            });
        }

        // Data layer
        let data_components: Vec<_> = components
            .keys()
            .filter(|name| {
                name.contains("db")
                    || name.contains("database")
                    || name.contains("persistence")
                    || name.contains("repository")
            })
            .cloned()
            .collect();

        if !data_components.is_empty() {
            layers.push(ArchitecturalLayer {
                name: "Data".to_string(),
                level: 2,
                modules: data_components,
            });
        }

        Ok(ArchitectureInfo {
            patterns,
            layers,
            components,
            violations,
        })
    }

    async fn update_architecture(
        &self,
        project_path: &Path,
        current: &ArchitectureInfo,
        changed_files: &[PathBuf],
    ) -> Result<ArchitectureInfo> {
        // For now, just re-extract if any architectural files changed
        let architectural_changes = changed_files.iter().any(|f| {
            let path_str = f.to_string_lossy();
            path_str.contains("/mod.rs")
                || path_str.contains("/lib.rs")
                || path_str.ends_with("Cargo.toml")
        });

        if architectural_changes {
            self.extract_architecture(project_path).await
        } else {
            Ok(current.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_pattern_detection() {
        let extractor = BasicArchitectureExtractor::new();
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // Create MVC structure
        fs::create_dir_all(project_path.join("src/controllers")).unwrap();
        fs::create_dir_all(project_path.join("src/views")).unwrap();
        fs::create_dir_all(project_path.join("src/models")).unwrap();

        let patterns = extractor.detect_patterns(project_path);
        assert!(patterns.contains(&"MVC".to_string()));

        // Create REST API structure
        fs::create_dir_all(project_path.join("src/handlers")).unwrap();
        fs::create_dir_all(project_path.join("src/routes")).unwrap();

        let patterns = extractor.detect_patterns(project_path);
        assert!(patterns.contains(&"REST API".to_string()));
    }

    #[test]
    fn test_component_detection() {
        let extractor = BasicArchitectureExtractor::new();
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // Create component structure
        fs::create_dir_all(project_path.join("src/auth")).unwrap();
        fs::create_dir_all(project_path.join("src/database")).unwrap();
        fs::create_dir_all(project_path.join("src/api")).unwrap();

        let components = extractor.detect_components(project_path);
        assert_eq!(components.len(), 3);
        assert!(components.contains_key("auth"));
        assert!(components.contains_key("database"));
        assert!(components.contains_key("api"));
    }

    #[test]
    fn test_violation_detection_god_component() {
        let extractor = BasicArchitectureExtractor::new();

        // Create a fake dependency graph with a god component
        let mut graph = crate::context::dependencies::DependencyGraph {
            nodes: HashMap::new(),
            edges: vec![],
            cycles: vec![],
            layers: vec![],
        };

        // Add a component with many dependencies
        for i in 0..15 {
            graph
                .edges
                .push(crate::context::dependencies::DependencyEdge {
                    from: "god_component".to_string(),
                    to: format!("dep_{i}"),
                    dep_type: crate::context::dependencies::DependencyType::Import,
                });
        }

        let violations = extractor.check_violations_graph(&graph);
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.rule == "Avoid god components"));
    }

    #[test]
    fn test_violation_detection_no_interfaces() {
        let extractor = BasicArchitectureExtractor::new();

        // Create components with no public interfaces
        let mut components = HashMap::new();
        components.insert(
            "isolated_component".to_string(),
            ComponentInfo {
                name: "isolated_component".to_string(),
                responsibility: "Does something".to_string(),
                interfaces: vec![],
                dependencies: vec!["other".to_string()],
            },
        );

        let graph = crate::context::dependencies::DependencyGraph {
            nodes: HashMap::new(),
            edges: vec![],
            cycles: vec![],
            layers: vec![],
        };

        let violations = extractor.check_violations_with_components(&graph, &components);
        assert!(violations
            .iter()
            .any(|v| v.rule == "Components should expose interfaces"
                && v.location.contains("isolated_component")));
    }

    #[tokio::test]
    async fn test_extract_architecture() {
        let extractor = BasicArchitectureExtractor::new();
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // Create a basic project structure
        fs::create_dir_all(project_path.join("src")).unwrap();
        fs::write(project_path.join("src/lib.rs"), "pub mod auth;").unwrap();
        fs::create_dir_all(project_path.join("src/auth")).unwrap();
        fs::write(
            project_path.join("src/auth/mod.rs"),
            "pub fn authenticate() {}",
        )
        .unwrap();

        let architecture = extractor.extract_architecture(project_path).await.unwrap();

        assert!(!architecture.patterns.is_empty());
        assert!(architecture.patterns.contains(&"Modular".to_string()));
        assert!(architecture.components.contains_key("auth"));
    }

    // Helper for violation tests
    impl BasicArchitectureExtractor {
        fn check_violations_with_components(
            &self,
            _graph: &crate::context::dependencies::DependencyGraph,
            components: &HashMap<String, ComponentInfo>,
        ) -> Vec<ArchitectureViolation> {
            let mut violations = Vec::new();

            // Check for components with no interfaces
            for (name, component) in components {
                if component.interfaces.is_empty() && !component.dependencies.is_empty() {
                    violations.push(ArchitectureViolation {
                        rule: "Components should expose interfaces".to_string(),
                        location: name.clone(),
                        severity: ViolationSeverity::Medium,
                        description: format!("{name} has dependencies but no public interfaces"),
                    });
                }
            }

            violations
        }
    }
}
