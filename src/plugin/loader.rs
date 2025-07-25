use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use super::{Plugin, PluginMetadata};

/// Plugin loader handles loading plugins from various sources
pub struct PluginLoader {
    supported_formats: Vec<PluginFormat>,
}

/// Supported plugin formats
#[derive(Debug, Clone)]
pub enum PluginFormat {
    /// Dynamic library (.so, .dylib, .dll)
    DynamicLibrary,
    /// WebAssembly module (.wasm)
    WebAssembly,
    /// Script-based plugin (.js, .py, .sh)
    Script(ScriptType),
}

#[derive(Debug, Clone)]
pub enum ScriptType {
    JavaScript,
    Python,
    Shell,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            supported_formats: vec![
                PluginFormat::DynamicLibrary,
                PluginFormat::WebAssembly,
                PluginFormat::Script(ScriptType::JavaScript),
                PluginFormat::Script(ScriptType::Python),
                PluginFormat::Script(ScriptType::Shell),
            ],
        }
    }

    /// Load a plugin from a directory
    pub async fn load_plugin(&self, plugin_dir: &PathBuf) -> Result<Box<dyn Plugin>> {
        info!("Loading plugin from: {}", plugin_dir.display());

        // Read plugin manifest
        let manifest_path = plugin_dir.join("plugin.toml");
        let manifest_content = tokio::fs::read_to_string(&manifest_path)
            .await
            .map_err(|e| Error::IO(format!("Failed to read plugin manifest: {e}")))?;

        let manifest: super::PluginManifest = toml::from_str(&manifest_content)
            .map_err(|e| Error::InvalidPlugin(format!("Invalid plugin manifest: {e}")))?;

        // Determine plugin format
        let format = self.detect_plugin_format(plugin_dir)?;

        // Load plugin based on format
        match format {
            PluginFormat::DynamicLibrary => self.load_dynamic_library(plugin_dir, &manifest).await,
            PluginFormat::WebAssembly => self.load_webassembly(plugin_dir, &manifest).await,
            PluginFormat::Script(script_type) => {
                self.load_script(plugin_dir, &manifest, script_type).await
            }
        }
    }

    /// Detect plugin format from directory contents
    fn detect_plugin_format(&self, plugin_dir: &Path) -> Result<PluginFormat> {
        // Check for dynamic library
        let lib_extensions = if cfg!(target_os = "windows") {
            vec!["dll"]
        } else if cfg!(target_os = "macos") {
            vec!["dylib", "so"]
        } else {
            vec!["so"]
        };

        for ext in &lib_extensions {
            if plugin_dir.join(format!("libplugin.{ext}")).exists()
                || plugin_dir.join(format!("plugin.{ext}")).exists()
            {
                return Ok(PluginFormat::DynamicLibrary);
            }
        }

        // Check for WebAssembly
        if plugin_dir.join("plugin.wasm").exists() {
            return Ok(PluginFormat::WebAssembly);
        }

        // Check for script files
        if plugin_dir.join("plugin.js").exists() || plugin_dir.join("index.js").exists() {
            return Ok(PluginFormat::Script(ScriptType::JavaScript));
        }

        if plugin_dir.join("plugin.py").exists() || plugin_dir.join("__main__.py").exists() {
            return Ok(PluginFormat::Script(ScriptType::Python));
        }

        if plugin_dir.join("plugin.sh").exists() {
            return Ok(PluginFormat::Script(ScriptType::Shell));
        }

        Err(Error::InvalidPlugin(format!(
            "No supported plugin format found in {}",
            plugin_dir.display()
        )))
    }

    /// Load a dynamic library plugin
    async fn load_dynamic_library(
        &self,
        plugin_dir: &Path,
        manifest: &super::PluginManifest,
    ) -> Result<Box<dyn Plugin>> {
        debug!(
            "Loading dynamic library plugin from: {}",
            plugin_dir.display()
        );

        // Find library file
        let _lib_file = self.find_library_file(plugin_dir)?;

        // Create a stub plugin for now (actual dynamic loading would require unsafe code)
        let plugin = StubPlugin::new(manifest.plugin.clone());

        warn!("Dynamic library loading not fully implemented, using stub plugin");
        Ok(Box::new(plugin))
    }

    /// Load a WebAssembly plugin
    async fn load_webassembly(
        &self,
        plugin_dir: &Path,
        manifest: &super::PluginManifest,
    ) -> Result<Box<dyn Plugin>> {
        debug!("Loading WebAssembly plugin from: {}", plugin_dir.display());

        let wasm_file = plugin_dir.join("plugin.wasm");
        if !wasm_file.exists() {
            return Err(Error::InvalidPlugin(
                "WebAssembly plugin missing plugin.wasm file".to_string(),
            ));
        }

        // Create a stub plugin for now (actual WASM loading would require wasmtime or similar)
        let plugin = StubPlugin::new(manifest.plugin.clone());

        warn!("WebAssembly loading not fully implemented, using stub plugin");
        Ok(Box::new(plugin))
    }

    /// Load a script-based plugin
    async fn load_script(
        &self,
        plugin_dir: &Path,
        manifest: &super::PluginManifest,
        script_type: ScriptType,
    ) -> Result<Box<dyn Plugin>> {
        debug!(
            "Loading script plugin ({:?}) from: {}",
            script_type,
            plugin_dir.display()
        );

        let script_file = match script_type {
            ScriptType::JavaScript => {
                if plugin_dir.join("plugin.js").exists() {
                    plugin_dir.join("plugin.js")
                } else {
                    plugin_dir.join("index.js")
                }
            }
            ScriptType::Python => {
                if plugin_dir.join("plugin.py").exists() {
                    plugin_dir.join("plugin.py")
                } else {
                    plugin_dir.join("__main__.py")
                }
            }
            ScriptType::Shell => plugin_dir.join("plugin.sh"),
        };

        if !script_file.exists() {
            return Err(Error::InvalidPlugin(format!(
                "Script plugin missing expected script file: {}",
                script_file.display()
            )));
        }

        // Create script plugin wrapper
        let plugin = ScriptPlugin::new(manifest.plugin.clone(), script_file, script_type);

        Ok(Box::new(plugin))
    }

    fn find_library_file(&self, plugin_dir: &Path) -> Result<PathBuf> {
        let lib_extensions = if cfg!(target_os = "windows") {
            vec!["dll"]
        } else if cfg!(target_os = "macos") {
            vec!["dylib", "so"]
        } else {
            vec!["so"]
        };

        for ext in &lib_extensions {
            let lib_path = plugin_dir.join(format!("libplugin.{ext}"));
            if lib_path.exists() {
                return Ok(lib_path);
            }

            let lib_path = plugin_dir.join(format!("plugin.{ext}"));
            if lib_path.exists() {
                return Ok(lib_path);
            }
        }

        Err(Error::InvalidPlugin(
            "No dynamic library file found".to_string(),
        ))
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub plugin implementation for testing and unsupported formats
struct StubPlugin {
    metadata: PluginMetadata,
}

impl StubPlugin {
    fn new(info: super::registry::PluginInfo) -> Self {
        let metadata = PluginMetadata {
            id: info.id,
            name: info.name,
            version: info.version,
            author: info.author,
            description: info.description,
            homepage: None,
            license: "Unknown".to_string(),
            capabilities: info.capabilities,
            dependencies: info
                .dependencies
                .into_iter()
                .map(|(name, version)| super::Dependency {
                    name,
                    version_req: semver::VersionReq::parse(&version).unwrap_or_default(),
                    optional: false,
                })
                .collect(),
            requested_permissions: info.requested_permissions,
            min_mmm_version: info.min_mmm_version,
            max_mmm_version: info.max_mmm_version,
        };

        Self { metadata }
    }
}

#[async_trait::async_trait]
impl Plugin for StubPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn init(&mut self, _context: super::PluginContext) -> Result<()> {
        info!("Stub plugin '{}' initialized", self.metadata.name);
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Stub plugin '{}' shutdown", self.metadata.name);
        Ok(())
    }
}

