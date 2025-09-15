use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Progress visualization configuration for workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressConfig {
    /// Display mode for progress visualization
    #[serde(default = "default_display_mode")]
    pub display_mode: ProgressDisplayMode,

    /// Update interval for progress bars (in milliseconds)
    #[serde(default = "default_update_interval")]
    pub update_interval: u64,

    /// Whether to show resource usage (CPU, memory, disk)
    #[serde(default = "default_show_resource_usage")]
    pub show_resource_usage: bool,

    /// Whether to enable web dashboard
    #[serde(default)]
    pub enable_dashboard: bool,

    /// Port for the web dashboard
    #[serde(default = "default_dashboard_port")]
    pub dashboard_port: u16,

    /// Log level for progress output
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,

    /// Window size for ETA calculation
    #[serde(default = "default_eta_window_size")]
    pub eta_window_size: usize,

    /// Whether to persist progress for resume
    #[serde(default = "default_persist_progress")]
    pub persist_progress: bool,

    /// Whether to show per-agent progress in MapReduce
    #[serde(default = "default_show_agent_progress")]
    pub show_agent_progress: bool,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            display_mode: default_display_mode(),
            update_interval: default_update_interval(),
            show_resource_usage: default_show_resource_usage(),
            enable_dashboard: false,
            dashboard_port: default_dashboard_port(),
            log_level: default_log_level(),
            eta_window_size: default_eta_window_size(),
            persist_progress: default_persist_progress(),
            show_agent_progress: default_show_agent_progress(),
        }
    }
}

/// Display mode for progress visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgressDisplayMode {
    /// Rich terminal UI with colors and animations
    Rich,
    /// Simple progress bars without animations
    Simple,
    /// JSON output for machine parsing
    Json,
    /// No progress display
    None,
}

/// Log level for progress output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

fn default_display_mode() -> ProgressDisplayMode {
    ProgressDisplayMode::Rich
}

fn default_update_interval() -> u64 {
    100 // milliseconds
}

fn default_show_resource_usage() -> bool {
    true
}

fn default_dashboard_port() -> u16 {
    8080
}

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

fn default_eta_window_size() -> usize {
    20
}

fn default_persist_progress() -> bool {
    true
}

fn default_show_agent_progress() -> bool {
    true
}

impl ProgressConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(mode) = std::env::var("PRODIGY_PROGRESS_MODE") {
            config.display_mode = match mode.to_lowercase().as_str() {
                "rich" => ProgressDisplayMode::Rich,
                "simple" => ProgressDisplayMode::Simple,
                "json" => ProgressDisplayMode::Json,
                "none" => ProgressDisplayMode::None,
                _ => ProgressDisplayMode::Rich,
            };
        }

        if let Ok(dashboard) = std::env::var("PRODIGY_ENABLE_DASHBOARD") {
            config.enable_dashboard = dashboard.eq_ignore_ascii_case("true");
        }

        if let Ok(port) = std::env::var("PRODIGY_DASHBOARD_PORT") {
            if let Ok(p) = port.parse() {
                config.dashboard_port = p;
            }
        }

        if let Ok(resources) = std::env::var("PRODIGY_SHOW_RESOURCES") {
            config.show_resource_usage = resources.eq_ignore_ascii_case("true");
        }

        config
    }

    /// Get the update interval as a Duration
    pub fn update_interval_duration(&self) -> Duration {
        Duration::from_millis(self.update_interval)
    }
}
