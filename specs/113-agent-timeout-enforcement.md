---
number: 113
title: MapReduce Agent Timeout Enforcement
category: reliability
priority: important
status: draft
dependencies: [109]
created: 2025-09-27
---

# Specification 113: MapReduce Agent Timeout Enforcement

## Context

The current MapReduce implementation includes an `agent_timeout_secs` configuration field but timeout enforcement is not implemented. This allows individual agents to run indefinitely, potentially blocking entire MapReduce jobs and consuming resources without bounds.

Current gaps:
- `agent_timeout_secs` field exists in MapReduceConfig but is not enforced
- No timeout mechanism for individual agent execution
- Missing timeout handling for different command types (Claude, shell, etc.)
- No configurable timeout policies (per-command vs. per-agent)
- Stuck agents can prevent job completion indefinitely
- No timeout monitoring or alerting

Without proper timeout enforcement, MapReduce jobs can become stuck on problematic work items, wasting computational resources and preventing timely completion of processing pipelines.

## Objective

Implement comprehensive agent timeout enforcement for MapReduce workflows that ensures jobs complete within reasonable time bounds, handles stuck agents gracefully, and provides configurable timeout policies for different execution scenarios.

## Requirements

### Functional Requirements

#### Timeout Configuration
- Support global agent timeout configuration via `agent_timeout_secs`
- Allow per-command timeout overrides in agent templates
- Support different timeout policies (per-agent vs. per-command)
- Enable timeout inheritance and escalation strategies
- Provide timeout configuration validation

#### Timeout Enforcement
- Enforce timeouts on individual agent execution
- Support granular timeouts for different command types
- Handle timeout events with configurable actions
- Integrate with existing error handling and DLQ systems
- Preserve partial results when timeouts occur

#### Timeout Monitoring
- Track agent execution time and timeout events
- Provide timeout metrics and alerting
- Log timeout incidents with diagnostic information
- Support timeout pattern analysis and reporting
- Monitor timeout trends across job executions

#### Recovery and Cleanup
- Clean up resources for timed-out agents
- Support timeout recovery strategies (retry, skip, fail)
- Integrate timeout handling with worktree cleanup
- Preserve timeout information in checkpoints
- Enable timeout-aware resume functionality

### Non-Functional Requirements
- Timeout enforcement should add minimal overhead (< 1% execution time)
- Timeout precision should be within 1-2 seconds of configured values
- Support for concurrent timeout monitoring across multiple agents
- Graceful degradation when timeout system experiences issues
- Clear and actionable timeout error messages

## Acceptance Criteria

- [ ] `agent_timeout_secs` configuration is enforced for agent execution
- [ ] Per-command timeout overrides work in agent templates
- [ ] Timed-out agents are terminated and cleaned up properly
- [ ] Timeout events are logged with appropriate detail
- [ ] Timeout metrics are collected and available for monitoring
- [ ] DLQ integration preserves timeout information for retry analysis
- [ ] Checkpoint/resume functionality handles timeout state correctly
- [ ] Performance impact of timeout monitoring is measurable and minimal

## Technical Details

### Implementation Approach

#### 1. Timeout Configuration Structure

Extend timeout configuration with comprehensive options:

```rust
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeoutPolicy {
    /// Timeout applies to entire agent execution
    PerAgent,
    /// Timeout applies to each command individually
    PerCommand,
    /// Timeout applies to agent with command-specific overrides
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeoutAction {
    /// Terminate agent and send item to DLQ
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

impl Default for TimeoutPolicy {
    fn default() -> Self {
        TimeoutPolicy::PerAgent
    }
}

impl Default for TimeoutAction {
    fn default() -> Self {
        TimeoutAction::Dlq
    }
}
```

#### 2. Timeout Enforcement Engine

Create a dedicated timeout monitoring and enforcement system:

