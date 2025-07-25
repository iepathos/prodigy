use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tracing::{debug, warn};

use super::PluginId;

/// Permission types that plugins can request
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    /// File system access
    FileSystem { path: PathBuf, access: FileAccess },
    /// Network access to specific hosts
    Network { hosts: Vec<String> },
    /// Environment variable access
    Environment { vars: Vec<String> },
    /// Command execution
    Command { executable: String },
    /// State access with scope
    State { scope: StateScope },
}

/// File access types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FileAccess {
    Read,
    Write,
    Execute,
}

/// State access scopes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StateScope {
    /// Only plugin's own state
    Plugin,
    /// Current project state
    Project,
    /// Global mmm state
    Global,
}

/// Permission set for a plugin
#[derive(Debug, Clone)]
pub struct PermissionSet {
    permissions: HashSet<Permission>,
}

impl PermissionSet {
    pub fn new() -> Self {
        Self {
            permissions: HashSet::new(),
        }
    }

    pub fn from_permissions(permissions: Vec<Permission>) -> Self {
        Self {
            permissions: permissions.into_iter().collect(),
        }
    }

    pub fn grant(&mut self, permission: Permission) {
        self.permissions.insert(permission);
    }

    pub fn revoke(&mut self, permission: &Permission) {
        self.permissions.remove(permission);
    }

    pub fn has_permission(&self, permission: &Permission) -> bool {
        // Check for exact match first
        if self.permissions.contains(permission) {
            return true;
        }

        // Check for broader permissions that would include this one
        match permission {
            Permission::FileSystem { path, access } => {
                // Check if we have broader file system permissions
                for perm in &self.permissions {
                    if let Permission::FileSystem {
                        path: granted_path,
                        access: granted_access,
                    } = perm
                    {
                        if path.starts_with(granted_path) && access <= granted_access {
                            return true;
                        }
                    }
                }
            }
            Permission::Network { hosts } => {
                // Check if we have broader network permissions
                for perm in &self.permissions {
                    if let Permission::Network {
                        hosts: granted_hosts,
                    } = perm
                    {
                        if hosts.iter().all(|h| {
                            granted_hosts.contains(h)
                                || granted_hosts
                                    .iter()
                                    .any(|gh| gh == "*" || h.ends_with(&format!(".{gh}")))
                        }) {
                            return true;
                        }
                    }
                }
            }
            Permission::Environment { vars } => {
                // Check if we have broader environment permissions
                for perm in &self.permissions {
                    if let Permission::Environment { vars: granted_vars } = perm {
                        if vars.iter().all(|v| {
                            granted_vars.contains(v)
                                || granted_vars
                                    .iter()
                                    .any(|gv| gv == "*" || v.starts_with(&gv.replace('*', "")))
                        }) {
                            return true;
                        }
                    }
                }
            }
            Permission::Command { executable } => {
                // Check if we have permission to execute this command
                for perm in &self.permissions {
                    if let Permission::Command {
                        executable: granted_exec,
                    } = perm
                    {
                        if executable == granted_exec || granted_exec == "*" {
                            return true;
                        }
                    }
                }
            }
            Permission::State { scope } => {
                // Check if we have broader state permissions
                for perm in &self.permissions {
                    if let Permission::State {
                        scope: granted_scope,
                    } = perm
                    {
                        match (scope, granted_scope) {
                            (StateScope::Plugin, _) => return true,
                            (StateScope::Project, StateScope::Project | StateScope::Global) => {
                                return true
                            }
                            (StateScope::Global, StateScope::Global) => return true,
                            _ => {}
                        }
                    }
                }
            }
        }

        false
    }

    pub fn list_permissions(&self) -> Vec<Permission> {
        self.permissions.iter().cloned().collect()
    }
}

impl Default for PermissionSet {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialOrd for FileAccess {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        use FileAccess::*;

