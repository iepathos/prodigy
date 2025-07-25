use crate::error::{Error, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

use super::{Event, EventCallback, LogLevel};

/// Plugin API trait that provides access to mmm functionality
#[async_trait]
pub trait PluginAPI: Send + Sync {
    // Project management
    async fn get_current_project(&self) -> Result<crate::project::Project>;
    async fn get_project(&self, id: &str) -> Result<Option<crate::project::Project>>;
    async fn get_spec(&self, name: &str) -> Result<Option<String>>;
    async fn update_spec_status(&self, name: &str, status: String) -> Result<()>;
    async fn get_project_config(&self, key: &str) -> Result<Option<Value>>;
    async fn set_project_config(&self, key: &str, value: Value) -> Result<()>;

    // Claude interaction
    async fn claude_request(&self, prompt: &str, options: ClaudeOptions) -> Result<String>;
    async fn get_claude_history(&self, spec: &str) -> Result<Vec<Exchange>>;
    async fn get_claude_usage(&self) -> Result<Usage>;

    // State management
    async fn get_state(&self, key: &str) -> Result<Option<Value>>;
    async fn set_state(&self, key: &str, value: Value) -> Result<()>;
    async fn delete_state(&self, key: &str) -> Result<()>;
    async fn list_state_keys(&self, prefix: &str) -> Result<Vec<String>>;

    // Events
    async fn emit_event(&self, event: Event) -> Result<()>;
    async fn subscribe_to_event(&self, event_type: &str, callback: EventCallback) -> Result<()>;

    // UI/Output
    async fn prompt_user(&self, message: &str, options: PromptOptions) -> Result<String>;
    async fn display_progress(&self, message: &str, progress: f32) -> Result<()>;
    async fn log(&self, level: LogLevel, message: &str) -> Result<()>;
    async fn show_notification(
        &self,
        title: &str,
        message: &str,
        level: NotificationLevel,
    ) -> Result<()>;

    // File system (sandboxed)
    async fn read_file(&self, path: &str) -> Result<String>;
    async fn write_file(&self, path: &str, content: &str) -> Result<()>;
    async fn list_files(&self, path: &str) -> Result<Vec<FileInfo>>;
    async fn file_exists(&self, path: &str) -> Result<bool>;

    // Workflow integration
    async fn trigger_workflow(&self, name: &str, inputs: HashMap<String, Value>) -> Result<String>;
    async fn get_workflow_status(&self, id: &str) -> Result<WorkflowStatus>;

    // Monitoring
    async fn record_metric(
        &self,
        name: &str,
        value: f64,
        tags: HashMap<String, String>,
    ) -> Result<()>;
    async fn start_trace(&self, name: &str) -> Result<TraceId>;
    async fn end_trace(&self, trace_id: TraceId, status: TraceStatus) -> Result<()>;
}

/// Options for Claude API requests
#[derive(Debug, Clone)]
pub struct ClaudeOptions {
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub context: Option<HashMap<String, Value>>,
    pub system_prompt: Option<String>,
}

impl Default for ClaudeOptions {
    fn default() -> Self {
        Self {
            model: None,
            max_tokens: Some(4096),
            temperature: Some(0.7),
            context: None,
            system_prompt: None,
        }
    }
}

/// Claude conversation exchange
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Exchange {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub prompt: String,
    pub response: String,
    pub tokens_used: u32,
    pub model: String,
}

/// Claude API usage statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Usage {
    pub total_tokens: u32,
    pub total_requests: u32,
    pub total_cost: f64,
    pub daily_tokens: u32,
    pub daily_requests: u32,
    pub daily_cost: f64,
}

/// Options for user prompts
#[derive(Debug, Clone)]
pub struct PromptOptions {
    pub input_type: InputType,
    pub default_value: Option<String>,
    pub validation: Option<ValidationRule>,
    pub choices: Option<Vec<String>>,
    pub multiline: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone)]
pub enum InputType {
    Text,
    Number,
    Boolean,
    Choice,
    MultiChoice,
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub enum ValidationRule {
    MinLength(usize),
    MaxLength(usize),
    Regex(String),
    Range(f64, f64),
    Email,
    Url,
    Custom(String), // Custom validation function name
}

impl Default for PromptOptions {
    fn default() -> Self {
        Self {
            input_type: InputType::Text,
            default_value: None,
            validation: None,
            choices: None,
            multiline: false,
            hidden: false,
        }
    }
}

/// Notification levels
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// File information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_directory: bool,
    pub modified: chrono::DateTime<chrono::Utc>,
    pub permissions: FilePermissions,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FilePermissions {
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
}

/// Workflow status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WorkflowStatus {
    Pending,
    Running { current_stage: String },
    Paused { at_stage: String, reason: String },
    Completed { result: Value },
    Failed { error: String, at_stage: String },
    Cancelled,
}

