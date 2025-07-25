use crate::error::{Error, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::{
    api::{DefaultPluginAPI, PluginAPI},
    loader::PluginLoader,
    registry::{PluginInfo, PluginRegistry},
    sandbox::PluginSandbox,
    security::{PermissionManager, SecurityPolicy},
    EventBus, Plugin, PluginContext, PluginId, PluginMetadata, PluginStatus,
};

/// Plugin manager coordinates all plugin operations
pub struct PluginManager {
    registry: Arc<RwLock<PluginRegistry>>,
    loader: Arc<PluginLoader>,
    sandbox: Arc<PluginSandbox>,
    permission_manager: Arc<RwLock<PermissionManager>>,
    security_policy: SecurityPolicy,
    loaded_plugins: Arc<RwLock<HashMap<PluginId, LoadedPlugin>>>,
    event_bus: Arc<RwLock<EventBus>>,

    // Dependencies for plugin API
    project_manager: Arc<crate::project::ProjectManager>,
    state_manager: Arc<crate::state::StateManager>,
    claude_manager: Arc<crate::claude::ClaudeManager>,
    workflow_engine: Arc<crate::workflow::WorkflowEngine>,
    // monitor: Arc<crate::monitor::Monitor>, // TODO: Fix Monitor type
}

/// A loaded plugin with its metadata and instance
struct LoadedPlugin {
    metadata: PluginMetadata,
    plugin: Box<dyn Plugin>,
    status: PluginStatus,
    api: Arc<dyn PluginAPI>,
    load_time: chrono::DateTime<chrono::Utc>,
    last_activity: chrono::DateTime<chrono::Utc>,
}

impl PluginManager {
    pub fn new(
        project_manager: Arc<crate::project::ProjectManager>,
        state_manager: Arc<crate::state::StateManager>,
        claude_manager: Arc<crate::claude::ClaudeManager>,
        workflow_engine: Arc<crate::workflow::WorkflowEngine>,
        // monitor: Arc<crate::monitor::Monitor>, // TODO: Fix Monitor type
    ) -> Self {
        Self {
            registry: Arc::new(RwLock::new(PluginRegistry::new())),
            loader: Arc::new(PluginLoader::new()),
            sandbox: Arc::new(PluginSandbox::new()),
            permission_manager: Arc::new(RwLock::new(PermissionManager::new())),
            security_policy: SecurityPolicy::default(),
            loaded_plugins: Arc::new(RwLock::new(HashMap::new())),
            event_bus: Arc::new(RwLock::new(EventBus::new())),
            project_manager,
            state_manager,
            claude_manager,
            workflow_engine,
            // monitor,
        }
    }

    /// Initialize the plugin manager and discover available plugins
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing plugin manager");

        // Discover plugins in standard locations
        let discovered = self.discover_plugins().await?;
        info!("Discovered {} plugins", discovered.len());

        // Register discovered plugins
        let mut registry = self.registry.write().await;
        for plugin_info in discovered {
            registry.register(plugin_info)?;
        }

        Ok(())
    }

    /// Discover plugins in standard locations
    pub async fn discover_plugins(&self) -> Result<Vec<PluginInfo>> {
        let mut plugins = Vec::new();

        // Search paths for plugins
        let search_paths = self.get_plugin_search_paths();

        for path in search_paths {
            if !path.exists() {
                continue;
            }

            debug!("Scanning plugin directory: {}", path.display());

            let mut entries = tokio::fs::read_dir(&path).await.map_err(|e| {
                Error::IO(format!(
                    "Failed to read plugin directory {}: {}",
                    path.display(),
                    e
                ))
            })?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| Error::IO(e.to_string()))?
            {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    if let Ok(plugin_info) = self.scan_plugin_directory(&entry_path).await {
                        plugins.push(plugin_info);
                    }
                }
            }
        }

        Ok(plugins)
    }

    /// Load a plugin by name
    pub async fn load_plugin(&self, name: &str) -> Result<PluginId> {
        info!("Loading plugin: {}", name);

        // Find plugin in registry
        let registry = self.registry.read().await;
        let plugin_info = registry
            .find_by_name(name)
            .ok_or_else(|| Error::PluginNotFound(name.to_string()))?;

        // Check if already loaded
        let loaded_plugins = self.loaded_plugins.read().await;
        if let Some(loaded) = loaded_plugins.values().find(|p| p.metadata.name == name) {
            return Ok(loaded.metadata.id);
        }
        drop(loaded_plugins);

        // Check version compatibility
        self.check_version_compatibility(&plugin_info)?;

        // Request permissions
        let mut permission_manager = self.permission_manager.write().await;
        let permissions_granted = permission_manager
            .request_permissions(plugin_info.id, plugin_info.requested_permissions.clone())
            .await?;

        if !permissions_granted {
            return Err(Error::PermissionDenied(format!(
                "Permissions not granted for plugin {}",
                name
            )));
        }

        let permission_set = permission_manager
            .get_permissions(&plugin_info.id)
            .ok_or_else(|| Error::Internal("Failed to get permissions after granting".to_string()))?
            .clone();
        drop(permission_manager);

        // Load plugin using loader
        let mut plugin = self.loader.load_plugin(&plugin_info.path).await?;

        // Create plugin API
        let api = Arc::new(DefaultPluginAPI::new(
            self.project_manager.clone(),
            self.state_manager.clone(),
            self.claude_manager.clone(),
            self.workflow_engine.clone(),
            // self.monitor.clone(), // TODO: Fix Monitor type
            plugin_info.id,
            Arc::new(permission_set),
        ));

        // Create plugin context
        let context = PluginContext {
            config: crate::config::Config::default(), // TODO: Load actual config
            api: Box::new((*api).clone()) as Box<dyn PluginAPI>,
            event_bus: crate::plugin::EventBus::new(),
            logger: tracing::info_span!("plugin", name = %name),
            data_dir: self.get_plugin_data_dir(&plugin_info.id),
            temp_dir: self.get_plugin_temp_dir(&plugin_info.id),
        };

        // Initialize plugin in sandbox
        self.sandbox
            .execute_safe(plugin_info.id, || async { plugin.init(context).await })
            .await?;

        let plugin_id = plugin_info.id;
        let loaded_plugin = LoadedPlugin {
            metadata: plugin.metadata().clone(),
            plugin,
            status: PluginStatus::Ready,
            api,
            load_time: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
        };

        // Store loaded plugin
        let mut loaded_plugins = self.loaded_plugins.write().await;
        loaded_plugins.insert(plugin_id, loaded_plugin);

        info!("Successfully loaded plugin: {} ({})", name, plugin_id);
        Ok(plugin_id)
    }

    /// Unload a plugin
    pub async fn unload_plugin(&self, plugin_id: &PluginId) -> Result<()> {
        info!("Unloading plugin: {}", plugin_id);

        let mut loaded_plugins = self.loaded_plugins.write().await;

        if let Some(mut loaded_plugin) = loaded_plugins.remove(plugin_id) {
            // Shutdown plugin in sandbox
            self.sandbox
                .execute_safe(*plugin_id, || async {
                    loaded_plugin.plugin.shutdown().await
                })
                .await?;

            // Remove permissions
            let mut permission_manager = self.permission_manager.write().await;
            permission_manager.remove_plugin(plugin_id);

            info!("Successfully unloaded plugin: {}", plugin_id);
        } else {
            warn!("Attempted to unload non-loaded plugin: {}", plugin_id);
        }

        Ok(())
    }

    /// Get a loaded plugin by ID
    pub async fn get_plugin(&self, plugin_id: &PluginId) -> Result<Arc<dyn PluginAPI>> {
        let loaded_plugins = self.loaded_plugins.read().await;

        match loaded_plugins.get(plugin_id) {
            Some(loaded_plugin) => Ok(loaded_plugin.api.clone()),
            None => Err(Error::PluginNotFound(plugin_id.to_string())),
        }
    }

    /// List all loaded plugins
    pub async fn list_loaded_plugins(&self) -> Result<Vec<PluginMetadata>> {
        let loaded_plugins = self.loaded_plugins.read().await;
        Ok(loaded_plugins
            .values()
            .map(|p| p.metadata.clone())
            .collect())
    }

    /// Get plugin status
    pub async fn get_plugin_status(&self, plugin_id: &PluginId) -> Result<PluginStatus> {
        let loaded_plugins = self.loaded_plugins.read().await;

        match loaded_plugins.get(plugin_id) {
            Some(loaded_plugin) => Ok(loaded_plugin.status.clone()),
            None => Err(Error::PluginNotFound(plugin_id.to_string())),
        }
    }

    /// Enable hot-reload for development
    pub async fn enable_hot_reload(&self, plugin_id: &PluginId) -> Result<()> {
        // TODO: Implement file watching and automatic reloading
        info!("Hot-reload enabled for plugin: {}", plugin_id);
        Ok(())
    }

    /// Reload a plugin (for development)
    pub async fn reload_plugin(&self, plugin_id: &PluginId) -> Result<()> {
        info!("Reloading plugin: {}", plugin_id);

        // Get plugin name before unloading
        let plugin_name = {
            let loaded_plugins = self.loaded_plugins.read().await;
            loaded_plugins
                .get(plugin_id)
                .map(|p| p.metadata.name.clone())
                .ok_or_else(|| Error::PluginNotFound(plugin_id.to_string()))?
        };

        // Unload and reload
        self.unload_plugin(plugin_id).await?;
        self.load_plugin(&plugin_name).await?;

        info!("Successfully reloaded plugin: {}", plugin_name);
        Ok(())
    }

    /// Execute a command from a command plugin
    pub async fn execute_command(
        &self,
        plugin_id: &PluginId,
        args: super::CommandArgs,
    ) -> Result<super::CommandResult> {
        let loaded_plugins = self.loaded_plugins.read().await;

        if let Some(loaded_plugin) = loaded_plugins.get(plugin_id) {
            // Check if plugin supports commands
            if !loaded_plugin
                .metadata
                .capabilities
                .iter()
                .any(|cap| matches!(cap, super::Capability::Command { .. }))
            {
                return Err(Error::InvalidOperation(format!(
                    "Plugin {} does not support commands",
                    plugin_id
                )));
            }

            // Cast to command plugin (unsafe, but we checked capabilities)
            let command_plugin = unsafe {
                std::mem::transmute::<&dyn Plugin, &dyn super::CommandPlugin>(
                    loaded_plugin.plugin.as_ref(),
                )
            };

            // Execute in sandbox
            self.sandbox
                .execute_safe(*plugin_id, || async { command_plugin.execute(args).await })
                .await
        } else {
            Err(Error::PluginNotFound(plugin_id.to_string()))
        }
    }

    /// Get event bus for external event emission
    pub async fn get_event_bus(&self) -> Arc<RwLock<EventBus>> {
        self.event_bus.clone()
    }

    /// Shutdown all plugins
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down plugin manager");

        let loaded_plugins = self.loaded_plugins.read().await;
        let plugin_ids: Vec<PluginId> = loaded_plugins.keys().cloned().collect();
        drop(loaded_plugins);

        for plugin_id in plugin_ids {
            if let Err(e) = self.unload_plugin(&plugin_id).await {
                error!("Failed to unload plugin {}: {}", plugin_id, e);
            }
        }

        info!("Plugin manager shutdown complete");
        Ok(())
    }

    fn get_plugin_search_paths(&self) -> Vec<PathBuf> {
        vec![
            // User plugins
            dirs::home_dir().unwrap_or_default().join(".mmm/plugins"),
            // System plugins
            PathBuf::from("/usr/local/lib/mmm/plugins"),
            // Project plugins
            std::env::current_dir()
                .unwrap_or_default()
                .join(".mmm/plugins"),
            // Development plugins
            std::env::current_dir().unwrap_or_default().join("plugins"),
        ]
    }

    async fn scan_plugin_directory(&self, path: &PathBuf) -> Result<PluginInfo> {
        let manifest_path = path.join("plugin.toml");

        if !manifest_path.exists() {
            return Err(Error::InvalidPlugin(format!(
                "No plugin.toml found in {}",
                path.display()
            )));
        }

        let manifest_content = tokio::fs::read_to_string(&manifest_path)
            .await
            .map_err(|e| Error::IO(e.to_string()))?;

        let manifest: super::PluginManifest = toml::from_str(&manifest_content)
            .map_err(|e| Error::InvalidPlugin(format!("Invalid plugin.toml: {}", e)))?;

        // Convert manifest to PluginInfo
        let plugin_info = PluginInfo {
            id: uuid::Uuid::new_v4(),
            name: manifest.plugin.name,
            version: manifest.plugin.version,
            author: manifest.plugin.author,
            description: manifest.plugin.description,
            path: path.clone(),
            manifest_path,
            requested_permissions: self.parse_permissions(&manifest.permissions),
            capabilities: self.parse_capabilities(&manifest.capabilities),
            dependencies: manifest.dependencies.unwrap_or_default(),
            min_mmm_version: semver::Version::parse("0.1.0").unwrap(),
            max_mmm_version: None,
        };

        Ok(plugin_info)
    }

    fn parse_permissions(
        &self,
        permissions: &Option<super::PermissionManifest>,
    ) -> Vec<super::security::Permission> {
        let mut perms = Vec::new();

        if let Some(perm_manifest) = permissions {
            if let Some(filesystem) = &perm_manifest.filesystem {
                for path_str in filesystem {
                    let access = if path_str.ends_with(":read") {
                        super::security::FileAccess::Read
                    } else if path_str.ends_with(":write") {
                        super::security::FileAccess::Write
                    } else if path_str.ends_with(":execute") {
                        super::security::FileAccess::Execute
                    } else {
                        super::security::FileAccess::Read
                    };

                    let path = PathBuf::from(path_str.split(':').next().unwrap_or(path_str));
                    perms.push(super::security::Permission::FileSystem { path, access });
                }
            }

            if let Some(network) = &perm_manifest.network {
                perms.push(super::security::Permission::Network {
                    hosts: network.clone(),
                });
            }

            if let Some(environment) = &perm_manifest.environment {
                perms.push(super::security::Permission::Environment {
                    vars: environment.clone(),
                });
            }

            if let Some(commands) = &perm_manifest.commands {
                for cmd in commands {
                    perms.push(super::security::Permission::Command {
                        executable: cmd.clone(),
                    });
                }
            }

            if let Some(state) = &perm_manifest.state {
                for scope_str in state {
                    let scope = match scope_str.as_str() {
                        "plugin" => super::security::StateScope::Plugin,
                        "project" => super::security::StateScope::Project,
                        "global" => super::security::StateScope::Global,
                        _ => super::security::StateScope::Plugin,
                    };
                    perms.push(super::security::Permission::State { scope });
                }
            }
        }

        perms
    }

    fn parse_capabilities(
        &self,
        capabilities: &Option<super::CapabilityManifest>,
    ) -> Vec<super::Capability> {
        let mut caps = Vec::new();

        if let Some(cap_manifest) = capabilities {
            if let Some(commands) = &cap_manifest.commands {
                for cmd in commands {
                    caps.push(super::Capability::Command {
                        name: cmd.clone(),
                        aliases: Vec::new(),
                        description: String::new(),
                    });
                }
            }

            if let Some(hooks) = &cap_manifest.hooks {
                for hook in hooks {
                    caps.push(super::Capability::Hook {
                        event: hook.clone(),
                        priority: 0,
                    });
                }
            }

            if let Some(integrations) = &cap_manifest.integrations {
                for integration in integrations {
                    caps.push(super::Capability::Integration {
                        service: integration.clone(),
                        version: "1.0".to_string(),
                    });
                }
            }
        }

        caps
    }

    fn check_version_compatibility(&self, plugin_info: &PluginInfo) -> Result<()> {
        let current_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))
            .map_err(|e| Error::Internal(format!("Invalid current version: {}", e)))?;

        // Check minimum version requirement
        if current_version < plugin_info.min_mmm_version {
            return Err(Error::IncompatibleVersion(format!(
                "Plugin {} requires mmm version {} or higher, but current version is {}",
                plugin_info.name, plugin_info.min_mmm_version, current_version
            )));
        }

        // Check maximum version requirement if specified
        if let Some(max_version) = &plugin_info.max_mmm_version {
            if current_version > *max_version {
                return Err(Error::IncompatibleVersion(format!(
                    "Plugin {} is not compatible with mmm version {}, maximum supported version is {}",
                    plugin_info.name, current_version, max_version
                )));
            }
        }

        Ok(())
    }

    fn get_plugin_data_dir(&self, plugin_id: &PluginId) -> PathBuf {
        dirs::data_dir()
            .unwrap_or_default()
            .join("mmm")
            .join("plugins")
            .join(plugin_id.to_string())
    }

    fn get_plugin_temp_dir(&self, plugin_id: &PluginId) -> PathBuf {
        std::env::temp_dir()
            .join("mmm")
            .join("plugins")
            .join(plugin_id.to_string())
    }
}
