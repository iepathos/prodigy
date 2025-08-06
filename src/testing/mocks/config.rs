//! Mock configuration loader for testing

use crate::config::{Config, ConfigLoader};
use anyhow::Result;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Builder for creating configured mock config loaders
pub struct MockConfigLoaderBuilder {
    config: Config,
    fail_on_load: Option<String>,
}

impl MockConfigLoaderBuilder {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
            fail_on_load: None,
        }
    }

    pub fn with_config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    pub fn fail_on_load(mut self, error: &str) -> Self {
        self.fail_on_load = Some(error.to_string());
        self
    }

    pub fn build(self) -> MockConfigLoader {
        MockConfigLoader {
            config: Arc::new(RwLock::new(self.config)),
            fail_on_load: self.fail_on_load,
        }
    }
}

/// Mock implementation of ConfigLoader for testing
pub struct MockConfigLoader {
    config: Arc<RwLock<Config>>,
    fail_on_load: Option<String>,
}

impl MockConfigLoader {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(Config::default())),
            fail_on_load: None,
        }
    }

    pub fn builder() -> MockConfigLoaderBuilder {
        MockConfigLoaderBuilder::new()
    }

    pub async fn load_with_explicit_path(
        &self,
        _path: &Path,
        _config_path: Option<&Path>,
    ) -> Result<()> {
        if let Some(error) = &self.fail_on_load {
            return Err(anyhow::anyhow!(error.clone()));
        }
        Ok(())
    }

    pub fn get_config(&self) -> Config {
        self.config.read().unwrap().clone()
    }
}
