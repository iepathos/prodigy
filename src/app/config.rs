//! Application configuration
//!
//! This module handles application-wide configuration settings.

use anyhow::Result;
use std::path::PathBuf;

/// Application configuration structure
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Verbosity level for logging
    pub verbose: u8,
    /// Working directory
    pub working_dir: PathBuf,
    /// Enable metrics collection
    pub metrics_enabled: bool,
}

impl AppConfig {
    /// Create a new application configuration
    pub fn new(verbose: u8) -> Result<Self> {
        let working_dir = std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?;

        Ok(Self {
            verbose,
            working_dir,
            metrics_enabled: false,
        })
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = dir;
        self
    }

    /// Enable metrics collection
    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.metrics_enabled = enabled;
        self
    }

    /// Get the log level string based on verbosity
    pub fn log_level(&self) -> &'static str {
        match self.verbose {
            0 => "info",
            1 => "debug",
            2 => "trace",
            _ => "trace,hyper=debug,tower=debug",
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            verbose: 0,
            working_dir: PathBuf::from("."),
            metrics_enabled: false,
        }
    }
}
