use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::warn;
use uuid::Uuid;

pub mod api;
pub mod loader;
pub mod manager;
pub mod marketplace;
pub mod registry;
pub mod sandbox;
pub mod security;

pub use api::PluginAPI;
pub use loader::PluginLoader;
pub use manager::PluginManager;
pub use marketplace::PluginMarketplace;
pub use registry::{PluginInfo, PluginRegistry};
pub use sandbox::PluginSandbox;
pub use security::{Permission, PermissionManager};

/// Plugin ID type
pub type PluginId = Uuid;

/// Plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata
    fn metadata(&self) -> &PluginMetadata;

    /// Initialize the plugin with context
    async fn init(&mut self, context: PluginContext) -> Result<()>;

    /// Shutdown the plugin gracefully
    async fn shutdown(&mut self) -> Result<()>;

    /// Get plugin status
    fn status(&self) -> PluginStatus {
        PluginStatus::Ready
    }
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: PluginId,
    pub name: String,
    pub version: semver::Version,
    pub author: String,
    pub description: String,
    pub homepage: Option<String>,
    pub license: String,
    pub capabilities: Vec<Capability>,
    pub dependencies: Vec<Dependency>,
    pub requested_permissions: Vec<Permission>,
    pub min_mmm_version: semver::Version,
    pub max_mmm_version: Option<semver::Version>,
}

/// Plugin capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Capability {
    /// Adds a new command
    Command {
        name: String,
        aliases: Vec<String>,
        description: String,
    },
    /// Hooks into events
    Hook { event: String, priority: i32 },
    /// Provides integration with external services
    Integration { service: String, version: String },
    /// Provides custom report formats
    Reporter { format: String, mime_type: String },
    /// Provides analysis capabilities
    Analyzer {
        name: String,
        file_types: Vec<String>,
    },
}

/// Plugin dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version_req: semver::VersionReq,
    pub optional: bool,
}

/// Plugin context provided during initialization
pub struct PluginContext {
    pub config: crate::config::Config,
    pub api: Box<dyn PluginAPI>,
    pub event_bus: EventBus,
    pub logger: tracing::Span,
    pub data_dir: PathBuf,
    pub temp_dir: PathBuf,
}

/// Plugin status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginStatus {
    Loading,
    Ready,
    Running,
    Error(String),
    Disabled,
}

/// Event system for plugins
pub struct EventBus {
    subscribers: HashMap<String, Vec<EventCallback>>,
}

pub type EventCallback = Box<dyn Fn(Event) -> Result<Option<Action>> + Send + Sync>;

/// Events that plugins can subscribe to
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    /// Before a spec is executed
    BeforeSpecRun {
        spec_id: String,
        context: HashMap<String, serde_json::Value>,
    },
    /// After a spec is executed
    AfterSpecRun {
        spec_id: String,
        result: SpecRunResult,
    },
    /// When a workflow stage completes
    WorkflowStageComplete {
        workflow_id: String,
        stage: String,
        result: StageResult,
    },
    /// When Claude responds
    ClaudeResponse {
        prompt: String,
        response: String,
        tokens_used: u32,
    },
    /// When a project is created
    ProjectCreated { project_id: String, name: String },
    /// When a project is deleted
    ProjectDeleted { project_id: String },
    /// When configuration changes
    ConfigChanged {
        key: String,
        old_value: Option<serde_json::Value>,
        new_value: serde_json::Value,
    },
}

/// Actions that can be returned by event handlers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// Stop processing this event
    Stop,
    /// Modify the event data
    Modify(serde_json::Value),
    /// Trigger another event
    Emit(Event),
    /// Log a message
    Log { level: LogLevel, message: String },
}

/// Log levels for plugin messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Result of spec execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecRunResult {
    pub success: bool,
    pub duration: std::time::Duration,
    pub output: String,
    pub errors: Vec<String>,
}

/// Result of workflow stage execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub success: bool,
    pub duration: std::time::Duration,
    pub output: String,
    pub next_stage: Option<String>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
        }
    }

    pub fn subscribe(&mut self, event_type: &str, callback: EventCallback) {
        self.subscribers
            .entry(event_type.to_string())
            .or_default()
            .push(callback);
    }

    pub async fn emit(&self, event: Event) -> Result<Vec<Action>> {
        let event_type = match &event {
            Event::BeforeSpecRun { .. } => "before_spec_run",
            Event::AfterSpecRun { .. } => "after_spec_run",
            Event::WorkflowStageComplete { .. } => "workflow_stage_complete",
            Event::ClaudeResponse { .. } => "claude_response",
            Event::ProjectCreated { .. } => "project_created",
            Event::ProjectDeleted { .. } => "project_deleted",
            Event::ConfigChanged { .. } => "config_changed",
        };

        let mut actions = Vec::new();

        if let Some(callbacks) = self.subscribers.get(event_type) {
            for callback in callbacks {
                match callback(event.clone()) {
                    Ok(Some(action)) => actions.push(action),
                    Ok(None) => {}
                    Err(e) => {
                        warn!("Plugin event handler failed: {}", e);
                    }
                }
            }
        }

        Ok(actions)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Command plugin trait for plugins that provide commands
#[async_trait]
pub trait CommandPlugin: Plugin {
    /// Execute the command
    async fn execute(&self, args: CommandArgs) -> Result<CommandResult>;

    /// Provide autocomplete suggestions
    async fn autocomplete(&self, partial: &str) -> Result<Vec<String>>;

    /// Get command help text
    fn help(&self) -> String;
}

/// Arguments passed to command plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandArgs {
    pub command: String,
    pub subcommand: Option<String>,
    pub args: Vec<String>,
    pub flags: HashMap<String, String>,
    pub context: HashMap<String, serde_json::Value>,
}

/// Result returned by command plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub output: String,
    pub exit_code: i32,
    pub artifacts: Vec<Artifact>,
}

/// Artifacts produced by commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub path: PathBuf,
    pub mime_type: String,
    pub size: u64,
}

/// Hook plugin trait for plugins that handle events
#[async_trait]
pub trait HookPlugin: Plugin {
    /// Handle an event
    async fn on_event(&mut self, event: Event) -> Result<Option<Action>>;

    /// Get list of events this plugin subscribes to
    fn subscribed_events(&self) -> Vec<String>;
}

/// Integration plugin trait for external service integrations
#[async_trait]
pub trait IntegrationPlugin: Plugin {
    /// Authenticate with the external service
    async fn authenticate(&mut self) -> Result<()>;

    /// Sync data with the external service
    async fn sync(&self) -> Result<SyncResult>;

    /// Check connection status
    async fn health_check(&self) -> Result<HealthStatus>;
}

/// Result of sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub items_synced: u32,
    pub items_created: u32,
    pub items_updated: u32,
    pub items_deleted: u32,
    pub errors: Vec<String>,
    pub duration: std::time::Duration,
}

/// Health status of integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

/// Plugin manifest loaded from plugin.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginInfo,
    pub dependencies: Option<HashMap<String, String>>,
    pub capabilities: Option<CapabilityManifest>,
    pub permissions: Option<PermissionManifest>,
    pub config: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityManifest {
    pub commands: Option<Vec<String>>,
    pub hooks: Option<Vec<String>>,
    pub integrations: Option<Vec<String>>,
    pub reporters: Option<Vec<String>>,
    pub analyzers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionManifest {
    pub filesystem: Option<Vec<String>>,
    pub network: Option<Vec<String>>,
    pub environment: Option<Vec<String>>,
    pub commands: Option<Vec<String>>,
    pub state: Option<Vec<String>>,
}
