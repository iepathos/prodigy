//! Timeout enforcement for MapReduce agents
//!
//! Provides comprehensive timeout management for agent execution,
//! including configuration, enforcement, monitoring, and recovery.

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, Notify, RwLock};
use tracing::{debug, error, info, warn};

/// Unique identifier for an agent
pub type AgentId = String;

/// Command type enumeration for timeout configuration
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandType {
    Claude,
    Shell,
    GoalSeek,
    Unknown,
}

impl CommandType {
    pub fn as_str(&self) -> &str {
        match self {
            CommandType::Claude => "claude",
            CommandType::Shell => "shell",
            CommandType::GoalSeek => "goal_seek",
            CommandType::Unknown => "unknown",
        }
    }
}

/// Timeout configuration for MapReduce agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Global agent timeout in seconds
    pub agent_timeout_secs: Option<u64>,

    /// Per-command timeout overrides
    #[serde(default)]
    pub command_timeouts: HashMap<String, u64>,

    /// Timeout policy
    #[serde(default)]
    pub timeout_policy: TimeoutPolicy,

    /// Grace period for cleanup after timeout
    #[serde(default = "default_cleanup_grace_period")]
    pub cleanup_grace_period_secs: u64,

    /// Action to take on timeout
    #[serde(default)]
    pub timeout_action: TimeoutAction,

    /// Enable timeout monitoring and metrics
    #[serde(default = "default_true")]
    pub enable_monitoring: bool,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            agent_timeout_secs: Some(600), // 10 minutes default
            command_timeouts: HashMap::new(),
            timeout_policy: TimeoutPolicy::default(),
            cleanup_grace_period_secs: default_cleanup_grace_period(),
            timeout_action: TimeoutAction::default(),
            enable_monitoring: true,
        }
    }
}

/// Timeout enforcement policy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum TimeoutPolicy {
    /// Timeout applies to entire agent execution
    #[default]
    PerAgent,
    /// Timeout applies to each command individually
    PerCommand,
    /// Timeout applies to agent with command-specific overrides
    Hybrid,
}

/// Action to take when timeout occurs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum TimeoutAction {
    /// Terminate agent and send item to DLQ
    #[default]
    Dlq,
    /// Terminate agent and skip item
    Skip,
    /// Terminate agent and fail entire job
    Fail,
    /// Attempt graceful termination then force kill
    GracefulTerminate,
}

fn default_cleanup_grace_period() -> u64 {
    30 // 30 seconds for cleanup
}

fn default_true() -> bool {
    true
}

/// Handle for tracking active timeouts
#[derive(Debug, Clone)]
pub struct TimeoutHandle {
    pub agent_id: AgentId,
    pub work_item_id: String,
    pub started_at: Instant,
    pub timeout_duration: Duration,
    pub cancel_notify: Arc<Notify>,
    pub command_timeouts: Vec<CommandTimeout>,
}

/// Command-specific timeout configuration
#[derive(Debug, Clone)]
pub struct CommandTimeout {
    pub command_index: usize,
    pub command_type: CommandType,
    pub timeout_duration: Duration,
    pub started_at: Option<Instant>,
}

/// Timeout event types for monitoring
#[derive(Debug, Clone)]
pub enum TimeoutEvent {
    AgentStarted {
        agent_id: AgentId,
        work_item_id: String,
        timeout_duration: Duration,
    },
    CommandStarted {
        agent_id: AgentId,
        command_index: usize,
        timeout_duration: Duration,
    },
    CommandCompleted {
        agent_id: AgentId,
        command_index: usize,
        duration: Duration,
    },
    AgentCompleted {
        agent_id: AgentId,
        total_duration: Duration,
    },
    TimeoutOccurred {
        agent_id: AgentId,
        work_item_id: String,
        timeout_type: TimeoutType,
        duration: Duration,
    },
    TimeoutResolved {
        agent_id: AgentId,
        resolution: TimeoutResolution,
    },
}

/// Type of timeout that occurred
#[derive(Debug, Clone)]
pub enum TimeoutType {
    Agent,
    Command {
        index: usize,
        command_type: CommandType,
    },
}