        match (self, other) {
            (Read, Read) => Some(Ordering::Equal),
            (Read, Write | Execute) => Some(Ordering::Less),
            (Write, Write) => Some(Ordering::Equal),
            (Write, Read) => Some(Ordering::Greater),
            (Write, Execute) => Some(Ordering::Less),
            (Execute, Execute) => Some(Ordering::Equal),
            (Execute, Read | Write) => Some(Ordering::Greater),
        }
    }
}

/// Permission manager handles plugin permissions
pub struct PermissionManager {
    granted_permissions: HashMap<PluginId, PermissionSet>,
    default_permissions: PermissionSet,
}

impl PermissionManager {
    pub fn new() -> Self {
        let mut default_permissions = PermissionSet::new();

        // Grant basic permissions by default
        default_permissions.grant(Permission::State {
            scope: StateScope::Plugin,
        });

        Self {
            granted_permissions: HashMap::new(),
            default_permissions,
        }
    }

    /// Request permissions for a plugin during installation
    pub async fn request_permissions(
        &mut self,
        plugin_id: PluginId,
        requested: Vec<Permission>,
    ) -> Result<bool> {
        if requested.is_empty() {
            // Grant default permissions
            self.granted_permissions
                .insert(plugin_id, self.default_permissions.clone());
            return Ok(true);
        }

        println!("Plugin '{plugin_id}' requests the following permissions:");
        for perm in &requested {
            println!("  - {}", self.format_permission(perm));
        }

        // In a real implementation, this would show a UI prompt
        // For now, we'll auto-approve non-dangerous permissions
        let approved = self.auto_approve_permissions(&requested);

        if approved {
            let mut permission_set = self.default_permissions.clone();
            for perm in requested {
                permission_set.grant(perm);
            }
            self.granted_permissions.insert(plugin_id, permission_set);
        }

        Ok(approved)
    }

    /// Get granted permissions for a plugin
    pub fn get_permissions(&self, plugin_id: &PluginId) -> Option<&PermissionSet> {
        self.granted_permissions.get(plugin_id)
    }

    /// Grant additional permission to a plugin
    pub fn grant_permission(&mut self, plugin_id: PluginId, permission: Permission) -> Result<()> {
        match self.granted_permissions.get_mut(&plugin_id) {
            Some(permissions) => {
                permissions.grant(permission);
                Ok(())
            }
            None => Err(Error::PluginNotFound(plugin_id.to_string())),
        }
    }

    /// Revoke permission from a plugin
    pub fn revoke_permission(
        &mut self,
        plugin_id: PluginId,
        permission: &Permission,
    ) -> Result<()> {
        match self.granted_permissions.get_mut(&plugin_id) {
            Some(permissions) => {
                permissions.revoke(permission);
                Ok(())
            }
            None => Err(Error::PluginNotFound(plugin_id.to_string())),
        }
    }

    /// Remove all permissions for a plugin
    pub fn remove_plugin(&mut self, plugin_id: &PluginId) {
        self.granted_permissions.remove(plugin_id);
    }

    /// Check if a plugin operation is allowed
    pub fn check_permission(&self, plugin_id: &PluginId, permission: &Permission) -> Result<()> {
        match self.granted_permissions.get(plugin_id) {
            Some(permissions) => {
                if permissions.has_permission(permission) {
                    Ok(())
                } else {
                    Err(Error::PermissionDenied(format!(
                        "Plugin {} does not have required permission: {}",
                        plugin_id,
                        self.format_permission(permission)
                    )))
                }
            }
            None => Err(Error::PluginNotFound(plugin_id.to_string())),
        }
    }

    fn format_permission(&self, permission: &Permission) -> String {
        match permission {
            Permission::FileSystem { path, access } => {
                format!("File system {:?} access to {}", access, path.display())
            }
            Permission::Network { hosts } => {
                format!("Network access to {}", hosts.join(", "))
            }
            Permission::Environment { vars } => {
                format!("Environment variable access to {}", vars.join(", "))
            }
            Permission::Command { executable } => {
                format!("Execute command: {executable}")
            }
            Permission::State { scope } => {
                format!("State access with {scope:?} scope")
            }
        }
    }