```rust
pub struct TimeoutEnforcer {
    config: TimeoutConfig,
    active_timeouts: Arc<Mutex<HashMap<AgentId, TimeoutHandle>>>,
    timeout_metrics: Arc<TimeoutMetrics>,
    event_sender: mpsc::Sender<TimeoutEvent>,
}

#[derive(Debug, Clone)]
pub struct TimeoutHandle {
    pub agent_id: AgentId,
    pub work_item_id: String,
    pub started_at: Instant,
    pub timeout_duration: Duration,
    pub cancel_token: CancellationToken,
    pub command_timeouts: Vec<CommandTimeout>,
}

#[derive(Debug, Clone)]
pub struct CommandTimeout {
    pub command_index: usize,
    pub command_type: CommandType,
    pub timeout_duration: Duration,
    pub started_at: Option<Instant>,
}

#[derive(Debug)]
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

#[derive(Debug, Clone)]
pub enum TimeoutType {
    Agent,
    Command { index: usize, command_type: CommandType },
}

#[derive(Debug, Clone)]
pub enum TimeoutResolution {
    GracefulTermination,
    ForceTermination,
    CleanupCompleted,
    CleanupFailed(String),
}

impl TimeoutEnforcer {
    pub fn new(config: TimeoutConfig) -> Self {
        let (event_sender, event_receiver) = mpsc::channel(1000);
        let timeout_metrics = Arc::new(TimeoutMetrics::new());

        // Start timeout monitoring task
        let enforcer = Self {
            config,
            active_timeouts: Arc::new(Mutex::new(HashMap::new())),
            timeout_metrics: timeout_metrics.clone(),
            event_sender,
        };

        // Spawn background timeout monitoring
        tokio::spawn(Self::timeout_monitor_task(
            enforcer.active_timeouts.clone(),
            timeout_metrics,
            event_receiver,
        ));

        enforcer
    }

    pub async fn register_agent_timeout(
        &self,
        agent_id: AgentId,
        work_item_id: String,
        agent_commands: &[WorkflowStep],
    ) -> Result<TimeoutHandle, TimeoutError> {
        let timeout_duration = self.calculate_agent_timeout(agent_commands)?;
        let command_timeouts = self.calculate_command_timeouts(agent_commands)?;

        let handle = TimeoutHandle {
            agent_id: agent_id.clone(),
            work_item_id: work_item_id.clone(),
            started_at: Instant::now(),
            timeout_duration,
            cancel_token: CancellationToken::new(),
            command_timeouts,
        };

        // Register timeout
        {
            let mut active = self.active_timeouts.lock().await;
            active.insert(agent_id.clone(), handle.clone());
        }

        // Schedule timeout check
        self.schedule_timeout_check(&handle).await?;

        // Send event
        self.event_sender.send(TimeoutEvent::AgentStarted {
            agent_id,
            work_item_id,
            timeout_duration,
        }).await?;

        Ok(handle)
    }

    pub async fn unregister_agent_timeout(&self, agent_id: &AgentId) -> Result<(), TimeoutError> {
        if let Some(handle) = {
            let mut active = self.active_timeouts.lock().await;
            active.remove(agent_id)
        } {
            // Cancel any pending timeouts
            handle.cancel_token.cancel();

            // Send completion event
            self.event_sender.send(TimeoutEvent::AgentCompleted {
                agent_id: agent_id.clone(),
                total_duration: handle.started_at.elapsed(),
            }).await?;
        }

        Ok(())
    }

    pub async fn register_command_timeout(
        &self,
        agent_id: &AgentId,
        command_index: usize,
    ) -> Result<(), TimeoutError> {
        if matches!(self.config.timeout_policy, TimeoutPolicy::PerCommand | TimeoutPolicy::Hybrid) {
            let mut active = self.active_timeouts.lock().await;
            if let Some(handle) = active.get_mut(agent_id) {
                if let Some(cmd_timeout) = handle.command_timeouts.get_mut(command_index) {
                    cmd_timeout.started_at = Some(Instant::now());

                    // Schedule command-specific timeout
                    self.schedule_command_timeout_check(agent_id, command_index, cmd_timeout.timeout_duration).await?;

                    // Send event
                    self.event_sender.send(TimeoutEvent::CommandStarted {
                        agent_id: agent_id.clone(),
                        command_index,
                        timeout_duration: cmd_timeout.timeout_duration,
                    }).await?;
                }
            }
        }

        Ok(())
    }

    async fn schedule_timeout_check(&self, handle: &TimeoutHandle) -> Result<(), TimeoutError> {
        let agent_id = handle.agent_id.clone();
        let work_item_id = handle.work_item_id.clone();
        let timeout_duration = handle.timeout_duration;
        let cancel_token = handle.cancel_token.clone();
        let event_sender = self.event_sender.clone();
        let active_timeouts = self.active_timeouts.clone();

        tokio::spawn(async move {
            // Wait for timeout or cancellation
            tokio::select! {
                _ = tokio::time::sleep(timeout_duration) => {
                    // Timeout occurred
                    let _ = event_sender.send(TimeoutEvent::TimeoutOccurred {
                        agent_id: agent_id.clone(),
                        work_item_id: work_item_id.clone(),
                        timeout_type: TimeoutType::Agent,
                        duration: timeout_duration,
                    }).await;

                    // Trigger timeout handling
                    Self::handle_agent_timeout(agent_id, active_timeouts, event_sender).await;
                }
                _ = cancel_token.cancelled() => {
                    // Timeout was cancelled (agent completed normally)
                }
            }
        });

        Ok(())
    }

    async fn handle_agent_timeout(
        agent_id: AgentId,
        active_timeouts: Arc<Mutex<HashMap<AgentId, TimeoutHandle>>>,
        event_sender: mpsc::Sender<TimeoutEvent>,
    ) {
        // Remove from active timeouts
        let handle = {
            let mut active = active_timeouts.lock().await;
            active.remove(&agent_id)
        };

        if let Some(handle) = handle {
            // Attempt graceful termination
            let resolution = Self::terminate_agent(&agent_id, &handle).await;

            // Send resolution event
            let _ = event_sender.send(TimeoutEvent::TimeoutResolved {
                agent_id,
                resolution,
            }).await;
        }
    }

    async fn terminate_agent(agent_id: &AgentId, handle: &TimeoutHandle) -> TimeoutResolution {
        // Signal the agent to terminate gracefully
        handle.cancel_token.cancel();

        // Wait for grace period
        tokio::time::sleep(Duration::from_secs(handle.timeout_duration.as_secs().min(30))).await;

        // Force termination if needed (implementation depends on agent architecture)
        // This might involve killing processes, cleaning up worktrees, etc.

        TimeoutResolution::GracefulTermination
    }

    fn calculate_agent_timeout(&self, commands: &[WorkflowStep]) -> Result<Duration, TimeoutError> {
        if let Some(global_timeout) = self.config.agent_timeout_secs {
            Ok(Duration::from_secs(global_timeout))
        } else {
            // Calculate timeout based on commands
            let estimated_duration = commands.len() as u64 * 60; // 1 minute per command default
            Ok(Duration::from_secs(estimated_duration))
        }
    }

    fn calculate_command_timeouts(&self, commands: &[WorkflowStep]) -> Result<Vec<CommandTimeout>, TimeoutError> {
        let mut timeouts = Vec::new();

        for (index, command) in commands.iter().enumerate() {
            let command_type = if command.claude.is_some() {
                CommandType::Claude
            } else if command.shell.is_some() {
                CommandType::Shell
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
            CommandType::Unknown => Duration::from_secs(120), // 2 minutes for unknown
        }
    }
}
```