/// How a timeout was resolved
#[derive(Debug, Clone)]
pub enum TimeoutResolution {
    GracefulTermination,
    ForceTermination,
    CleanupCompleted,
    CleanupFailed(String),
}

/// Timeout metrics for monitoring and analysis
#[derive(Debug)]
pub struct TimeoutMetrics {
    pub total_agents_started: AtomicU64,
    pub total_agents_completed: AtomicU64,
    pub total_timeouts_occurred: AtomicU64,
    pub timeout_by_type: Arc<Mutex<HashMap<String, u64>>>,
    pub average_execution_time: Arc<Mutex<RunningAverage>>,
    pub timeout_trends: Arc<Mutex<Vec<TimeoutEvent>>>,
}

impl Default for TimeoutMetrics {
    fn default() -> Self {
        Self {
            total_agents_started: AtomicU64::new(0),
            total_agents_completed: AtomicU64::new(0),
            total_timeouts_occurred: AtomicU64::new(0),
            timeout_by_type: Arc::new(Mutex::new(HashMap::new())),
            average_execution_time: Arc::new(Mutex::new(RunningAverage::new(100))),
            timeout_trends: Arc::new(Mutex::new(Vec::with_capacity(1000))),
        }
    }
}

impl TimeoutMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn record_agent_started(&self) {
        self.total_agents_started.fetch_add(1, Ordering::Relaxed);
    }

    pub async fn record_agent_completed(&self, duration: Duration) {
        self.total_agents_completed.fetch_add(1, Ordering::Relaxed);

        let mut avg = self.average_execution_time.lock().await;
        avg.add_sample(duration.as_secs_f64());
    }

    pub async fn record_timeout(&self, timeout_type: TimeoutType) {
        self.total_timeouts_occurred.fetch_add(1, Ordering::Relaxed);

        let type_str = match timeout_type {
            TimeoutType::Agent => "agent".to_string(),
            TimeoutType::Command { command_type, .. } => {
                format!("command_{}", command_type.as_str())
            }
        };

        let mut by_type = self.timeout_by_type.lock().await;
        *by_type.entry(type_str).or_insert(0) += 1;
    }

    pub async fn record_timeout_event(&self, event: TimeoutEvent) {
        let mut trends = self.timeout_trends.lock().await;
        trends.push(event);

        // Keep only recent 1000 events
        while trends.len() > 1000 {
            trends.remove(0);
        }
    }

    pub async fn get_timeout_summary(&self) -> TimeoutSummary {
        let started = self.total_agents_started.load(Ordering::Relaxed);
        let completed = self.total_agents_completed.load(Ordering::Relaxed);
        let timeouts = self.total_timeouts_occurred.load(Ordering::Relaxed);

        let avg_time = {
            let avg = self.average_execution_time.lock().await;
            avg.get_average()
        };

        let timeout_rate = if started > 0 {
            (timeouts as f64 / started as f64) * 100.0
        } else {
            0.0
        };

        TimeoutSummary {
            agents_started: started,
            agents_completed: completed,
            timeouts_occurred: timeouts,
            timeout_rate_percent: timeout_rate,
            average_execution_time_secs: avg_time,
        }
    }
}

/// Summary of timeout metrics
#[derive(Debug, Serialize)]
pub struct TimeoutSummary {
    pub agents_started: u64,
    pub agents_completed: u64,
    pub timeouts_occurred: u64,
    pub timeout_rate_percent: f64,
    pub average_execution_time_secs: f64,
}

/// Running average calculator for metrics
#[derive(Debug)]
pub struct RunningAverage {
    samples: Vec<f64>,
    capacity: usize,
    sum: f64,
}

impl RunningAverage {
    fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            capacity,
            sum: 0.0,
        }
    }

    fn add_sample(&mut self, value: f64) {
        if self.samples.len() >= self.capacity {
            if let Some(old_value) = self.samples.first() {
                self.sum -= old_value;
                self.samples.remove(0);
            }
        }

        self.samples.push(value);
        self.sum += value;
    }

    fn get_average(&self) -> f64 {
        if self.samples.is_empty() {
            0.0
        } else {
            self.sum / self.samples.len() as f64
        }
    }
}

