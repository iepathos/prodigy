---
number: 185
title: Claude Command Retry with Stillwater Effects
category: parallel
priority: high
status: draft
dependencies: [183, 121]
created: 2025-11-26
---

# Specification 185: Claude Command Retry with Stillwater Effects

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: Spec 183 (Effect-Based Workflow Execution), Spec 121 (Claude Observability)

## Context

Claude Code commands can fail due to transient errors:
- **HTTP 500 errors**: Internal server errors
- **Overload errors**: API rate limiting or capacity issues
- **Network timeouts**: Temporary connectivity problems

Currently, when these transient errors occur:
1. The workflow fails immediately
2. No checkpoint is saved (see Spec 184)
3. User must manually restart the entire workflow
4. All progress is lost

Stillwater provides `Effect::retry_if` with:
- Configurable retry policies (exponential backoff, jitter)
- Selective retry based on error type (transient vs permanent)
- Retry hooks for observability (logging, metrics)
- Timeout support

This specification defines how to wrap Claude command execution with Stillwater's retry functionality.

## Objective

Implement automatic retry for Claude commands using Stillwater's Effect pattern to:
1. Automatically retry transient errors (500, overload, timeout)
2. Fail fast for permanent errors (auth, invalid command)
3. Provide visibility into retry attempts
4. Support configurable retry policies
5. Integrate with checkpoint system for state preservation

## Requirements

### Functional Requirements

#### FR1: Transient Error Detection
- **MUST** classify 500 errors as transient (retryable)
- **MUST** classify overload/rate limit errors as transient
- **MUST** classify network timeouts as transient
- **MUST** classify authentication errors as permanent (non-retryable)
- **MUST** classify invalid command errors as permanent

#### FR2: Retry Policy Configuration
- **MUST** support exponential backoff by default
- **MUST** support configurable initial delay (default: 5 seconds)
- **MUST** support configurable max retries (default: 5)
- **MUST** support jitter to prevent thundering herd
- **MUST** support per-workflow retry policy override

#### FR3: Retry Observability
- **MUST** log each retry attempt with attempt number and delay
- **MUST** log error type and message for each failure
- **MUST** expose retry metrics (attempts, total duration)
- **MUST** preserve Claude JSON log location for debugging (Spec 121)

#### FR4: Timeout Support
- **MUST** support per-command timeout
- **MUST** support default timeout (10 minutes)
- **MUST** treat timeout as transient error (retryable)
- **MUST** log timeout with command that timed out

### Non-Functional Requirements

#### NFR1: Performance
- Retry overhead MUST be minimal (jitter calculation)
- Backoff delays MUST be accurate (±100ms)

#### NFR2: Reliability
- Retry state MUST survive checkpoint/resume
- Interrupted retries MUST be resumable

#### NFR3: User Experience
- Clear indication when retry is in progress
- Total wait time should be reasonable (< 5 minutes typical)
- User can interrupt retry with Ctrl+C

## Acceptance Criteria

### Error Classification

- [ ] **AC1**: 500 error triggers retry
  - Claude returns HTTP 500
  - First retry after 5 seconds
  - Up to 5 retries with exponential backoff
  - Checkpoint saved if all retries exhausted

- [ ] **AC2**: Overload error triggers retry
  - Claude returns "overloaded" error
  - Same retry behavior as 500
  - Eventually succeeds when capacity available

- [ ] **AC3**: Auth error fails fast
  - Claude returns authentication error
  - No retry attempted
  - Immediate failure with clear message

- [ ] **AC4**: Invalid command fails fast
  - Claude command syntax error
  - No retry attempted
  - Immediate failure with error details

### Retry Behavior

- [ ] **AC5**: Exponential backoff applied
  - Delays: 5s, 10s, 20s, 40s, 80s
  - Jitter applied (±25% by default)
  - Total maximum wait: ~155 seconds

- [ ] **AC6**: Retry logging
  - Log: "Claude command failed (attempt 1/5): 500 Internal Server Error"
  - Log: "Retrying in 5.2 seconds..."
  - Log: "Claude command succeeded on attempt 3"

- [ ] **AC7**: Configurable retry policy
  - Workflow YAML: `retry: { max_attempts: 3, initial_delay: 2s }`
  - Overrides default policy for that command

- [ ] **AC8**: Timeout triggers retry
  - Command runs longer than timeout (default 10 min)
  - Treated as transient error
  - Retried according to policy

### Integration

- [ ] **AC9**: Checkpoint on retry exhaustion
  - All 5 retries fail
  - Checkpoint saved with `Failed { retryable: true }`
  - Resume will retry the command again

- [ ] **AC10**: Resume continues retry sequence
  - Workflow interrupted during retry
  - Resume loads checkpoint
  - Continues retrying from where it left off

