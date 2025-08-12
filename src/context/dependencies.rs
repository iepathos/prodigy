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
        .expect("Failed to compile ES6 import regex");
        for cap in es6_import_re.captures_iter(content) {
            if let Some(path) = cap.get(1) {
                imports.push(path.as_str().to_string());
            }
        }

        // CommonJS requires
        let require_re = regex::Regex::new(r#"require\s*\(['"]([^'"]+)['"]\)"#)
            .expect("Failed to compile CommonJS require regex");
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
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
            || path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml"))
        {
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
                e.depth() == 0
                    || (!name.starts_with('.') && name != "target" && name != "node_modules")
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
                    let exports = self.parse_exports(&content, "rs");

                    let node = ModuleNode {
                        path: relative_path.to_string_lossy().to_string(),
                        module_type,
                        imports: imports.clone(),
                        exports,
                        external_deps: Vec::new(),
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

            // Process Cargo.toml to get external dependencies
            if path.file_name().and_then(|s| s.to_str()) == Some("Cargo.toml") {
                if let Ok(content) = tokio::fs::read_to_string(path).await {
                    let external_deps = self.parse_cargo_dependencies(&content);

                    // Store external dependencies in a special node
                    let node = ModuleNode {
                        path: relative_path.to_string_lossy().to_string(),
                        module_type: ModuleType::Config,
                        imports: Vec::new(),
                        exports: Vec::new(),
                        external_deps,
                    };

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

    /// Parse exports from a file based on its extension
    fn parse_exports(&self, content: &str, ext: &str) -> Vec<String> {
        match ext {
            "rs" => self.parse_rust_exports(content),
            "js" | "ts" | "jsx" | "tsx" => self.parse_js_exports(content),
            "py" => self.parse_python_exports(content),
            _ => Vec::new(),
        }
    }

    /// Parse Rust exports (public items)
    fn parse_rust_exports(&self, content: &str) -> Vec<String> {
        let mut exports = Vec::new();

        // Match public functions, structs, enums, traits, etc.
        let pub_re = regex::Regex::new(r"pub\s+(fn|struct|enum|trait|type|const|static)\s+(\w+)")
            .expect("Failed to compile Rust export regex");

        for cap in pub_re.captures_iter(content) {
            if let Some(name) = cap.get(2) {
                exports.push(name.as_str().to_string());
            }
        }

        // Match pub use statements
        let pub_use_re = regex::Regex::new(r"pub\s+use\s+[^;]+::\{?([^;}]+)\}?")
            .expect("Failed to compile pub use regex");

        for cap in pub_use_re.captures_iter(content) {
            if let Some(items) = cap.get(1) {
                // Handle multiple items in curly braces
                for item in items.as_str().split(',') {
                    let item = item.trim();
                    if !item.is_empty() {
                        exports.push(item.to_string());
                    }
                }
            }
        }

        exports
    }

    /// Parse JavaScript/TypeScript exports
    fn parse_js_exports(&self, content: &str) -> Vec<String> {
        let mut exports = Vec::new();

        // Named exports
        let named_export_re =
            regex::Regex::new(r"export\s+(?:const|let|var|function|class)\s+(\w+)")
                .expect("Failed to compile JS named export regex");

        for cap in named_export_re.captures_iter(content) {
            if let Some(name) = cap.get(1) {
                exports.push(name.as_str().to_string());
            }
        }

        // Export statements
        let export_re =
            regex::Regex::new(r"export\s+\{([^}]+)\}").expect("Failed to compile JS export regex");

        for cap in export_re.captures_iter(content) {
            if let Some(items) = cap.get(1) {
                for item in items.as_str().split(',') {
                    let item = item.trim().split(" as ").next().unwrap_or("").trim();
                    if !item.is_empty() {
                        exports.push(item.to_string());
                    }
                }
            }
        }

        exports
    }

    /// Parse Python exports (__all__ or public names)
    fn parse_python_exports(&self, content: &str) -> Vec<String> {
        // Try to extract from __all__ first
        if let Some(exports) = Self::extract_python_all_exports(content) {
            return exports;
        }

        // Fall back to extracting public definitions
        Self::extract_python_public_definitions(content)
    }

    /// Extract exports from Python __all__ definition
    fn extract_python_all_exports(content: &str) -> Option<Vec<String>> {
        let all_re = regex::Regex::new(r"__all__\s*=\s*\[([^\]]*)\]")
            .expect("Failed to compile Python __all__ regex");

        all_re.captures(content).map(|cap| {
            cap.get(1)
                .map(|items| Self::parse_python_string_list(items.as_str()))
                .unwrap_or_default()
        })
    }

    /// Parse a comma-separated list of Python strings
    fn parse_python_string_list(items_str: &str) -> Vec<String> {
        items_str
            .split(',')
            .map(|item| item.trim().trim_matches(|c| c == '"' || c == '\''))
            .filter(|item| !item.is_empty())
            .map(String::from)
            .collect()
    }

    /// Extract public function and class definitions from Python code
    fn extract_python_public_definitions(content: &str) -> Vec<String> {
        let def_re = regex::Regex::new(r"^(def|class)\s+([a-zA-Z]\w*)")
            .expect("Failed to compile Python definition regex");

        content
            .lines()
            .filter_map(|line| def_re.captures(line))
            .filter_map(|cap| cap.get(2).map(|m| m.as_str()))
            .filter(|name| !name.starts_with('_'))
            .map(String::from)
            .collect()
    }

    /// Parse dependencies from Cargo.toml
    fn parse_cargo_dependencies(&self, content: &str) -> Vec<String> {
        let mut deps = Vec::new();

        // Simple regex to find dependency names in [dependencies] section
        let mut in_deps_section = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('[') {
                in_deps_section = trimmed == "[dependencies]"
                    || trimmed == "[dev-dependencies]"
                    || trimmed == "[build-dependencies]";
            } else if in_deps_section && trimmed.contains('=') {
                if let Some(dep_name) = trimmed.split('=').next() {
                    let dep_name = dep_name.trim();
                    if !dep_name.is_empty() {
                        deps.push(dep_name.to_string());
                    }
                }
            }
        }

        deps
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
    fn test_parse_js_exports() {
        let analyzer = BasicDependencyAnalyzer::new();

        // Test 1: Named const export
        let content = "export const API_URL = 'https://api.example.com';";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0], "API_URL");

        // Test 2: Named let export
        let content = "export let counter = 0;";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0], "counter");

        // Test 3: Named var export
        let content = "export var isEnabled = true;";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0], "isEnabled");

        // Test 4: Named function export
        let content = "export function processData(data) { return data; }";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0], "processData");

        // Test 5: Named class export
        let content = "export class UserService { constructor() {} }";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0], "UserService");

        // Test 6: Export statement with multiple items
        let content = "const a = 1; const b = 2; export { a, b };";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 2);
        assert!(exports.contains(&"a".to_string()));
        assert!(exports.contains(&"b".to_string()));

        // Test 7: Export statement with 'as' renaming
        let content = "const internal = 'value'; export { internal as external };";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0], "internal");
    }

    #[test]
    fn test_parse_js_exports_complex() {
        let analyzer = BasicDependencyAnalyzer::new();

        // Test mixed export types in single file
        let content = r#"
            export const CONFIG = { debug: true };
            export function calculate(x, y) {
                return x + y;
            }
            export class Calculator {
                add(a, b) { return a + b; }
            }
            const helper1 = () => {};
            const helper2 = () => {};
            export { helper1, helper2 };
        "#;

        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 5);
        assert!(exports.contains(&"CONFIG".to_string()));
        assert!(exports.contains(&"calculate".to_string()));
        assert!(exports.contains(&"Calculator".to_string()));
        assert!(exports.contains(&"helper1".to_string()));
        assert!(exports.contains(&"helper2".to_string()));
    }

    #[test]
    fn test_parse_js_exports_with_spaces() {
        let analyzer = BasicDependencyAnalyzer::new();

        // Test export statement with various spacing
        let content = "export   {   item1  ,   item2   ,  item3   };";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 3);
        assert!(exports.contains(&"item1".to_string()));
        assert!(exports.contains(&"item2".to_string()));
        assert!(exports.contains(&"item3".to_string()));
    }

    #[test]
    fn test_parse_js_exports_multiline() {
        let analyzer = BasicDependencyAnalyzer::new();

        // Test multiline export statement
        let content = r#"export {
            moduleA,
            moduleB,
            moduleC as moduleRenamed
        };"#;

        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 3);
        assert!(exports.contains(&"moduleA".to_string()));
        assert!(exports.contains(&"moduleB".to_string()));
        assert!(exports.contains(&"moduleC".to_string()));
    }

    #[test]
    fn test_parse_js_exports_edge_cases() {
        let analyzer = BasicDependencyAnalyzer::new();

        // Test empty content
        let exports = analyzer.parse_js_exports("");
        assert_eq!(exports.len(), 0);

        // Test content with no exports
        let content = "const internal = 'not exported';";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 0);

        // Test empty export statement
        let content = "export { };";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 0);

        // Test export with only whitespace
        let content = "export {   };";
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 0);
    }

    #[test]
    fn test_parse_js_exports_real_world() {
        let analyzer = BasicDependencyAnalyzer::new();

        // Test realistic JavaScript module exports
        let content = r#"
            // API endpoints
            export const API_BASE = 'https://api.example.com';
            export const API_VERSION = 'v1';
            
            // Utility functions
            export function formatDate(date) {
                return date.toISOString();
            }
            
            export function parseResponse(response) {
                return JSON.parse(response);
            }
            
            // Service class
            export class ApiService {
                constructor(baseUrl) {
                    this.baseUrl = baseUrl;
                }
            }
            
            // Re-export from other modules
            const utils = { format: () => {} };
            const helpers = { parse: () => {} };
            export { utils, helpers as utilHelpers };
            
            // Default export (not captured by this parser)
            export default ApiService;
        "#;

        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 7);
        assert!(exports.contains(&"API_BASE".to_string()));
        assert!(exports.contains(&"API_VERSION".to_string()));
        assert!(exports.contains(&"formatDate".to_string()));
        assert!(exports.contains(&"parseResponse".to_string()));
        assert!(exports.contains(&"ApiService".to_string()));
        assert!(exports.contains(&"utils".to_string()));
        assert!(exports.contains(&"helpers".to_string()));
    }

    #[test]
    fn test_parse_js_exports_typescript() {
        let analyzer = BasicDependencyAnalyzer::new();

        // Test TypeScript-specific exports (should work the same)
        let content = r#"
            export interface User {
                id: number;
                name: string;
            }
            
            export type UserId = number;
            
            export enum Status {
                Active,
                Inactive
            }
            
            export const getUser: (id: UserId) => User;
        "#;

        // Note: The current regex doesn't capture interface, type, or enum
        // but does capture const declarations
        let exports = analyzer.parse_js_exports(content);
        assert_eq!(exports.len(), 1);
        assert!(exports.contains(&"getUser".to_string()));
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

    #[test]
    fn test_parse_python_exports_with_all() {
        let analyzer = BasicDependencyAnalyzer::new();

        let content = r#"
"""Module docstring"""

__all__ = ['public_func', "PublicClass", 'CONSTANT']

def public_func():
    pass

def _private_func():
    pass

class PublicClass:
    pass

class _PrivateClass:
    pass

CONSTANT = 42
"#;

        let exports = analyzer.parse_python_exports(content);
        assert_eq!(exports.len(), 3);
        assert!(exports.contains(&"public_func".to_string()));
        assert!(exports.contains(&"PublicClass".to_string()));
        assert!(exports.contains(&"CONSTANT".to_string()));
    }

    #[test]
    fn test_parse_python_exports_without_all() {
        let analyzer = BasicDependencyAnalyzer::new();

        let content = r#"
def public_function():
    """Public function"""
    pass

def _private_function():
    """Private function"""
    pass

class PublicClass:
    """Public class"""
    pass

class _PrivateClass:
    """Private class"""
    pass

def another_public():
    pass
"#;

        let exports = analyzer.parse_python_exports(content);
        assert_eq!(exports.len(), 3);
        assert!(exports.contains(&"public_function".to_string()));
        assert!(exports.contains(&"PublicClass".to_string()));
        assert!(exports.contains(&"another_public".to_string()));
        assert!(!exports.iter().any(|e| e.starts_with('_')));
    }

    #[test]
    fn test_parse_python_exports_empty_all() {
        let analyzer = BasicDependencyAnalyzer::new();

        let content = r#"
__all__ = []

def some_function():
    pass
"#;

        let exports = analyzer.parse_python_exports(content);
        // Empty __all__ is valid and means explicitly no exports
        assert_eq!(exports.len(), 0);
    }

    #[test]
    fn test_parse_python_exports_multiline_all() {
        let analyzer = BasicDependencyAnalyzer::new();

        // Note: Current implementation only handles single-line __all__
        let content = r#"
__all__ = [
    'func1',
    'func2'
]

def func1():
    pass

def func2():
    pass
"#;

        let exports = analyzer.parse_python_exports(content);
        // Falls back to parsing function definitions since multiline __all__ isn't supported
        assert_eq!(exports.len(), 2);
        assert!(exports.contains(&"func1".to_string()));
        assert!(exports.contains(&"func2".to_string()));
    }

    #[test]
    fn test_parse_python_exports_mixed_quotes() {
        let analyzer = BasicDependencyAnalyzer::new();

        let content = r#"
__all__ = ['single', "double", 'mixed']
"#;

        let exports = analyzer.parse_python_exports(content);
        assert_eq!(exports.len(), 3);
        assert!(exports.contains(&"single".to_string()));
        assert!(exports.contains(&"double".to_string()));
        assert!(exports.contains(&"mixed".to_string()));
    }

    #[test]
    fn test_parse_python_exports_with_spaces() {
        let analyzer = BasicDependencyAnalyzer::new();

        let content = r#"
__all__ = [  'spaced'  ,  "items"  ,  'here'  ]
"#;

        let exports = analyzer.parse_python_exports(content);
        assert_eq!(exports.len(), 3);
        assert!(exports.contains(&"spaced".to_string()));
        assert!(exports.contains(&"items".to_string()));
        assert!(exports.contains(&"here".to_string()));
    }

    #[test]
    fn test_extract_python_all_exports() {
        let content = r#"
__all__ = ['export1', "export2", 'export3']
"#;

        let exports = BasicDependencyAnalyzer::extract_python_all_exports(content);
        assert!(exports.is_some());
        let exports = exports.unwrap();
        assert_eq!(exports.len(), 3);
        assert!(exports.contains(&"export1".to_string()));
        assert!(exports.contains(&"export2".to_string()));
        assert!(exports.contains(&"export3".to_string()));

        // Test empty __all__
        let content_empty = "__all__ = []";
        let exports_empty = BasicDependencyAnalyzer::extract_python_all_exports(content_empty);
        assert!(exports_empty.is_some());
        assert_eq!(exports_empty.unwrap().len(), 0);

        // Test no __all__
        let content_none = "def some_func(): pass";
        assert!(BasicDependencyAnalyzer::extract_python_all_exports(content_none).is_none());
    }

    #[test]
    fn test_parse_python_string_list() {
        let items = "'item1', \"item2\", 'item3'";
        let parsed = BasicDependencyAnalyzer::parse_python_string_list(items);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], "item1");
        assert_eq!(parsed[1], "item2");
        assert_eq!(parsed[2], "item3");
    }

    #[test]
    fn test_extract_python_public_definitions() {
        let content = r#"
def public_func():
    pass

def _private_func():
    pass

class PublicClass:
    pass

class _PrivateClass:
    pass
"#;

        let exports = BasicDependencyAnalyzer::extract_python_public_definitions(content);
        assert_eq!(exports.len(), 2);
        assert!(exports.contains(&"public_func".to_string()));
        assert!(exports.contains(&"PublicClass".to_string()));
    }

    #[test]
    fn test_parse_python_exports_edge_cases() {
        let analyzer = BasicDependencyAnalyzer::new();

        // Empty content
        assert_eq!(analyzer.parse_python_exports("").len(), 0);

        // Only comments
        let content = "# Comment\n# Another comment";
        assert_eq!(analyzer.parse_python_exports(content).len(), 0);

        // Malformed __all__
        let content = "__all__ = 'not_a_list'";
        let exports = analyzer.parse_python_exports(content);
        assert_eq!(exports.len(), 0);
    }
}