/// Main timeout enforcement engine
pub struct TimeoutEnforcer {
    config: TimeoutConfig,
    active_timeouts: Arc<RwLock<HashMap<AgentId, TimeoutHandle>>>,
    timeout_metrics: Arc<TimeoutMetrics>,
    event_sender: mpsc::Sender<TimeoutEvent>,
}

impl TimeoutEnforcer {
    /// Create a new timeout enforcer
    pub fn new(config: TimeoutConfig) -> Self {
        let (event_sender, mut event_receiver) = mpsc::channel(1000);
        let timeout_metrics = Arc::new(TimeoutMetrics::new());

        // Start background event processor
        let metrics = timeout_metrics.clone();
        tokio::spawn(async move {
            while let Some(event) = event_receiver.recv().await {
                metrics.record_timeout_event(event).await;
            }
        });

        Self {
            config,
            active_timeouts: Arc::new(RwLock::new(HashMap::new())),
            timeout_metrics,
            event_sender,
        }
    }

    /// Register a timeout for an agent
    pub async fn register_agent_timeout(
        &self,
        agent_id: AgentId,
        work_item_id: String,
        agent_commands: &[crate::cook::workflow::WorkflowStep],
    ) -> MapReduceResult<TimeoutHandle> {
        let timeout_duration = self.calculate_agent_timeout(agent_commands)?;
        let command_timeouts = self.calculate_command_timeouts(agent_commands)?;

        let handle = TimeoutHandle {
            agent_id: agent_id.clone(),
            work_item_id: work_item_id.clone(),
            started_at: Instant::now(),
            timeout_duration,
            cancel_notify: Arc::new(Notify::new()),
            command_timeouts,
        };

        // Register timeout
        {
            let mut active = self.active_timeouts.write().await;
            active.insert(agent_id.clone(), handle.clone());
        }

        // Schedule timeout check
        self.schedule_timeout_check(&handle).await?;

        // Record metrics
        self.timeout_metrics.record_agent_started().await;

        // Send event
        let _ = self
            .event_sender
            .send(TimeoutEvent::AgentStarted {
                agent_id,
                work_item_id,
                timeout_duration,
            })
            .await;

        Ok(handle)
    }

    /// Unregister an agent timeout (called when agent completes)
    pub async fn unregister_agent_timeout(&self, agent_id: &AgentId) -> MapReduceResult<()> {
        if let Some(handle) = {
            let mut active = self.active_timeouts.write().await;
            active.remove(agent_id)
        } {
            // Cancel any pending timeouts
            handle.cancel_notify.notify_one();

            let duration = handle.started_at.elapsed();

            // Record metrics
            self.timeout_metrics.record_agent_completed(duration).await;

            // Send completion event
            let _ = self
                .event_sender
                .send(TimeoutEvent::AgentCompleted {
                    agent_id: agent_id.clone(),
                    total_duration: duration,
                })
                .await;
        }

        Ok(())
    }