## Technical Details

### Implementation Approach

#### 1. Error Classification

```rust
/// Claude command error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum ClaudeError {
    #[error("HTTP {status}: {message}")]
    HttpError { status: u16, message: String },

    #[error("Service overloaded: {message}")]
    Overloaded { message: String },

    #[error("Network timeout after {duration:?}")]
    Timeout { duration: Duration },

    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Invalid command: {message}")]
    InvalidCommand { message: String },

    #[error("Process failed: {message}")]
    ProcessError { message: String },
}

impl ClaudeError {
    /// Check if error is transient (should retry)
    pub fn is_transient(&self) -> bool {
        match self {
            ClaudeError::HttpError { status, .. } => {
                // 5xx errors are transient
                *status >= 500 && *status < 600
            }
            ClaudeError::Overloaded { .. } => true,
            ClaudeError::Timeout { .. } => true,
            // Permanent errors
            ClaudeError::AuthenticationFailed { .. } => false,
            ClaudeError::InvalidCommand { .. } => false,
            ClaudeError::ProcessError { .. } => {
                // Process errors might be transient (e.g., spawning failed)
                // Be conservative and retry
                true
            }
        }
    }

    /// Parse error from Claude output/exit code
    pub fn from_process_output(output: &ProcessOutput) -> Self {
        let stderr = &output.stderr;

        // Check for specific error patterns
        if stderr.contains("500") || stderr.contains("Internal Server Error") {
            return ClaudeError::HttpError {
                status: 500,
                message: stderr.clone(),
            };
        }

        if stderr.contains("overload") || stderr.contains("rate limit") {
            return ClaudeError::Overloaded {
                message: stderr.clone(),
            };
        }

        if stderr.contains("authentication") || stderr.contains("unauthorized") {
            return ClaudeError::AuthenticationFailed {
                message: stderr.clone(),
            };
        }

        // Default to process error
        ClaudeError::ProcessError {
            message: stderr.clone(),
        }
    }
}
```

#### 2. Retry Policy Configuration

```rust
use stillwater::{RetryPolicy, JitterStrategy};
use std::time::Duration;

/// Default retry policy for Claude commands
///
/// IMPORTANT: RetryPolicy requires at least one bound (max_retries OR max_delay).
/// Without at least one bound, the policy will panic at construction.
pub fn default_claude_retry_policy() -> RetryPolicy {
    RetryPolicy::exponential(Duration::from_secs(5))
        .with_max_retries(5)        // Required: at least one bound must be set
        .with_jitter(JitterStrategy::Proportional(0.25))
        .with_max_delay(Duration::from_secs(120))
}

/// Parse retry policy from workflow configuration
///
/// NOTE: Ensures at least one bound (max_retries or max_delay) is always set
/// to satisfy RetryPolicy's validation requirements.
pub fn parse_retry_policy(config: Option<&RetryConfig>) -> RetryPolicy {
    match config {
        Some(cfg) => {
            let base = match cfg.strategy.as_deref() {
                Some("constant") => RetryPolicy::constant(cfg.initial_delay()),
                Some("linear") => RetryPolicy::linear(cfg.initial_delay()),
                Some("exponential") | None => RetryPolicy::exponential(cfg.initial_delay()),
                Some(other) => {
                    warn!("Unknown retry strategy '{}', using exponential", other);
                    RetryPolicy::exponential(cfg.initial_delay())
                }
            };

            // Always set max_retries to ensure at least one bound is set
            let mut policy = base.with_max_retries(cfg.max_attempts.unwrap_or(5));

            if let Some(jitter) = cfg.jitter {
                policy = policy.with_jitter(JitterStrategy::Proportional(jitter));
            }

            if let Some(max_delay) = cfg.max_delay {
                policy = policy.with_max_delay(max_delay);
            }

            policy
        }
        None => default_claude_retry_policy(),
    }
}

/// Retry configuration in workflow YAML
#[derive(Debug, Clone, Deserialize)]
pub struct RetryConfig {
    /// Retry strategy: constant, linear, exponential (default)
    pub strategy: Option<String>,
    /// Initial delay between retries
    pub initial_delay_secs: Option<u64>,
    /// Maximum retry attempts
    pub max_attempts: Option<u32>,
    /// Jitter factor (0.0 - 1.0)
    pub jitter: Option<f64>,
    /// Maximum delay cap
    pub max_delay_secs: Option<u64>,
}

impl RetryConfig {
    fn initial_delay(&self) -> Duration {
        Duration::from_secs(self.initial_delay_secs.unwrap_or(5))
    }

    fn max_delay(&self) -> Option<Duration> {
        self.max_delay_secs.map(Duration::from_secs)
    }
}
```