    fn auto_approve_permissions(&self, permissions: &[Permission]) -> bool {
        // Auto-approve safe permissions
        for perm in permissions {
            match perm {
                Permission::FileSystem { path, access } => {
                    // Only approve read access to safe directories
                    if matches!(access, FileAccess::Write | FileAccess::Execute) {
                        warn!(
                            "Auto-denying file system write/execute permission to {}",
                            path.display()
                        );
                        return false;
                    }

                    // Check if path is in a safe directory
                    let safe_dirs = ["specs/", "docs/", "templates/", ".mmm/", "plugins/"];

                    if !safe_dirs.iter().any(|safe| path.starts_with(safe)) {
                        warn!(
                            "Auto-denying file system access to unsafe path: {}",
                            path.display()
                        );
                        return false;
                    }
                }
                Permission::Network { hosts } => {
                    // Only approve access to known safe hosts
                    let safe_hosts = [
                        "api.anthropic.com",
                        "github.com",
                        "gitlab.com",
                        "bitbucket.org",
                        "registry.npmjs.org",
                        "crates.io",
                    ];

                    for host in hosts {
                        if !safe_hosts.contains(&host.as_str()) && host != "localhost" {
                            warn!("Auto-denying network access to unsafe host: {}", host);
                            return false;
                        }
                    }
                }
                Permission::Command { executable } => {
                    // Only approve safe commands
                    let safe_commands = [
                        "git", "npm", "yarn", "cargo", "rustc", "node", "python", "python3",
                    ];

                    if !safe_commands.contains(&executable.as_str()) {
                        warn!("Auto-denying command execution: {}", executable);
                        return false;
                    }
                }
                Permission::Environment { vars } => {
                    // Check for sensitive environment variables
                    for var in vars {
                        if var.contains("KEY") || var.contains("SECRET") || var.contains("TOKEN") {
                            warn!(
                                "Auto-denying access to sensitive environment variable: {}",
                                var
                            );
                            return false;
                        }
                    }
                }
                Permission::State { scope } => {
                    // Global state access requires approval
                    if matches!(scope, StateScope::Global) {
                        warn!("Auto-denying global state access");
                        return false;
                    }
                }
            }
        }

        debug!("Auto-approving safe permissions");
        true
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Security policy for plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    /// Whether to auto-approve safe permissions
    pub auto_approve_safe: bool,
    /// Maximum file size plugins can read
    pub max_file_size: u64,
    /// Maximum number of network requests per minute
    pub max_network_requests_per_minute: u32,
    /// Maximum execution time for plugin operations
    pub max_execution_time: std::time::Duration,
    /// Whether to sandbox plugin execution
    pub enable_sandboxing: bool,
    /// Allowed file extensions for reading
    pub allowed_file_extensions: Vec<String>,
    /// Blocked file paths
    pub blocked_file_paths: Vec<String>,
    /// Blocked network hosts
    pub blocked_network_hosts: Vec<String>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            auto_approve_safe: true,
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_network_requests_per_minute: 60,
            max_execution_time: std::time::Duration::from_secs(30),
            enable_sandboxing: true,
            allowed_file_extensions: vec![
                ".md".to_string(),
                ".txt".to_string(),
                ".json".to_string(),
                ".yaml".to_string(),
                ".yml".to_string(),
                ".toml".to_string(),
                ".rs".to_string(),
                ".js".to_string(),
                ".ts".to_string(),
                ".py".to_string(),
                ".sh".to_string(),
            ],
            blocked_file_paths: vec![
                "/etc/passwd".to_string(),
                "/etc/shadow".to_string(),
                "~/.ssh/".to_string(),
                "~/.aws/".to_string(),
            ],
            blocked_network_hosts: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "0.0.0.0".to_string(),
            ],
        }
    }
}
