use crate::error::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

/// Plugin marketplace for discovering and installing plugins
pub struct PluginMarketplace {
    client: Client,
    registry_url: String,
    cache_dir: PathBuf,
    auth_token: Option<String>,
}

impl PluginMarketplace {
    pub fn new(registry_url: String, cache_dir: PathBuf) -> Self {
        Self {
            client: Client::new(),
            registry_url,
            cache_dir,
            auth_token: None,
        }
    }

    /// Set authentication token for private registries
    pub fn set_auth_token(&mut self, token: String) {
        self.auth_token = Some(token);
    }

    /// Search for plugins in the marketplace
    pub async fn search(&self, query: &str) -> Result<Vec<PluginListing>> {
        info!("Searching marketplace for: {}", query);

        let url = format!("{}/search", self.registry_url);
        let mut request = self.client.get(&url).query(&[("q", query)]);

        if let Some(token) = &self.auth_token {
            request = request.bearer_auth(token);
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to search marketplace: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::Network(format!(
                "Marketplace search failed with status: {}",
                response.status()
            )));
        }

        let search_response: SearchResponse = response
            .json()
            .await
            .map_err(|e| Error::Deserialization(format!("Failed to parse search response: {e}")))?;

        Ok(search_response.plugins)
    }

    /// Get plugin details from marketplace
    pub async fn get_plugin_info(&self, name: &str) -> Result<PluginListing> {
        debug!("Getting plugin info for: {}", name);

        let url = format!("{}/plugins/{}", self.registry_url, name);
        let mut request = self.client.get(&url);

        if let Some(token) = &self.auth_token {
            request = request.bearer_auth(token);
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to get plugin info: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::Network(format!(
                "Failed to get plugin info, status: {}",
                response.status()
            )));
        }

        let plugin: PluginListing = response
            .json()
            .await
            .map_err(|e| Error::Deserialization(format!("Failed to parse plugin info: {e}")))?;

        Ok(plugin)
    }

    /// Install a plugin from the marketplace
    pub async fn install(&self, plugin_name: &str, version: Option<&str>) -> Result<PathBuf> {
        info!(
            "Installing plugin: {} (version: {:?})",
            plugin_name, version
        );

        // Get plugin information
        let plugin_info = self.get_plugin_info(plugin_name).await?;

        // Determine version to install
        let install_version = match version {
            Some(v) => {
                if !plugin_info.versions.contains_key(v) {
                    return Err(Error::InvalidVersion(format!(
                        "Version {v} not found for plugin {plugin_name}"
                    )));
                }
                v.to_string()
            }
            None => plugin_info.latest_version.clone(),
        };

        // Check if already installed
        let install_dir = self.get_plugin_install_dir(plugin_name, &install_version);
        if install_dir.exists() {
            warn!(
                "Plugin {} version {} already installed",
                plugin_name, install_version
            );
            return Ok(install_dir);
        }

        // Download plugin
        let download_info = plugin_info.versions.get(&install_version).ok_or_else(|| {
            Error::InvalidVersion(format!("Version {install_version} info not found"))
        })?;

        let plugin_data = self.download_plugin(download_info).await?;

        // Verify signature if available
        if let Some(signature) = &download_info.signature {
            self.verify_signature(&plugin_data, signature)?;
        }

        // Extract plugin
        let install_path = self
            .extract_plugin(&plugin_data, plugin_name, &install_version)
            .await?;

        // Run post-install hooks
        self.run_post_install(&install_path).await?;

        info!(
            "Successfully installed plugin: {} version {}",
            plugin_name, install_version
        );
        Ok(install_path)
    }

    /// Uninstall a plugin
    pub async fn uninstall(&self, plugin_name: &str, version: Option<&str>) -> Result<()> {
        info!(
            "Uninstalling plugin: {} (version: {:?})",
            plugin_name, version
        );

        let install_dir = if let Some(v) = version {
            self.get_plugin_install_dir(plugin_name, v)
        } else {
            // Find any installed version
            let base_dir = self.cache_dir.join("installed").join(plugin_name);
            if !base_dir.exists() {
                return Err(Error::PluginNotFound(plugin_name.to_string()));
            }

            // Get the first (and hopefully only) version
            let mut entries = fs::read_dir(&base_dir)
                .await
                .map_err(|e| Error::IO(e.to_string()))?;

            if let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| Error::IO(e.to_string()))?
            {
                entry.path()
            } else {
                return Err(Error::PluginNotFound(plugin_name.to_string()));
            }
        };

        if !install_dir.exists() {
            return Err(Error::PluginNotFound(format!(
                "Plugin {} not found at {}",
                plugin_name,
                install_dir.display()
            )));
        }

        // Run pre-uninstall hooks
        self.run_pre_uninstall(&install_dir).await?;

        // Remove plugin directory
        fs::remove_dir_all(&install_dir)
            .await
            .map_err(|e| Error::IO(format!("Failed to remove plugin directory: {e}")))?;

        info!("Successfully uninstalled plugin: {}", plugin_name);
        Ok(())
    }

    /// List installed plugins
    pub async fn list_installed(&self) -> Result<Vec<InstalledPlugin>> {
        let installed_dir = self.cache_dir.join("installed");
        if !installed_dir.exists() {
            return Ok(Vec::new());
        }

        let mut installed = Vec::new();
        let mut entries = fs::read_dir(&installed_dir)
            .await
            .map_err(|e| Error::IO(e.to_string()))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::IO(e.to_string()))?
        {
            let plugin_name = entry.file_name().to_string_lossy().to_string();
            let plugin_dir = entry.path();

            // Check for version directories
            let mut version_entries = fs::read_dir(&plugin_dir)
                .await
                .map_err(|e| Error::IO(e.to_string()))?;

            while let Some(version_entry) = version_entries
                .next_entry()
                .await
                .map_err(|e| Error::IO(e.to_string()))?
            {
                let version = version_entry.file_name().to_string_lossy().to_string();
                let install_path = version_entry.path();

                // Read plugin manifest
                let manifest_path = install_path.join("plugin.toml");
                if manifest_path.exists() {
                    if let Ok(manifest_content) = fs::read_to_string(&manifest_path).await {
                        if let Ok(manifest) =
                            toml::from_str::<super::PluginManifest>(&manifest_content)
                        {
                            installed.push(InstalledPlugin {
                                name: plugin_name.clone(),
                                version: version.clone(),
                                author: manifest.plugin.author,
                                description: manifest.plugin.description,
                                install_path: install_path.clone(),
                                install_date: self.get_install_date(&install_path).await?,
                            });
                        }
                    }
                }
            }
        }

        Ok(installed)
    }

    /// Update a plugin to the latest version
    pub async fn update(&self, plugin_name: &str) -> Result<()> {
        info!("Updating plugin: {}", plugin_name);

        // Get current installed version
        let installed = self.list_installed().await?;
        let current = installed
            .iter()
            .find(|p| p.name == plugin_name)
            .ok_or_else(|| Error::PluginNotFound(plugin_name.to_string()))?;

        // Get latest version from marketplace
        let plugin_info = self.get_plugin_info(plugin_name).await?;

        // Check if update is needed
        let current_version = semver::Version::parse(&current.version)
            .map_err(|e| Error::InvalidVersion(e.to_string()))?;
        let latest_version = semver::Version::parse(&plugin_info.latest_version)
            .map_err(|e| Error::InvalidVersion(e.to_string()))?;

        if current_version >= latest_version {
            info!(
                "Plugin {} is already up to date ({})",
                plugin_name, current.version
            );
            return Ok(());
        }

        // Uninstall current version
        self.uninstall(plugin_name, Some(&current.version)).await?;

        // Install latest version
        self.install(plugin_name, Some(&plugin_info.latest_version))
            .await?;

        info!(
            "Successfully updated plugin {} from {} to {}",
            plugin_name, current.version, plugin_info.latest_version
        );
        Ok(())
    }

    /// Publish a plugin to the marketplace
    pub async fn publish(&self, plugin_path: &Path, api_key: &str) -> Result<()> {
        info!("Publishing plugin from: {}", plugin_path.display());

        // Read plugin manifest
        let manifest_path = plugin_path.join("plugin.toml");
        let manifest_content = fs::read_to_string(&manifest_path)
            .await
            .map_err(|e| Error::IO(format!("Failed to read manifest: {e}")))?;

        let manifest: super::PluginManifest = toml::from_str(&manifest_content)
            .map_err(|e| Error::InvalidPlugin(format!("Invalid manifest: {e}")))?;

        // Create plugin package
        let package_path = self.create_package(plugin_path, &manifest).await?;

        // Upload to marketplace
        let upload_url = format!("{}/publish", self.registry_url);

        let package_data = fs::read(&package_path)
            .await
            .map_err(|e| Error::IO(format!("Failed to read package: {e}")))?;

        let form = reqwest::multipart::Form::new()
            .part(
                "package",
                reqwest::multipart::Part::bytes(package_data)
                    .file_name("plugin.tar.gz")
                    .mime_str("application/gzip")?,
            )
            .text("manifest", manifest_content);

        let response = self
            .client
            .post(&upload_url)
            .bearer_auth(api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to publish plugin: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::Network(format!(
                "Plugin publish failed with status: {}",
                response.status()
            )));
        }

        // Clean up package file
        let _ = fs::remove_file(&package_path).await;

        info!("Successfully published plugin: {}", manifest.plugin.name);
        Ok(())
    }

    async fn download_plugin(&self, download_info: &DownloadInfo) -> Result<Vec<u8>> {
        debug!("Downloading plugin from: {}", download_info.url);

        let mut request = self.client.get(&download_info.url);
        if let Some(token) = &self.auth_token {
            request = request.bearer_auth(token);
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to download plugin: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::Network(format!(
                "Plugin download failed with status: {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::Network(format!("Failed to read plugin data: {e}")))?;

        Ok(bytes.to_vec())
    }

    fn verify_signature(&self, _data: &[u8], signature: &str) -> Result<()> {
        // In a real implementation, this would verify cryptographic signature
        debug!("Verifying plugin signature: {}", signature);

        // For now, just check that signature is not empty
        if signature.is_empty() {
            return Err(Error::InvalidSignature("Empty signature".to_string()));
        }

        // TODO: Implement actual signature verification
        Ok(())
    }

    async fn extract_plugin(&self, data: &[u8], name: &str, version: &str) -> Result<PathBuf> {
        let install_dir = self.get_plugin_install_dir(name, version);

        // Create install directory
        fs::create_dir_all(&install_dir)
            .await
            .map_err(|e| Error::IO(format!("Failed to create install directory: {e}")))?;

        // Write archive to temporary file
        let temp_path = install_dir.join("plugin.tar.gz");
        fs::write(&temp_path, data)
            .await
            .map_err(|e| Error::IO(format!("Failed to write plugin archive: {e}")))?;

        // Extract archive (using tar command for simplicity)
        let output = tokio::process::Command::new("tar")
            .args(["-xzf", "plugin.tar.gz"])
            .current_dir(&install_dir)
            .output()
            .await
            .map_err(|e| Error::Command(format!("Failed to extract plugin: {e}")))?;

        if !output.status.success() {
            return Err(Error::Command(format!(
                "Plugin extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Remove temporary archive
        let _ = fs::remove_file(&temp_path).await;

        Ok(install_dir)
    }

    async fn run_post_install(&self, plugin_path: &PathBuf) -> Result<()> {
        let post_install_script = plugin_path.join("scripts").join("post-install.sh");

        if !post_install_script.exists() {
            return Ok(());
        }

        debug!(
            "Running post-install script: {}",
            post_install_script.display()
        );

        let output = tokio::process::Command::new("bash")
            .arg(&post_install_script)
            .current_dir(plugin_path)
            .output()
            .await
            .map_err(|e| Error::Command(format!("Failed to run post-install script: {e}")))?;

        if !output.status.success() {
            warn!(
                "Post-install script failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    async fn run_pre_uninstall(&self, plugin_path: &PathBuf) -> Result<()> {
        let pre_uninstall_script = plugin_path.join("scripts").join("pre-uninstall.sh");

        if !pre_uninstall_script.exists() {
            return Ok(());
        }

        debug!(
            "Running pre-uninstall script: {}",
            pre_uninstall_script.display()
        );

        let output = tokio::process::Command::new("bash")
            .arg(&pre_uninstall_script)
            .current_dir(plugin_path)
            .output()
            .await
            .map_err(|e| Error::Command(format!("Failed to run pre-uninstall script: {e}")))?;

        if !output.status.success() {
            warn!(
                "Pre-uninstall script failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    async fn create_package(
        &self,
        plugin_path: &Path,
        manifest: &super::PluginManifest,
    ) -> Result<PathBuf> {
        let package_name = format!(
            "{}-{}.tar.gz",
            manifest.plugin.name, manifest.plugin.version
        );
        let package_path = self.cache_dir.join("packages").join(&package_name);

        // Create packages directory
        if let Some(parent) = package_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::IO(format!("Failed to create packages directory: {e}")))?;
        }

        // Create tar archive
        let output = tokio::process::Command::new("tar")
            .args([
                "-czf",
                package_path.to_str().unwrap(),
                "-C",
                plugin_path.to_str().unwrap(),
                ".",
            ])
            .output()
            .await
            .map_err(|e| Error::Command(format!("Failed to create package: {e}")))?;

        if !output.status.success() {
            return Err(Error::Command(format!(
                "Package creation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(package_path)
    }

    fn get_plugin_install_dir(&self, name: &str, version: &str) -> PathBuf {
        self.cache_dir.join("installed").join(name).join(version)
    }

    async fn get_install_date(&self, path: &PathBuf) -> Result<chrono::DateTime<chrono::Utc>> {
        let metadata = fs::metadata(path)
            .await
            .map_err(|e| Error::IO(e.to_string()))?;

        let created = metadata.created().map_err(|e| Error::IO(e.to_string()))?;

        Ok(chrono::DateTime::from(created))
    }
}

/// Marketplace search response
#[derive(Debug, Deserialize)]
struct SearchResponse {
    plugins: Vec<PluginListing>,
    #[allow(dead_code)]
    total: usize,
    #[allow(dead_code)]
    page: usize,
    #[allow(dead_code)]
    per_page: usize,
}

/// Plugin listing in marketplace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginListing {
    pub name: String,
    pub author: String,
    pub description: String,
    pub latest_version: String,
    pub versions: HashMap<String, DownloadInfo>,
    pub tags: Vec<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: String,
    pub downloads: u64,
    pub rating: f32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Download information for a plugin version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub url: String,
    pub size: u64,
    pub checksum: String,
    pub signature: Option<String>,
}

/// Information about an installed plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub install_path: PathBuf,
    pub install_date: chrono::DateTime<chrono::Utc>,
}