#### 3. Effect-Based Claude Execution with Retry

```rust
use stillwater::{Effect, EffectContext, RetryPolicy, RetryEvent};

/// Execute Claude command with retry for transient errors
pub fn execute_claude_with_retry(
    command: &str,
    retry_policy: RetryPolicy,
) -> Effect<ClaudeResult, ClaudeError, WorkflowEnv> {
    let command = command.to_string();

    Effect::retry_if(
        move || {
            let cmd = command.clone();
            execute_raw_claude(cmd)
        },
        retry_policy,
        |error| error.is_transient(),
    )
    .context(format!("Executing Claude command: {}", truncate(&command, 50)))
}

/// Execute Claude command with retry and observability hooks
pub fn execute_claude_with_hooks(
    command: &str,
    retry_policy: RetryPolicy,
) -> Effect<ClaudeResult, ClaudeError, WorkflowEnv> {
    let command = command.to_string();

    Effect::retry_with_hooks(
        move || {
            let cmd = command.clone();
            execute_raw_claude(cmd)
        },
        retry_policy,
        |event: &RetryEvent<'_, ClaudeError>| {
            log_retry_event(event);
        },
    )
    .map(|exhausted| exhausted.into_value())
    .map_err(|exhausted| {
        // Log final failure
        error!(
            "Claude command failed after {} attempts over {:?}: {}",
            exhausted.attempts,
            exhausted.total_duration,
            exhausted.final_error
        );
        exhausted.into_error()
    })
    .context(format!("Executing Claude command: {}", truncate(&command, 50)))
}

/// Log retry event for observability
fn log_retry_event(event: &RetryEvent<'_, ClaudeError>) {
    warn!(
        "Claude command failed (attempt {}/max): {}",
        event.attempt,
        event.error
    );

    if let Some(delay) = event.next_delay {
        info!(
            "Retrying in {:.1} seconds (elapsed: {:?})",
            delay.as_secs_f64(),
            event.elapsed
        );
    }
}

/// Raw Claude command execution (single attempt)
fn execute_raw_claude(command: String) -> Effect<ClaudeResult, ClaudeError, WorkflowEnv> {
    Effect::from_async(move |env: &WorkflowEnv| async move {
        let interpolated = interpolate_command(&command, &env.variables);

        let process_command = ProcessCommand {
            program: "claude".to_string(),
            args: vec!["--print".to_string(), interpolated],
            working_dir: Some(env.worktree_path.clone()),
            env: env.claude_env_vars(),
            timeout: env.claude_timeout,
            stdin: None,
            suppress_stderr: false,
        };

        let output = env.subprocess
            .runner()
            .run(process_command)
            .await
            .map_err(|e| ClaudeError::ProcessError { message: e.to_string() })?;

        if output.status.success() {
            Ok(ClaudeResult {
                output: output.stdout,
                json_log_location: output.json_log_location(),
            })
        } else {
            Err(ClaudeError::from_process_output(&output))
        }
    })
}
```

#### 4. Timeout Integration

```rust
use stillwater::TimeoutError;

/// Execute Claude command with timeout and retry
pub fn execute_claude_step(
    command: &str,
    config: &StepConfig,
) -> Effect<StepResult, StepError, WorkflowEnv> {
    let retry_policy = parse_retry_policy(config.retry.as_ref());
    let timeout = config.timeout.unwrap_or(Duration::from_secs(600)); // 10 min default

    execute_claude_with_hooks(command, retry_policy)
        .with_timeout(timeout)
        .map_err(|timeout_err| match timeout_err {
            TimeoutError::Timeout { duration } => {
                StepError::ClaudeTimeout {
                    command: command.to_string(),
                    duration,
                }
            }
            TimeoutError::Inner(claude_err) => StepError::ClaudeFailed(claude_err),
        })
        .map(|result| StepResult {
            success: true,
            output: Some(result.output),
            json_log_location: result.json_log_location,
            ..Default::default()
        })
}
```

#### 5. Workflow YAML Configuration

```yaml
name: example-with-retry

commands:
  - claude: "/process-data"
    retry:
      strategy: exponential  # constant, linear, or exponential
      initial_delay_secs: 5
      max_attempts: 5
      jitter: 0.25          # 25% jitter
      max_delay_secs: 120   # cap at 2 minutes
    timeout_secs: 600       # 10 minute timeout

  - claude: "/critical-command"
    retry:
      max_attempts: 10      # More retries for critical commands
      initial_delay_secs: 2

  - shell: "echo done"      # Shell commands don't retry by default
```

### Architecture Changes

#### Modified Components

1. **ClaudeExecutor** - Add retry wrapping
2. **StepExecutor** - Use effect-based Claude execution
3. **WorkflowConfig** - Parse retry configuration