/// Trace ID for monitoring
pub type TraceId = uuid::Uuid;

/// Trace status
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum TraceStatus {
    Success,
    Error,
    Cancelled,
}

/// Default implementation of PluginAPI
#[derive(Clone)]
pub struct DefaultPluginAPI {
    project_manager: std::sync::Arc<crate::project::ProjectManager>,
    state_manager: std::sync::Arc<crate::state::StateManager>,
    claude_manager: std::sync::Arc<crate::claude::ClaudeManager>,
    workflow_engine: std::sync::Arc<crate::workflow::WorkflowEngine>,
    // monitor: std::sync::Arc<crate::monitor::Monitor>, // TODO: Fix Monitor type
    plugin_id: super::PluginId,
    permissions: std::sync::Arc<super::security::PermissionSet>,
}

impl DefaultPluginAPI {
    pub fn new(
        project_manager: std::sync::Arc<crate::project::ProjectManager>,
        state_manager: std::sync::Arc<crate::state::StateManager>,
        claude_manager: std::sync::Arc<crate::claude::ClaudeManager>,
        workflow_engine: std::sync::Arc<crate::workflow::WorkflowEngine>,
        // monitor: std::sync::Arc<crate::monitor::Monitor>, // TODO: Fix Monitor type
        plugin_id: super::PluginId,
        permissions: std::sync::Arc<super::security::PermissionSet>,
    ) -> Self {
        Self {
            project_manager,
            state_manager,
            claude_manager,
            workflow_engine,
            // monitor,
            plugin_id,
            permissions,
        }
    }

    fn check_permission(&self, permission: &super::security::Permission) -> Result<()> {
        if !self.permissions.has_permission(permission) {
            return Err(Error::PermissionDenied(format!(
                "Plugin {} does not have required permission: {:?}",
                self.plugin_id, permission
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl PluginAPI for DefaultPluginAPI {
    async fn get_current_project(&self) -> Result<crate::project::Project> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Project,
        })?;

        self.project_manager
            .current_project()
            .cloned()
            .ok_or_else(|| crate::Error::NotFound("No current project".to_string()))
    }

    async fn get_project(&self, id: &str) -> Result<Option<crate::project::Project>> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Project,
        })?;

        Ok(self.project_manager.get_project(id).ok().cloned())
    }

    async fn get_spec(&self, name: &str) -> Result<Option<String>> {
        self.check_permission(&super::security::Permission::FileSystem {
            path: std::path::PathBuf::from("specs"),
            access: super::security::FileAccess::Read,
        })?;

        // Implementation would load spec from file system
        todo!("Implement spec loading")
    }