/// Script plugin wrapper that executes external scripts
struct ScriptPlugin {
    metadata: PluginMetadata,
    script_path: PathBuf,
    script_type: ScriptType,
}

impl ScriptPlugin {
    fn new(
        info: super::registry::PluginInfo,
        script_path: PathBuf,
        script_type: ScriptType,
    ) -> Self {
        let metadata = PluginMetadata {
            id: info.id,
            name: info.name,
            version: info.version,
            author: info.author,
            description: info.description,
            homepage: None,
            license: "Unknown".to_string(),
            capabilities: info.capabilities,
            dependencies: info
                .dependencies
                .into_iter()
                .map(|(name, version)| super::Dependency {
                    name,
                    version_req: semver::VersionReq::parse(&version).unwrap_or_default(),
                    optional: false,
                })
                .collect(),
            requested_permissions: info.requested_permissions,
            min_mmm_version: info.min_mmm_version,
            max_mmm_version: info.max_mmm_version,
        };

        Self {
            metadata,
            script_path,
            script_type,
        }
    }

    async fn execute_script(&self, command: &str, args: &[&str]) -> Result<String> {
        let (executable, script_args) = match &self.script_type {
            ScriptType::JavaScript => ("node", vec![self.script_path.to_str().unwrap(), command]),
            ScriptType::Python => ("python3", vec![self.script_path.to_str().unwrap(), command]),
            ScriptType::Shell => ("bash", vec![self.script_path.to_str().unwrap(), command]),
        };

        let mut cmd_args = script_args;
        cmd_args.extend(args);

        let output = tokio::process::Command::new(executable)
            .args(&cmd_args)
            .output()
            .await
            .map_err(|e| Error::PluginExecution(format!("Failed to execute script: {e}")))?;

        if !output.status.success() {
            return Err(Error::PluginExecution(format!(
                "Script execution failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait::async_trait]
impl Plugin for ScriptPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn init(&mut self, _context: super::PluginContext) -> Result<()> {
        info!(
            "Script plugin '{}' initialized from: {}",
            self.metadata.name,
            self.script_path.display()
        );

        // Call init method in script
        self.execute_script("init", &[]).await?;

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Script plugin '{}' shutdown", self.metadata.name);

        // Call shutdown method in script
        self.execute_script("shutdown", &[]).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl super::CommandPlugin for ScriptPlugin {
    async fn execute(&self, args: super::CommandArgs) -> Result<super::CommandResult> {
        let args_json = serde_json::to_string(&args).map_err(Error::Serialization)?;

        let output = self.execute_script("execute", &[&args_json]).await?;

        // Parse result from script output
        let result: super::CommandResult =
            serde_json::from_str(&output).unwrap_or_else(|_| super::CommandResult {
                success: true,
                output,
                exit_code: 0,
                artifacts: Vec::new(),
            });

        Ok(result)
    }

    async fn autocomplete(&self, partial: &str) -> Result<Vec<String>> {
        let output = self.execute_script("autocomplete", &[partial]).await?;

        // Parse completions from script output
        let completions: Vec<String> = serde_json::from_str(&output)
            .unwrap_or_else(|_| output.lines().map(|s| s.to_string()).collect());

        Ok(completions)
    }

    fn help(&self) -> String {
        // For script plugins, help is static from metadata
        self.metadata.description.clone()
    }
}

#[async_trait::async_trait]
impl super::HookPlugin for ScriptPlugin {
    async fn on_event(&mut self, event: super::Event) -> Result<Option<super::Action>> {
        let event_json = serde_json::to_string(&event).map_err(Error::Serialization)?;

        let output = self.execute_script("on_event", &[&event_json]).await?;

        if output.trim().is_empty() {
            return Ok(None);
        }

        // Parse action from script output
        let action: Option<super::Action> = serde_json::from_str(&output).unwrap_or(None);

        Ok(action)
    }

    fn subscribed_events(&self) -> Vec<String> {
        // Extract from capabilities
        self.metadata
            .capabilities
            .iter()
            .filter_map(|cap| match cap {
                super::Capability::Hook { event, .. } => Some(event.clone()),
                _ => None,
            })
            .collect()
    }
}
