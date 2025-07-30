//! Dependency graph analysis for understanding module relationships

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Trait for dependency analysis
#[async_trait::async_trait]
pub trait DependencyAnalyzer: Send + Sync {
    /// Analyze dependencies in the project
    async fn analyze_dependencies(&self, project_path: &Path) -> Result<DependencyGraph>;

    /// Update dependencies based on changed files
    async fn update_dependencies(
        &self,
        project_path: &Path,
        current: &DependencyGraph,
        changed_files: &[PathBuf],
    ) -> Result<DependencyGraph>;
}

/// Dependency graph representing module relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub nodes: HashMap<String, ModuleNode>,
    pub edges: Vec<DependencyEdge>,
    pub cycles: Vec<Vec<String>>,
    pub layers: Vec<ArchitecturalLayer>,
}

/// A module/file in the dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleNode {
    pub path: String,
    pub module_type: ModuleType,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub external_deps: Vec<String>,
}

/// Type of module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModuleType {
    Library,
    Binary,
    Test,
    Build,
    Config,
}

/// Edge in the dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    pub dep_type: DependencyType,
}

/// Type of dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    Import,
    Export,
    Test,
    Build,
}

/// Architectural layer in the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitecturalLayer {
    pub name: String,
    pub level: u32,
    pub modules: Vec<String>,
}

impl DependencyGraph {
    /// Get dependencies for a specific file
    pub fn get_file_dependencies(&self, file: &Path) -> Vec<String> {
        let path = file.to_string_lossy().to_string();
        if let Some(node) = self.nodes.get(&path) {
            node.imports.clone()
        } else {
            Vec::new()
        }
    }

    /// Get modules with high coupling
    pub fn get_coupling_hotspots(&self) -> Vec<(&str, usize)> {
        let mut coupling_counts: HashMap<&str, usize> = HashMap::new();

        for edge in &self.edges {
            *coupling_counts.entry(&edge.from).or_insert(0) += 1;
        }

        let mut hotspots: Vec<_> = coupling_counts.into_iter().collect();
        hotspots.sort_by(|a, b| b.1.cmp(&a.1));
        hotspots
    }

    /// Detect circular dependencies using DFS
    fn detect_cycles(&mut self) {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut cycles = Vec::new();

        for node in self.nodes.keys() {
            if !visited.contains(node) {
                let mut path = Vec::new();
                self.dfs_cycles(node, &mut visited, &mut rec_stack, &mut path, &mut cycles);
            }
        }

        self.cycles = cycles;
    }

    fn dfs_cycles(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        path.push(node.to_string());

        // Get neighbors
        let neighbors: Vec<String> = self
            .edges
            .iter()
            .filter(|e| e.from == node)
            .map(|e| e.to.clone())
            .collect();

        for neighbor in neighbors {
            if !visited.contains(&neighbor) {
                self.dfs_cycles(&neighbor, visited, rec_stack, path, cycles);
            } else if rec_stack.contains(&neighbor) {
                // Found a cycle
                if let Some(start_idx) = path.iter().position(|n| n == &neighbor) {
                    let cycle: Vec<String> = path[start_idx..].to_vec();
                    cycles.push(cycle);
                }
            }
        }

        path.pop();
        rec_stack.remove(node);
    }
}

/// Basic dependency analyzer implementation
pub struct BasicDependencyAnalyzer;

impl Default for BasicDependencyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicDependencyAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Parse imports from a Rust file
    fn parse_rust_imports(&self, content: &str) -> Vec<String> {
        let mut imports = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("use ") {
                if let Some(import) = line.strip_prefix("use ").and_then(|s| s.split(';').next()) {
                    imports.push(import.trim().to_string());
                }
            } else if line.starts_with("mod ") {
                if let Some(module) = line.strip_prefix("mod ").and_then(|s| s.split(';').next()) {
                    imports.push(format!("mod::{}", module.trim()));
                }
            }
        }

        imports
    }

    /// Determine module type from path
    fn get_module_type(&self, path: &Path) -> ModuleType {
        let path_str = path.to_string_lossy();

        if path_str.contains("/tests/") || path_str.ends_with("_test.rs") {
            ModuleType::Test
        } else if path_str.contains("/bin/") || path_str == "src/main.rs" {
            ModuleType::Binary
        } else if path_str.contains("build.rs") {
            ModuleType::Build
        } else if path_str.ends_with(".toml") || path_str.ends_with(".yaml") {
            ModuleType::Config
        } else {
            ModuleType::Library
        }
    }
}