#### 3. Timeout Metrics and Monitoring

Implement comprehensive timeout monitoring:

```rust
#[derive(Debug)]
pub struct TimeoutMetrics {
    pub total_agents_started: AtomicU64,
    pub total_agents_completed: AtomicU64,
    pub total_timeouts_occurred: AtomicU64,
    pub timeout_by_type: Arc<Mutex<HashMap<TimeoutType, u64>>>,
    pub average_execution_time: Arc<Mutex<RunningAverage>>,
    pub timeout_trends: Arc<Mutex<VecDeque<TimeoutEvent>>>,
}

impl TimeoutMetrics {
    pub fn new() -> Self {
        Self {
            total_agents_started: AtomicU64::new(0),
            total_agents_completed: AtomicU64::new(0),
            total_timeouts_occurred: AtomicU64::new(0),
            timeout_by_type: Arc::new(Mutex::new(HashMap::new())),
            average_execution_time: Arc::new(Mutex::new(RunningAverage::new(100))),
            timeout_trends: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
        }
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

        let mut by_type = self.timeout_by_type.lock().await;
        *by_type.entry(timeout_type).or_insert(0) += 1;
    }

    pub async fn record_timeout_event(&self, event: TimeoutEvent) {
        let mut trends = self.timeout_trends.lock().await;
        trends.push_back(event);

        // Keep only recent events
        while trends.len() > 1000 {
            trends.pop_front();
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

#[derive(Debug, Serialize)]
pub struct TimeoutSummary {
    pub agents_started: u64,
    pub agents_completed: u64,
    pub timeouts_occurred: u64,
    pub timeout_rate_percent: f64,
    pub average_execution_time_secs: f64,
}

#[derive(Debug)]
struct RunningAverage {
    samples: VecDeque<f64>,
    capacity: usize,
    sum: f64,
}

impl RunningAverage {
    fn new(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
            sum: 0.0,
        }
    }

    fn add_sample(&mut self, value: f64) {
        if self.samples.len() >= self.capacity {
            if let Some(old_value) = self.samples.pop_front() {
                self.sum -= old_value;
            }
        }

        self.samples.push_back(value);
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
```