    async fn update_spec_status(&self, name: &str, status: String) -> Result<()> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Project,
        })?;

        // Implementation would update spec status in state
        todo!("Implement spec status update")
    }

    async fn get_project_config(&self, key: &str) -> Result<Option<Value>> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Project,
        })?;

        self.state_manager
            .get_value(&format!("project.config.{}", key))
            .await
    }

    async fn set_project_config(&self, key: &str, value: Value) -> Result<()> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Project,
        })?;

        self.state_manager
            .set_value(&format!("project.config.{}", key), value)
            .await
    }

    async fn claude_request(&self, prompt: &str, options: ClaudeOptions) -> Result<String> {
        self.check_permission(&super::security::Permission::Network {
            hosts: vec!["api.anthropic.com".to_string()],
        })?;

        // TODO: Implement Claude request
        Ok(format!("Mock response for prompt: {}", prompt))
    }

    async fn get_claude_history(&self, spec: &str) -> Result<Vec<Exchange>> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Project,
        })?;

        // Implementation would load conversation history
        todo!("Implement history loading")
    }

    async fn get_claude_usage(&self) -> Result<Usage> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Global,
        })?;

        // Implementation would load usage statistics
        todo!("Implement usage statistics")
    }

    async fn get_state(&self, key: &str) -> Result<Option<Value>> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Plugin,
        })?;

        let plugin_key = format!("plugin.{}.{}", self.plugin_id, key);
        self.state_manager.get_value(&plugin_key).await
    }

    async fn set_state(&self, key: &str, value: Value) -> Result<()> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Plugin,
        })?;

        let plugin_key = format!("plugin.{}.{}", self.plugin_id, key);
        self.state_manager.set_value(&plugin_key, value).await
    }

    async fn delete_state(&self, key: &str) -> Result<()> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Plugin,
        })?;

        let plugin_key = format!("plugin.{}.{}", self.plugin_id, key);
        self.state_manager.delete_value(&plugin_key).await
    }

    async fn list_state_keys(&self, prefix: &str) -> Result<Vec<String>> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Plugin,
        })?;

        let plugin_prefix = format!("plugin.{}.{}", self.plugin_id, prefix);
        self.state_manager.list_keys(&plugin_prefix).await
    }

    async fn emit_event(&self, event: Event) -> Result<()> {
        // Events are always allowed
        debug!("Plugin {} emitting event: {:?}", self.plugin_id, event);
        // Implementation would emit event to event bus
        Ok(())
    }

    async fn subscribe_to_event(&self, event_type: &str, callback: EventCallback) -> Result<()> {
        // Event subscription is always allowed
        debug!(
            "Plugin {} subscribing to event: {}",
            self.plugin_id, event_type
        );
        // Implementation would subscribe to event bus
        Ok(())
    }

    async fn prompt_user(&self, message: &str, options: PromptOptions) -> Result<String> {
        // User prompts are always allowed
        info!("Plugin {} prompting user: {}", self.plugin_id, message);

        // Implementation would show prompt to user
        // For now, return a mock response
        Ok("user_response".to_string())
    }

    async fn display_progress(&self, message: &str, progress: f32) -> Result<()> {
        // Progress display is always allowed
        info!(
            "Plugin {} progress: {} ({}%)",
            self.plugin_id,
            message,
            progress * 100.0
        );
        Ok(())
    }

    async fn log(&self, level: LogLevel, message: &str) -> Result<()> {
        // Logging is always allowed
        match level {
            LogLevel::Error => error!("[Plugin {}] {}", self.plugin_id, message),
            LogLevel::Warn => warn!("[Plugin {}] {}", self.plugin_id, message),
            LogLevel::Info => info!("[Plugin {}] {}", self.plugin_id, message),
            LogLevel::Debug => debug!("[Plugin {}] {}", self.plugin_id, message),
            LogLevel::Trace => tracing::trace!("[Plugin {}] {}", self.plugin_id, message),
        }
        Ok(())
    }

    async fn show_notification(
        &self,
        title: &str,
        message: &str,
        level: NotificationLevel,
    ) -> Result<()> {
        // Notifications are always allowed
        info!(
            "Plugin {} notification [{}]: {} - {}",
            self.plugin_id,
            match level {
                NotificationLevel::Info => "INFO",
                NotificationLevel::Success => "SUCCESS",
                NotificationLevel::Warning => "WARNING",
                NotificationLevel::Error => "ERROR",
            },
            title,
            message
        );
        Ok(())
    }

    async fn read_file(&self, path: &str) -> Result<String> {
        let path_buf = std::path::PathBuf::from(path);
        self.check_permission(&super::security::Permission::FileSystem {
            path: path_buf.clone(),
            access: super::security::FileAccess::Read,
        })?;

        tokio::fs::read_to_string(&path_buf)
            .await
            .map_err(|e| Error::IO(e.to_string()))
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let path_buf = std::path::PathBuf::from(path);
        self.check_permission(&super::security::Permission::FileSystem {
            path: path_buf.clone(),
            access: super::security::FileAccess::Write,
        })?;

        tokio::fs::write(&path_buf, content)
            .await
            .map_err(|e| Error::IO(e.to_string()))
    }

    async fn list_files(&self, path: &str) -> Result<Vec<FileInfo>> {
        let path_buf = std::path::PathBuf::from(path);
        self.check_permission(&super::security::Permission::FileSystem {
            path: path_buf.clone(),
            access: super::security::FileAccess::Read,
        })?;

        // Implementation would list directory contents
        todo!("Implement file listing")
    }

    async fn file_exists(&self, path: &str) -> Result<bool> {
        let path_buf = std::path::PathBuf::from(path);
        self.check_permission(&super::security::Permission::FileSystem {
            path: path_buf.clone(),
            access: super::security::FileAccess::Read,
        })?;

        Ok(path_buf.exists())
    }

    async fn trigger_workflow(&self, name: &str, inputs: HashMap<String, Value>) -> Result<String> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Project,
        })?;

        // Implementation would trigger workflow
        todo!("Implement workflow triggering")
    }

    async fn get_workflow_status(&self, id: &str) -> Result<WorkflowStatus> {
        self.check_permission(&super::security::Permission::State {
            scope: super::security::StateScope::Project,
        })?;

        // Implementation would get workflow status
        todo!("Implement workflow status")
    }

    async fn record_metric(
        &self,
        name: &str,
        value: f64,
        tags: HashMap<String, String>,
    ) -> Result<()> {
        // Metrics recording is always allowed
        // TODO: Implement metrics recording
        Ok(())
    }

    async fn start_trace(&self, name: &str) -> Result<TraceId> {
        // Tracing is always allowed
        let trace_id = TraceId::new_v4();
        // TODO: Implement tracing
        Ok(trace_id)
    }

    async fn end_trace(&self, trace_id: TraceId, status: TraceStatus) -> Result<()> {
        // Tracing is always allowed
        // TODO: Implement tracing
        Ok(())
    }
}