#[async_trait::async_trait]
impl DependencyAnalyzer for BasicDependencyAnalyzer {
    async fn analyze_dependencies(&self, project_path: &Path) -> Result<DependencyGraph> {
        use walkdir::WalkDir;

        let mut nodes = HashMap::new();
        let mut edges = Vec::new();

        // Walk through all source files
        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "target" && name != "node_modules"
            })
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let relative_path = path.strip_prefix(project_path).unwrap_or(path);

            // Process Rust files
            if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                if let Ok(content) = tokio::fs::read_to_string(path).await {
                    let imports = self.parse_rust_imports(&content);
                    let module_type = self.get_module_type(relative_path);

                    let node = ModuleNode {
                        path: relative_path.to_string_lossy().to_string(),
                        module_type,
                        imports: imports.clone(),
                        exports: Vec::new(),       // TODO: Parse exports
                        external_deps: Vec::new(), // TODO: Parse Cargo.toml
                    };

                    // Create edges for imports
                    for import in &imports {
                        edges.push(DependencyEdge {
                            from: relative_path.to_string_lossy().to_string(),
                            to: import.clone(),
                            dep_type: DependencyType::Import,
                        });
                    }

                    nodes.insert(relative_path.to_string_lossy().to_string(), node);
                }
            }
        }

        // Create dependency graph
        let mut graph = DependencyGraph {
            nodes,
            edges,
            cycles: Vec::new(),
            layers: Vec::new(),
        };

        // Detect cycles
        graph.detect_cycles();

        // Detect architectural layers
        graph.layers = self.detect_layers(&graph);

        Ok(graph)
    }

    async fn update_dependencies(
        &self,
        project_path: &Path,
        current: &DependencyGraph,
        changed_files: &[PathBuf],
    ) -> Result<DependencyGraph> {
        let mut graph = current.clone();

        // Re-analyze changed files
        for file in changed_files {
            if file.extension().and_then(|s| s.to_str()) == Some("rs") {
                let full_path = project_path.join(file);
                if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                    let imports = self.parse_rust_imports(&content);
                    let module_type = self.get_module_type(file);

                    let node = ModuleNode {
                        path: file.to_string_lossy().to_string(),
                        module_type,
                        imports: imports.clone(),
                        exports: Vec::new(),
                        external_deps: Vec::new(),
                    };

                    // Remove old edges for this file
                    graph.edges.retain(|e| e.from != file.to_string_lossy());

                    // Add new edges
                    for import in &imports {
                        graph.edges.push(DependencyEdge {
                            from: file.to_string_lossy().to_string(),
                            to: import.clone(),
                            dep_type: DependencyType::Import,
                        });
                    }

                    graph.nodes.insert(file.to_string_lossy().to_string(), node);
                }
            }
        }

        // Re-detect cycles
        graph.detect_cycles();

        // Re-detect layers
        graph.layers = self.detect_layers(&graph);

        Ok(graph)
    }
}

impl BasicDependencyAnalyzer {
    /// Detect architectural layers based on directory structure
    fn detect_layers(&self, graph: &DependencyGraph) -> Vec<ArchitecturalLayer> {
        let mut layers = Vec::new();
        let mut layer_map: HashMap<String, Vec<String>> = HashMap::new();

        // Group modules by directory depth
        for path in graph.nodes.keys() {
            let depth = path.matches('/').count();
            let layer_name = match depth {
                0 => "root",
                1 => "top-level",
                2 => "module",
                _ => "deep",
            };

            layer_map
                .entry(layer_name.to_string())
                .or_default()
                .push(path.clone());
        }

        // Create layers
        let layer_order = ["root", "top-level", "module", "deep"];
        for (level, name) in layer_order.iter().enumerate() {
            if let Some(modules) = layer_map.get(*name) {
                layers.push(ArchitecturalLayer {
                    name: name.to_string(),
                    level: level as u32,
                    modules: modules.clone(),
                });
            }
        }

        layers
    }
}