    /// Register command execution start
    pub async fn register_command_start(
        &self,
        agent_id: &AgentId,
        command_index: usize,
    ) -> MapReduceResult<()> {
        if matches!(
            self.config.timeout_policy,
            TimeoutPolicy::PerCommand | TimeoutPolicy::Hybrid
        ) {
            let active = self.active_timeouts.read().await;
            if let Some(handle) = active.get(agent_id) {
                if let Some(cmd_timeout) = handle.command_timeouts.get(command_index) {
                    let _ = self
                        .event_sender
                        .send(TimeoutEvent::CommandStarted {
                            agent_id: agent_id.clone(),
                            command_index,
                            timeout_duration: cmd_timeout.timeout_duration,
                        })
                        .await;

                    // Schedule command-specific timeout if needed
                    if matches!(self.config.timeout_policy, TimeoutPolicy::PerCommand) {
                        self.schedule_command_timeout_check(
                            agent_id,
                            command_index,
                            cmd_timeout.timeout_duration,
                        )
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Register command completion
    pub async fn register_command_completion(
        &self,
        agent_id: &AgentId,
        command_index: usize,
        duration: Duration,
    ) -> MapReduceResult<()> {
        let _ = self
            .event_sender
            .send(TimeoutEvent::CommandCompleted {
                agent_id: agent_id.clone(),
                command_index,
                duration,
            })
            .await;

        Ok(())
    }

    /// Schedule a timeout check for an agent
    async fn schedule_timeout_check(&self, handle: &TimeoutHandle) -> MapReduceResult<()> {
        let agent_id = handle.agent_id.clone();
        let work_item_id = handle.work_item_id.clone();
        let timeout_duration = handle.timeout_duration;
        let cancel_notify = handle.cancel_notify.clone();
        let event_sender = self.event_sender.clone();
        let active_timeouts = self.active_timeouts.clone();
        let timeout_action = self.config.timeout_action.clone();
        let timeout_metrics = self.timeout_metrics.clone();

        tokio::spawn(async move {
            // Wait for timeout or cancellation
            tokio::select! {
                _ = tokio::time::sleep(timeout_duration) => {
                    // Timeout occurred
                    warn!("Agent {} timed out after {:?}", agent_id, timeout_duration);

                    // Record metrics
                    timeout_metrics.record_timeout(TimeoutType::Agent).await;

                    // Send timeout event
                    let _ = event_sender.send(TimeoutEvent::TimeoutOccurred {
                        agent_id: agent_id.clone(),
                        work_item_id: work_item_id.clone(),
                        timeout_type: TimeoutType::Agent,
                        duration: timeout_duration,
                    }).await;

                    // Handle timeout
                    Self::handle_agent_timeout(
                        agent_id,
                        active_timeouts,
                        event_sender,
                        timeout_action,
                    ).await;
                }
                _ = cancel_notify.notified() => {
                    // Timeout was cancelled (agent completed normally)
                    debug!("Timeout cancelled for agent {}", agent_id);
                }
            }
        });

        Ok(())
    }

    /// Schedule a command-specific timeout check
    async fn schedule_command_timeout_check(
        &self,
        agent_id: &AgentId,
        command_index: usize,
        timeout_duration: Duration,
    ) -> MapReduceResult<()> {
        let agent_id = agent_id.clone();
        let event_sender = self.event_sender.clone();
        let timeout_metrics = self.timeout_metrics.clone();

        tokio::spawn(async move {
            tokio::time::sleep(timeout_duration).await;

            // Command timeout occurred
            warn!(
                "Command {} for agent {} timed out after {:?}",
                command_index, agent_id, timeout_duration
            );

            // Record metrics
            timeout_metrics
                .record_timeout(TimeoutType::Command {
                    index: command_index,
                    command_type: CommandType::Unknown,
                })
                .await;

            // Send timeout event
            let _ = event_sender
                .send(TimeoutEvent::TimeoutOccurred {
                    agent_id: agent_id.clone(),
                    work_item_id: String::new(),
                    timeout_type: TimeoutType::Command {
                        index: command_index,
                        command_type: CommandType::Unknown,
                    },
                    duration: timeout_duration,
                })
                .await;
        });

        Ok(())
    }

    /// Handle agent timeout
    async fn handle_agent_timeout(
        agent_id: AgentId,
        active_timeouts: Arc<RwLock<HashMap<AgentId, TimeoutHandle>>>,
        event_sender: mpsc::Sender<TimeoutEvent>,
        timeout_action: TimeoutAction,
    ) {
        // Remove from active timeouts
        let handle = {
            let mut active = active_timeouts.write().await;
            active.remove(&agent_id)
        };

        if let Some(_handle) = handle {
            // Determine resolution based on timeout action
            let resolution = match timeout_action {
                TimeoutAction::GracefulTerminate => {
                    // Attempt graceful termination
                    info!("Attempting graceful termination for agent {}", agent_id);
                    TimeoutResolution::GracefulTermination
                }
                TimeoutAction::Dlq => {
                    // Send to DLQ
                    info!("Sending work item to DLQ for agent {}", agent_id);
                    TimeoutResolution::CleanupCompleted
                }
                TimeoutAction::Skip => {
                    // Skip the item
                    info!("Skipping work item for agent {}", agent_id);
                    TimeoutResolution::CleanupCompleted
                }
                TimeoutAction::Fail => {
                    // Fail the job
                    error!("Failing job due to agent {} timeout", agent_id);
                    TimeoutResolution::ForceTermination
                }
            };

            // Send resolution event
            let _ = event_sender
                .send(TimeoutEvent::TimeoutResolved {
                    agent_id,
                    resolution,
                })
                .await;
        }
    }

    /// Calculate agent timeout based on configuration
    fn calculate_agent_timeout(
        &self,
        commands: &[crate::cook::workflow::WorkflowStep],
    ) -> MapReduceResult<Duration> {
        if let Some(global_timeout) = self.config.agent_timeout_secs {
            Ok(Duration::from_secs(global_timeout))
        } else {
            // Calculate based on number of commands with defaults
            let estimated_duration = commands.len() as u64 * 60; // 1 minute per command default
            Ok(Duration::from_secs(estimated_duration.max(60))) // At least 1 minute
        }
    }

    /// Calculate command-specific timeouts
    fn calculate_command_timeouts(
        &self,
        commands: &[crate::cook::workflow::WorkflowStep],
    ) -> MapReduceResult<Vec<CommandTimeout>> {
        let mut timeouts = Vec::new();

        for (index, command) in commands.iter().enumerate() {
            let command_type = if command.claude.is_some() {
                CommandType::Claude
            } else if command.shell.is_some() {
                CommandType::Shell
            } else if command.goal_seek.is_some() {
                CommandType::GoalSeek
            } else {
                CommandType::Unknown
            };

            let timeout_duration = self.get_command_timeout(&command_type, index);

            timeouts.push(CommandTimeout {
                command_index: index,
                command_type,
                timeout_duration,
                started_at: None,
            });
        }

        Ok(timeouts)
    }

    /// Get timeout for a specific command
    fn get_command_timeout(&self, command_type: &CommandType, index: usize) -> Duration {
        // Check for specific command timeout override
        let command_key = format!("{}_{}", command_type.as_str(), index);
        if let Some(timeout) = self.config.command_timeouts.get(&command_key) {
            return Duration::from_secs(*timeout);
        }

        // Check for command type timeout
        if let Some(timeout) = self.config.command_timeouts.get(command_type.as_str()) {
            return Duration::from_secs(*timeout);
        }

        // Use default based on command type
        match command_type {
            CommandType::Claude => Duration::from_secs(300), // 5 minutes for Claude commands
            CommandType::Shell => Duration::from_secs(60),   // 1 minute for shell commands
            CommandType::GoalSeek => Duration::from_secs(600), // 10 minutes for goal seek
            CommandType::Unknown => Duration::from_secs(120), // 2 minutes for unknown
        }
    }

    /// Check if timeout enforcement is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enable_monitoring && self.config.agent_timeout_secs.is_some()
    }

    /// Get current timeout metrics
    pub async fn get_metrics(&self) -> TimeoutSummary {
        self.timeout_metrics.get_timeout_summary().await
    }
}

/// Timeout error types
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError {
    #[error("Agent {agent_id} timed out after {duration:?}")]
    AgentTimeout {
        agent_id: AgentId,
        duration: Duration,
    },

    #[error("Command {command_index} in agent {agent_id} timed out after {duration:?}")]
    CommandTimeout {
        agent_id: AgentId,
        command_index: usize,
        duration: Duration,
    },

    #[error("Invalid timeout configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Timeout enforcer not available")]
    EnforcerUnavailable,

    #[error("Failed to register timeout: {0}")]
    RegistrationFailed(String),
}

impl From<TimeoutError> for MapReduceError {
    fn from(error: TimeoutError) -> Self {
        MapReduceError::ProcessingError(error.to_string())
    }
}