#### 4. Agent Integration

Integrate timeout enforcement with MapReduce agent execution:

```rust
impl MapReduceAgent {
    pub async fn execute_with_timeout(
        &mut self,
        work_item: WorkItem,
        timeout_enforcer: &TimeoutEnforcer,
    ) -> Result<AgentResult, AgentError> {
        let agent_id = self.id.clone();
        let work_item_id = work_item.id.clone();

        // Register timeout for this agent
        let timeout_handle = timeout_enforcer
            .register_agent_timeout(agent_id.clone(), work_item_id, &self.agent_template)
            .await?;

        // Execute with timeout monitoring
        let result = tokio::select! {
            result = self.execute_commands(&work_item, timeout_enforcer) => {
                result
            }
            _ = timeout_handle.cancel_token.cancelled() => {
                // Agent was cancelled due to timeout
                Err(AgentError::Timeout {
                    agent_id: agent_id.clone(),
                    duration: timeout_handle.started_at.elapsed(),
                })
            }
        };

        // Unregister timeout
        timeout_enforcer.unregister_agent_timeout(&agent_id).await?;

        result
    }

    async fn execute_commands(
        &mut self,
        work_item: &WorkItem,
        timeout_enforcer: &TimeoutEnforcer,
    ) -> Result<AgentResult, AgentError> {
        let mut command_results = Vec::new();

        for (index, command) in self.agent_template.iter().enumerate() {
            // Register command timeout if using per-command or hybrid policy
            timeout_enforcer.register_command_timeout(&self.id, index).await?;

            // Execute command
            let result = self.execute_single_command(command, work_item).await?;
            command_results.push(result);

            // Record command completion
            timeout_enforcer.record_command_completion(&self.id, index).await?;
        }

        Ok(AgentResult {
            agent_id: self.id.clone(),
            work_item_id: work_item.id.clone(),
            success: true,
            command_results,
            execution_time: self.start_time.elapsed(),
            metadata: AgentMetadata::default(),
        })
    }
}
```

### YAML Configuration Integration

