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

    /// Parse imports from a JavaScript/TypeScript file
    #[allow(dead_code)]
    fn parse_js_imports(&self, content: &str) -> Vec<String> {
        let mut imports = Vec::new();

        // ES6 imports
        let es6_import_re = regex::Regex::new(
            r#"import\s+(?:(?:\{[^}]*\}|\*\s+as\s+\w+|\w+)\s+from\s+)?['"]([^'"]+)['"]"#,
        )
        .unwrap();
        for cap in es6_import_re.captures_iter(content) {
            if let Some(path) = cap.get(1) {
                imports.push(path.as_str().to_string());
            }
        }

        // CommonJS requires
        let require_re = regex::Regex::new(r#"require\s*\(['"]([^'"]+)['"]\)"#).unwrap();
        for cap in require_re.captures_iter(content) {
            if let Some(path) = cap.get(1) {
                imports.push(path.as_str().to_string());
            }
        }

        imports
    }

    /// Parse imports from a Python file
    #[allow(dead_code)]
    fn parse_python_imports(&self, content: &str) -> Vec<String> {
        let mut imports = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("import ") {
                if let Some(module) = line.strip_prefix("import ") {
                    imports.push(module.split_whitespace().next().unwrap_or("").to_string());
                }
            } else if line.starts_with("from ") {
                if let Some(rest) = line.strip_prefix("from ") {
                    if let Some(module) = rest.split(" import").next() {
                        imports.push(module.trim().to_string());
                    }
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
                // Don't filter out the root directory
                e.depth() == 0 || (!name.starts_with('.') && name != "target" && name != "node_modules")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_import_parsing() {
        let analyzer = BasicDependencyAnalyzer::new();

        let content = r#"
            use std::path::Path;
            use std::collections::{HashMap, HashSet};
            use crate::context::AnalysisResult;
            use super::*;
            mod submodule;
            pub mod public_module;
        "#;

        let imports = analyzer.parse_rust_imports(content);
        assert_eq!(imports.len(), 5);
        assert!(imports.contains(&"std::path::Path".to_string()));
        assert!(imports.contains(&"std::collections::{HashMap, HashSet}".to_string()));
        assert!(imports.contains(&"crate::context::AnalysisResult".to_string()));
        assert!(imports.contains(&"super::*".to_string()));
        assert!(imports.contains(&"mod::submodule".to_string()));
    }

    #[test]
    fn test_js_import_parsing() {
        let analyzer = BasicDependencyAnalyzer::new();

        let content = r#"
            import React from 'react';
            import { useState, useEffect } from 'react';
            import * as utils from './utils';
            const fs = require('fs');
            const { readFile } = require('fs/promises');
            import './styles.css';
        "#;

        let imports = analyzer.parse_js_imports(content);
        assert_eq!(imports.len(), 6);
        assert!(imports.contains(&"react".to_string()));
        assert!(imports.contains(&"./utils".to_string()));
        assert!(imports.contains(&"fs".to_string()));
        assert!(imports.contains(&"fs/promises".to_string()));
        assert!(imports.contains(&"./styles.css".to_string()));
    }

    #[test]
    fn test_python_import_parsing() {
        let analyzer = BasicDependencyAnalyzer::new();

        let content = r#"
            import os
            import sys
            from pathlib import Path
            from typing import List, Dict
            from ..utils import helper
            import numpy as np
        "#;

        let imports = analyzer.parse_python_imports(content);
        assert_eq!(imports.len(), 6);
        assert!(imports.contains(&"os".to_string()));
        assert!(imports.contains(&"sys".to_string()));
        assert!(imports.contains(&"pathlib".to_string()));
        assert!(imports.contains(&"typing".to_string()));
        assert!(imports.contains(&"..utils".to_string()));
        assert!(imports.contains(&"numpy".to_string()));
    }

    #[test]
    fn test_module_type_detection() {
        let analyzer = BasicDependencyAnalyzer::new();

        assert!(matches!(
            analyzer.get_module_type(Path::new("src/main.rs")),
            ModuleType::Binary
        ));
        assert!(matches!(
            analyzer.get_module_type(Path::new("tests/integration_test.rs")),
            ModuleType::Test
        ));
        assert!(matches!(
            analyzer.get_module_type(Path::new("build.rs")),
            ModuleType::Build
        ));
        assert!(matches!(
            analyzer.get_module_type(Path::new("Cargo.toml")),
            ModuleType::Config
        ));
        assert!(matches!(
            analyzer.get_module_type(Path::new("src/lib.rs")),
            ModuleType::Library
        ));
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = DependencyGraph {
            nodes: HashMap::new(),
            edges: vec![
                DependencyEdge {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    dep_type: DependencyType::Import,
                },
                DependencyEdge {
                    from: "B".to_string(),
                    to: "C".to_string(),
                    dep_type: DependencyType::Import,
                },
                DependencyEdge {
                    from: "C".to_string(),
                    to: "A".to_string(),
                    dep_type: DependencyType::Import,
                },
            ],
            cycles: vec![],
            layers: vec![],
        };

        // Add nodes
        graph.nodes.insert(
            "A".to_string(),
            ModuleNode {
                path: "A".to_string(),
                module_type: ModuleType::Library,
                imports: vec!["B".to_string()],
                exports: vec![],
                external_deps: vec![],
            },
        );
        graph.nodes.insert(
            "B".to_string(),
            ModuleNode {
                path: "B".to_string(),
                module_type: ModuleType::Library,
                imports: vec!["C".to_string()],
                exports: vec![],
                external_deps: vec![],
            },
        );
        graph.nodes.insert(
            "C".to_string(),
            ModuleNode {
                path: "C".to_string(),
                module_type: ModuleType::Library,
                imports: vec!["A".to_string()],
                exports: vec![],
                external_deps: vec![],
            },
        );

        graph.detect_cycles();

        assert_eq!(graph.cycles.len(), 1);
        assert_eq!(graph.cycles[0].len(), 3);
        assert!(graph.cycles[0].contains(&"A".to_string()));
        assert!(graph.cycles[0].contains(&"B".to_string()));
        assert!(graph.cycles[0].contains(&"C".to_string()));
    }

    #[test]
    fn test_layer_detection() {
        let analyzer = BasicDependencyAnalyzer::new();
        let mut graph = DependencyGraph {
            nodes: HashMap::new(),
            edges: vec![],
            cycles: vec![],
            layers: vec![],
        };

        // Add nodes at different depths
        graph.nodes.insert(
            "main.rs".to_string(),
            ModuleNode {
                path: "main.rs".to_string(),
                module_type: ModuleType::Binary,
                imports: vec![],
                exports: vec![],
                external_deps: vec![],
            },
        );
        graph.nodes.insert(
            "src/lib.rs".to_string(),
            ModuleNode {
                path: "src/lib.rs".to_string(),
                module_type: ModuleType::Library,
                imports: vec![],
                exports: vec![],
                external_deps: vec![],
            },
        );
        graph.nodes.insert(
            "src/context/mod.rs".to_string(),
            ModuleNode {
                path: "src/context/mod.rs".to_string(),
                module_type: ModuleType::Library,
                imports: vec![],
                exports: vec![],
                external_deps: vec![],
            },
        );

        let layers = analyzer.detect_layers(&graph);

        assert!(layers.len() >= 2);
        assert_eq!(layers[0].name, "root");
        assert_eq!(layers[0].level, 0);
        assert!(layers[0].modules.contains(&"main.rs".to_string()));
    }
}
