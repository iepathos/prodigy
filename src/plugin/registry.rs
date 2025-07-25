use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::{security::Permission, Capability, PluginId};

/// Plugin registry manages available plugins
#[derive(Debug, Clone)]
pub struct PluginRegistry {
    plugins: HashMap<PluginId, PluginInfo>,
    name_index: HashMap<String, PluginId>,
}

/// Information about a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: PluginId,
    pub name: String,
    pub version: semver::Version,
    pub author: String,
    pub description: String,
    pub path: PathBuf,
    pub manifest_path: PathBuf,
    pub requested_permissions: Vec<Permission>,
    pub capabilities: Vec<Capability>,
    pub dependencies: HashMap<String, String>,
    pub min_mmm_version: semver::Version,
    pub max_mmm_version: Option<semver::Version>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            name_index: HashMap::new(),
        }
    }

    /// Register a plugin in the registry
    pub fn register(&mut self, plugin_info: PluginInfo) -> Result<PluginId> {
        // Check for name conflicts
        if self.name_index.contains_key(&plugin_info.name) {
            return Err(Error::PluginAlreadyExists(plugin_info.name));
        }

        let plugin_id = plugin_info.id;
        self.name_index.insert(plugin_info.name.clone(), plugin_id);
        self.plugins.insert(plugin_id, plugin_info);

        Ok(plugin_id)
    }

    /// Unregister a plugin from the registry
    pub fn unregister(&mut self, plugin_id: &PluginId) -> Result<()> {
        if let Some(plugin_info) = self.plugins.remove(plugin_id) {
            self.name_index.remove(&plugin_info.name);
            Ok(())
        } else {
            Err(Error::PluginNotFound(plugin_id.to_string()))
        }
    }

    /// Get plugin information by ID
    pub fn get(&self, plugin_id: &PluginId) -> Option<&PluginInfo> {
        self.plugins.get(plugin_id)
    }

    /// Find plugin by name
    pub fn find_by_name(&self, name: &str) -> Option<&PluginInfo> {
        self.name_index
            .get(name)
            .and_then(|id| self.plugins.get(id))
    }

    /// List all registered plugins
    pub fn list_all(&self) -> Vec<&PluginInfo> {
        self.plugins.values().collect()
    }

    /// Find plugins by capability
    pub fn find_by_capability(&self, capability: &str) -> Vec<&PluginInfo> {
        self.plugins
            .values()
            .filter(|plugin| {
                plugin.capabilities.iter().any(|cap| match cap {
                    Capability::Command { name, .. } => name == capability,
                    Capability::Hook { event, .. } => event == capability,
                    Capability::Integration { service, .. } => service == capability,
                    Capability::Reporter { format, .. } => format == capability,
                    Capability::Analyzer { name, .. } => name == capability,
                })
            })
            .collect()
    }

    /// Search plugins by keyword
    pub fn search(&self, query: &str) -> Vec<&PluginInfo> {
        let query_lower = query.to_lowercase();

        self.plugins
            .values()
            .filter(|plugin| {
                plugin.name.to_lowercase().contains(&query_lower)
                    || plugin.description.to_lowercase().contains(&query_lower)
                    || plugin.author.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Get plugins that depend on a specific plugin
    pub fn get_dependents(&self, plugin_name: &str) -> Vec<&PluginInfo> {
        self.plugins
            .values()
            .filter(|plugin| plugin.dependencies.contains_key(plugin_name))
            .collect()
    }

    /// Check if a plugin satisfies version requirements
    pub fn check_dependency(&self, name: &str, version_req: &str) -> Result<bool> {
        let requirement = semver::VersionReq::parse(version_req).map_err(|e| {
            Error::InvalidVersion(format!("Invalid version requirement '{version_req}': {e}"))
        })?;

        if let Some(plugin) = self.find_by_name(name) {
            Ok(requirement.matches(&plugin.version))
        } else {
            Ok(false)
        }
    }

    /// Get plugin dependency graph
    pub fn get_dependency_graph(&self) -> Result<DependencyGraph> {
        let mut graph = DependencyGraph::new();

        // Add all plugins as nodes
        for plugin in self.plugins.values() {
            graph.add_node(plugin.name.clone(), plugin.version.clone());
        }

        // Add dependency edges
        for plugin in self.plugins.values() {
            for (dep_name, dep_version) in &plugin.dependencies {
                if let Some(dep_plugin) = self.find_by_name(dep_name) {
                    let version_req = semver::VersionReq::parse(dep_version).map_err(|e| {
                        Error::InvalidVersion(format!(
                            "Invalid version requirement '{dep_version}': {e}"
                        ))
                    })?;

                    if !version_req.matches(&dep_plugin.version) {
                        return Err(Error::DependencyConflict(format!(
                            "Plugin {} requires {} {} but found {}",
                            plugin.name, dep_name, dep_version, dep_plugin.version
                        )));
                    }

                    graph.add_edge(plugin.name.clone(), dep_name.clone());
                } else {
                    return Err(Error::MissingDependency(format!(
                        "Plugin {} depends on {} which is not available",
                        plugin.name, dep_name
                    )));
                }
            }
        }

        Ok(graph)
    }

    /// Get load order based on dependencies
    pub fn get_load_order(&self) -> Result<Vec<String>> {
        let graph = self.get_dependency_graph()?;
        graph.topological_sort()
    }

    /// Export registry to file
    pub async fn export_to_file(&self, path: &PathBuf) -> Result<()> {
        let registry_data = RegistryData {
            plugins: self.plugins.values().cloned().collect(),
        };

        let json = serde_json::to_string_pretty(&registry_data).map_err(Error::Serialization)?;

        tokio::fs::write(path, json)
            .await
            .map_err(|e| Error::IO(e.to_string()))?;

        Ok(())
    }

    /// Import registry from file
    pub async fn import_from_file(&mut self, path: &PathBuf) -> Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| Error::IO(e.to_string()))?;

        let registry_data: RegistryData =
            serde_json::from_str(&content).map_err(|e| Error::Deserialization(e.to_string()))?;

        // Clear existing registry
        self.plugins.clear();
        self.name_index.clear();

        // Register imported plugins
        for plugin_info in registry_data.plugins {
            self.register(plugin_info)?;
        }

        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable registry data for export/import
#[derive(Debug, Serialize, Deserialize)]
struct RegistryData {
    plugins: Vec<PluginInfo>,
}

/// Dependency graph for plugin loading order
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    nodes: HashMap<String, semver::Version>,
    edges: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, name: String, version: semver::Version) {
        self.nodes.insert(name.clone(), version);
        self.edges.entry(name).or_default();
    }

    pub fn add_edge(&mut self, from: String, to: String) {
        self.edges.entry(from).or_default().push(to);
    }

    /// Perform topological sort to get load order
    pub fn topological_sort(&self) -> Result<Vec<String>> {
        let mut visited = std::collections::HashSet::new();
        let mut temp_visited = std::collections::HashSet::new();
        let mut result = Vec::new();

        for node in self.nodes.keys() {
            if !visited.contains(node) {
                self.dfs_visit(node, &mut visited, &mut temp_visited, &mut result)?;
            }
        }

        result.reverse();
        Ok(result)
    }

    fn dfs_visit(
        &self,
        node: &str,
        visited: &mut std::collections::HashSet<String>,
        temp_visited: &mut std::collections::HashSet<String>,
        result: &mut Vec<String>,
    ) -> Result<()> {
        if temp_visited.contains(node) {
            return Err(Error::CircularDependency(format!(
                "Circular dependency detected involving plugin: {node}"
            )));
        }

        if visited.contains(node) {
            return Ok(());
        }

        temp_visited.insert(node.to_string());

        if let Some(dependencies) = self.edges.get(node) {
            for dep in dependencies {
                self.dfs_visit(dep, visited, temp_visited, result)?;
            }
        }

        temp_visited.remove(node);
        visited.insert(node.to_string());
        result.push(node.to_string());

        Ok(())
    }

    /// Check if the graph has cycles
    pub fn has_cycles(&self) -> bool {
        self.topological_sort().is_err()
    }

    /// Get all paths from one node to another
    pub fn find_paths(&self, from: &str, to: &str) -> Vec<Vec<String>> {
        let mut paths = Vec::new();
        let mut current_path = Vec::new();
        let mut visited = std::collections::HashSet::new();

        self.find_paths_recursive(from, to, &mut current_path, &mut visited, &mut paths);
        paths
    }

    fn find_paths_recursive(
        &self,
        current: &str,
        target: &str,
        current_path: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
        paths: &mut Vec<Vec<String>>,
    ) {
        if visited.contains(current) {
            return;
        }

        current_path.push(current.to_string());
        visited.insert(current.to_string());

        if current == target {
            paths.push(current_path.clone());
        } else if let Some(dependencies) = self.edges.get(current) {
            for dep in dependencies {
                self.find_paths_recursive(dep, target, current_path, visited, paths);
            }
        }

        current_path.pop();
        visited.remove(current);
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}