```yaml
name: example-with-timeout-config
mode: mapreduce

map:
  input: "large-dataset.json"
  json_path: "$.items[*]"

  # Global agent timeout
  agent_timeout_secs: 600  # 10 minutes per agent

  # Timeout configuration
  timeout_config:
    timeout_policy: hybrid
    cleanup_grace_period_secs: 30
    timeout_action: dlq
    enable_monitoring: true

    # Per-command type timeouts
    command_timeouts:
      claude: 300     # 5 minutes for Claude commands
      shell: 60       # 1 minute for shell commands
      claude_0: 600   # 10 minutes for first Claude command specifically

  agent_template:
    - claude: "/analyze-complex-item '${item}'"  # Uses claude_0 timeout (600s)
    - shell: "validate-results ${item.id}"       # Uses shell timeout (60s)
    - claude: "/generate-report '${item}'"       # Uses claude timeout (300s)

reduce:
  - claude: "/summarize-results ${map.results}"
```

### Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError {
    #[error("Agent {agent_id} timed out after {duration:?}")]
    AgentTimeout { agent_id: AgentId, duration: Duration },

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

    #[error("Timeout monitoring error: {0}")]
    MonitoringError(#[from] tokio::sync::mpsc::error::SendError<TimeoutEvent>),
}

impl From<TimeoutError> for AgentError {
    fn from(timeout_error: TimeoutError) -> Self {
        match timeout_error {
            TimeoutError::AgentTimeout { agent_id, duration } => {
                AgentError::Timeout { agent_id, duration }
            }
            TimeoutError::CommandTimeout { agent_id, command_index, duration } => {
                AgentError::CommandTimeout { agent_id, command_index, duration }
            }
            _ => AgentError::TimeoutSystem(timeout_error.to_string()),
        }
    }
}
```

## Testing Strategy

### Unit Tests
- Test timeout configuration parsing and validation
- Test timeout calculation for different command types
- Test timeout enforcement timing accuracy
- Test timeout event generation and handling
- Test metrics collection and calculation

### Integration Tests
- Test end-to-end agent timeout enforcement
- Test per-command timeout functionality
- Test timeout integration with DLQ system
- Test timeout handling during checkpoint/resume
- Test timeout cleanup integration with worktree management

### Performance Tests
- Benchmark timeout monitoring overhead
- Test timeout precision under high load
- Test concurrent timeout monitoring
- Test memory usage of timeout tracking

### Reliability Tests
- Test timeout enforcement with stuck processes
- Test timeout system recovery from failures
- Test timeout accuracy under system load
- Test graceful degradation when timeout system fails

## Migration Strategy

### Phase 1: Core Timeout Infrastructure
1. Implement `TimeoutEnforcer` and basic timeout monitoring
2. Add timeout configuration parsing and validation
3. Implement timeout metrics collection

### Phase 2: Agent Integration
1. Integrate timeout enforcement with agent execution
2. Add per-command timeout support
3. Implement timeout error handling and recovery

### Phase 3: Monitoring and Management
1. Add timeout monitoring dashboard
2. Implement timeout alerting and notifications
3. Add CLI commands for timeout management

### Phase 4: Advanced Features
1. Add adaptive timeout adjustment based on patterns
2. Implement timeout prediction and recommendations
3. Add timeout optimization tools

## Documentation Requirements

- Update MapReduce configuration documentation with timeout options
- Document timeout policies and their trade-offs
- Create troubleshooting guide for timeout-related issues
- Document timeout monitoring and metrics
- Add best practices for timeout configuration

## Risk Assessment

### High Risk
- **False Timeouts**: Overly aggressive timeouts might terminate legitimate long-running operations
- **Resource Leaks**: Failed timeout cleanup might leave resources hanging
- **System Impact**: Timeout monitoring overhead might impact overall performance

### Medium Risk
- **Configuration Complexity**: Complex timeout configurations might be error-prone
- **Monitoring Overhead**: Extensive timeout monitoring might consume significant resources
- **Recovery Failures**: Failed timeout recovery might leave agents in inconsistent state

### Mitigation Strategies
- Implement conservative default timeouts with clear configuration guidance
- Provide timeout monitoring tools to help users optimize configurations
- Add comprehensive logging and diagnostics for timeout events
- Implement robust cleanup procedures with fallback mechanisms
- Include timeout configuration validation with helpful error messages