#### New Components

1. **ClaudeError enum** - Error classification
2. **RetryConfig parsing** - YAML to RetryPolicy
3. **Retry hooks** - Observability integration

### Retry State in Checkpoints

When retries are exhausted, the checkpoint includes retry metadata:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryMetadata {
    /// Total retry attempts made
    pub attempts: u32,
    /// Total duration spent retrying
    pub total_duration: Duration,
    /// History of errors
    pub error_history: Vec<RetryAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryAttempt {
    pub attempt_number: u32,
    pub error: String,
    pub timestamp: DateTime<Utc>,
    pub delay_before_next: Option<Duration>,
}
```

## Dependencies

### Prerequisites
- **Spec 183**: Effect-Based Workflow Execution (Effect infrastructure)
- **Spec 121**: Claude Observability (JSON log location)

### Affected Components
- Claude command execution
- Workflow step execution
- Checkpoint format (retry metadata)

### External Dependencies
- `stillwater` with features: `["async", "jitter"]`
  - `async`: Required for all retry functions
  - `jitter`: Required for jitter functionality (without it, jitter calls silently no-op)

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_error_classification() {
    assert!(ClaudeError::HttpError { status: 500, message: "".into() }.is_transient());
    assert!(ClaudeError::HttpError { status: 503, message: "".into() }.is_transient());
    assert!(!ClaudeError::HttpError { status: 400, message: "".into() }.is_transient());
    assert!(ClaudeError::Overloaded { message: "".into() }.is_transient());
    assert!(ClaudeError::Timeout { duration: Duration::from_secs(60) }.is_transient());
    assert!(!ClaudeError::AuthenticationFailed { message: "".into() }.is_transient());
}

#[test]
fn test_retry_policy_parsing() {
    let config = RetryConfig {
        strategy: Some("exponential".into()),
        initial_delay_secs: Some(2),
        max_attempts: Some(3),
        jitter: Some(0.1),
        max_delay_secs: Some(30),
    };

    let policy = parse_retry_policy(Some(&config));
    // Verify policy properties...
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_retry_on_500_error() {
    let mut mock = MockClaudeExecutor::new();
    mock.expect_execute()
        .times(3)
        .returning(|_| Err(ClaudeError::HttpError { status: 500, message: "error".into() }))
        .then()
        .returning(|_| Ok(ClaudeResult { output: "success".into(), .. }));

    let env = create_env_with_mock(mock);
    let policy = RetryPolicy::constant(Duration::from_millis(10)).with_max_retries(5);

    let result = execute_claude_with_retry("/test", policy)
        .run(&env)
        .await;

    assert!(result.is_ok());
    // Verify 4 total attempts (3 failures + 1 success)
}

#[tokio::test]
async fn test_no_retry_on_auth_error() {
    let mut mock = MockClaudeExecutor::new();
    mock.expect_execute()
        .times(1) // Should only be called once
        .returning(|_| Err(ClaudeError::AuthenticationFailed { message: "bad token".into() }));

    let env = create_env_with_mock(mock);
    let policy = default_claude_retry_policy();

    let result = execute_claude_with_retry("/test", policy)
        .run(&env)
        .await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ClaudeError::AuthenticationFailed { .. }));
}

#[tokio::test]
async fn test_retry_exhaustion_saves_checkpoint() {
    let mock = MockClaudeExecutor::always_fails(ClaudeError::HttpError { status: 500, .. });
    let storage = InMemoryCheckpointStorage::new();
    let env = create_env_with_mock_and_storage(mock, storage.clone());

    let result = with_checkpointing(0, &claude_step("/test"))
        .run(&env)
        .await;

    assert!(result.is_err());

    // Checkpoint should exist with retry metadata
    let checkpoint = storage.load(&env.session_id).await.unwrap().unwrap();
    assert!(matches!(checkpoint.state, CheckpointState::Failed { retryable: true, .. }));
}
```

## Documentation Requirements

### Code Documentation
- Document ClaudeError variants and classification
- Document retry policy configuration options
- Document hook usage for observability

### User Documentation
- Document retry configuration in workflow YAML
- Explain transient vs permanent errors
- Troubleshoot "retry exhausted" errors

## Migration and Compatibility

### Breaking Changes
None - retry is additive behavior with sensible defaults.

### Compatibility
- Existing workflows get default retry policy
- Explicit retry: false disables retry if needed
- All existing tests continue to pass

## Success Metrics

### Quantitative
- 90%+ reduction in workflow failures due to transient Claude errors
- Average retry success within 3 attempts
- < 1% increase in workflow execution time from retry overhead

### Qualitative
- Clear visibility into retry behavior
- User confidence that transient errors are handled
- Reduced manual workflow restarts
