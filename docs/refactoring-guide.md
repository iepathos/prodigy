# Refactoring Guide: Separating I/O from Business Logic

## Quick Reference Checklist

### Signs a Function Needs Refactoring

- [ ] Contains `fs::`, `File::`, or any file operations
- [ ] Uses `println!`, `log::`, or other output operations
- [ ] Makes network calls or database queries
- [ ] Modifies global state or environment variables
- [ ] Is longer than 20 lines
- [ ] Has more than 3 levels of nesting
- [ ] Requires mock objects for testing
- [ ] Has "and" in its name (e.g., `load_and_validate`)
- [ ] Returns `Result` due to I/O, not logic errors
- [ ] Difficult to test edge cases

## Refactoring Patterns

### Pattern 1: Extract Validation Logic

**Before:**
```rust
pub fn load_and_validate_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&content)?;

    if config.timeout < 1 {
        log::error!("Invalid timeout: {}", config.timeout);
        return Err(anyhow!("Timeout must be positive"));
    }

    if config.workers > 100 {
        log::warn!("High worker count: {}", config.workers);
    }

    Ok(config)
}
```

**After:**
```rust
// Pure function in src/core/config/
pub struct ConfigValidation {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn validate_config(config: &Config) -> ConfigValidation {
    let mut validation = ConfigValidation::default();

    if config.timeout < 1 {
        validation.errors.push(format!("Timeout must be positive, got {}", config.timeout));
    }

    if config.workers > 100 {
        validation.warnings.push(format!("High worker count: {}", config.workers));
    }

    validation
}

// I/O wrapper in src/config/
pub fn load_and_validate_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&content)?;

    let validation = core::config::validate_config(&config);

    for warning in &validation.warnings {
        log::warn!("{}", warning);
    }

    if !validation.errors.is_empty() {
        for error in &validation.errors {
            log::error!("{}", error);
        }
        return Err(anyhow!("Config validation failed"));
    }

    Ok(config)
}
```

### Pattern 2: Separate Calculation from Persistence

**Before:**
```rust
pub async fn update_session_metrics(&mut self, event: Event) -> Result<()> {
    self.event_count += 1;
    self.last_event_time = Utc::now();

    if event.is_error() {
        self.error_count += 1;
        self.status = SessionStatus::Failed;
    }

    let metrics_file = format!("sessions/{}/metrics.json", self.id);
    fs::write(&metrics_file, serde_json::to_string(&self)?)?;

    Ok(())
}
```

**After:**
```rust
// Pure function in src/core/session/
#[derive(Clone)]
pub struct SessionMetrics {
    pub event_count: u32,
    pub error_count: u32,
    pub last_event_time: DateTime<Utc>,
    pub status: SessionStatus,
}

pub fn calculate_metrics_update(
    current: SessionMetrics,
    event: Event,
    timestamp: DateTime<Utc>,
) -> SessionMetrics {
    SessionMetrics {
        event_count: current.event_count + 1,
        error_count: if event.is_error() {
            current.error_count + 1
        } else {
            current.error_count
        },
        last_event_time: timestamp,
        status: if event.is_error() {
            SessionStatus::Failed
        } else {
            current.status
        },
    }
}

// I/O wrapper in src/session/
pub async fn update_session_metrics(
    session_id: &str,
    event: Event,
) -> Result<()> {
    let current = load_metrics(session_id).await?;
    let updated = core::session::calculate_metrics_update(
        current,
        event,
        Utc::now(),
    );
    save_metrics(session_id, &updated).await?;
    Ok(())
}
```

### Pattern 3: Extract Complex Business Rules

**Before:**
```rust
pub fn process_work_item(&mut self, item: &WorkItem) -> Result<()> {
    let log_file = File::create(format!("logs/{}.log", item.id))?;

    if item.priority > 5 && self.high_priority_count < 10 {
        writeln!(log_file, "Processing high priority item")?;
        self.high_priority_count += 1;
        self.queue.push_front(item.clone());
    } else if item.retries < 3 && !item.is_toxic {
        writeln!(log_file, "Queuing for retry")?;
        let mut retry_item = item.clone();
        retry_item.retries += 1;
        self.retry_queue.push(retry_item);
    } else {
        writeln!(log_file, "Moving to dead letter queue")?;
        self.dlq.push(item.clone());
    }

    Ok(())
}
```

**After:**
```rust
// Pure functions in src/core/queue/
pub enum ItemDestination {
    HighPriority,
    Retry(u32), // with retry count
    DeadLetter,
}

pub fn determine_item_destination(
    item: &WorkItem,
    high_priority_count: usize,
) -> ItemDestination {
    if should_process_as_high_priority(item, high_priority_count) {
        ItemDestination::HighPriority
    } else if should_retry(item) {
        ItemDestination::Retry(item.retries + 1)
    } else {
        ItemDestination::DeadLetter
    }
}

fn should_process_as_high_priority(
    item: &WorkItem,
    current_count: usize,
) -> bool {
    item.priority > 5 && current_count < 10
}

fn should_retry(item: &WorkItem) -> bool {
    item.retries < 3 && !item.is_toxic
}

// I/O wrapper in src/queue/
pub fn process_work_item(
    queue_state: &mut QueueState,
    item: &WorkItem,
) -> Result<()> {
    let destination = core::queue::determine_item_destination(
        item,
        queue_state.high_priority_count,
    );

    let log_file = File::create(format!("logs/{}.log", item.id))?;

    match destination {
        ItemDestination::HighPriority => {
            writeln!(log_file, "Processing high priority item")?;
            queue_state.high_priority_count += 1;
            queue_state.queue.push_front(item.clone());
        }
        ItemDestination::Retry(retry_count) => {
            writeln!(log_file, "Queuing for retry")?;
            let mut retry_item = item.clone();
            retry_item.retries = retry_count;
            queue_state.retry_queue.push(retry_item);
        }
        ItemDestination::DeadLetter => {
            writeln!(log_file, "Moving to dead letter queue")?;
            queue_state.dlq.push(item.clone());
        }
    }

    Ok(())
}
```

### Pattern 4: Pipeline Transformations

**Before:**
```rust
pub fn process_workflow(path: &Path) -> Result<ExecutionPlan> {
    let content = fs::read_to_string(path)?;
    let mut workflow: Workflow = serde_yaml::from_str(&content)?;

    // Validation
    if workflow.steps.is_empty() {
        return Err(anyhow!("Empty workflow"));
    }

    // Enrichment
    for step in &mut workflow.steps {
        if step.timeout.is_none() {
            step.timeout = Some(60);
        }
        step.id = Uuid::new_v4().to_string();
    }

    // Optimization
    workflow.steps.retain(|s| !s.skip);

    // Planning
    let plan = ExecutionPlan {
        workflow_id: workflow.id,
        steps: workflow.steps,
        parallel_groups: compute_parallel_groups(&workflow),
    };

    fs::write("execution_plan.json", serde_json::to_string(&plan)?)?;

    Ok(plan)
}
```

**After:**
```rust
// Pure pipeline in src/core/workflow/
pub fn transform_workflow_to_plan(workflow: Workflow) -> Result<ExecutionPlan> {
    workflow
        .validate()
        .map(enrich_steps)
        .map(optimize_workflow)
        .map(create_execution_plan)
}

fn validate(workflow: Workflow) -> Result<Workflow> {
    if workflow.steps.is_empty() {
        Err(WorkflowError::EmptyWorkflow)
    } else {
        Ok(workflow)
    }
}

fn enrich_steps(mut workflow: Workflow) -> Workflow {
    workflow.steps = workflow.steps
        .into_iter()
        .map(|mut step| {
            step.timeout = step.timeout.or(Some(60));
            step.id = generate_step_id();
            step
        })
        .collect();
    workflow
}

fn optimize_workflow(mut workflow: Workflow) -> Workflow {
    workflow.steps.retain(|s| !s.skip);
    workflow
}

fn create_execution_plan(workflow: Workflow) -> ExecutionPlan {
    ExecutionPlan {
        workflow_id: workflow.id.clone(),
        parallel_groups: compute_parallel_groups(&workflow),
        steps: workflow.steps,
    }
}

// I/O wrapper
pub fn process_workflow(path: &Path) -> Result<ExecutionPlan> {
    let content = fs::read_to_string(path)?;
    let workflow = serde_yaml::from_str(&content)?;

    let plan = core::workflow::transform_workflow_to_plan(workflow)?;

    fs::write("execution_plan.json", serde_json::to_string(&plan)?)?;

    Ok(plan)
}
```

## Testing Strategies

### Testing Pure Functions

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_errors() {
        let config = Config {
            timeout: -1,
            workers: 150,
        };

        let validation = validate_config(&config);

        assert_eq!(validation.errors.len(), 1);
        assert_eq!(validation.warnings.len(), 1);
        assert!(validation.errors[0].contains("positive"));
        assert!(validation.warnings[0].contains("High worker"));
    }

    #[test]
    fn test_item_destination_high_priority() {
        let item = WorkItem {
            priority: 10,
            retries: 0,
            is_toxic: false,
            ..Default::default()
        };

        let destination = determine_item_destination(&item, 5);

        assert!(matches!(destination, ItemDestination::HighPriority));
    }

    #[test]
    fn test_metrics_calculation() {
        let current = SessionMetrics {
            event_count: 5,
            error_count: 1,
            last_event_time: Utc::now() - Duration::hours(1),
            status: SessionStatus::Running,
        };

        let error_event = Event::Error("test".into());
        let now = Utc::now();

        let updated = calculate_metrics_update(current, error_event, now);

        assert_eq!(updated.event_count, 6);
        assert_eq!(updated.error_count, 2);
        assert_eq!(updated.status, SessionStatus::Failed);
        assert_eq!(updated.last_event_time, now);
    }
}
```

### Testing I/O Wrappers

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_config_loading() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");

        let valid_config = r#"{"timeout": 30, "workers": 10}"#;
        fs::write(&config_path, valid_config).unwrap();

        let result = load_and_validate_config(&config_path).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().timeout, 30);
    }

    #[tokio::test]
    async fn test_invalid_config_rejection() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");

        let invalid_config = r#"{"timeout": -1, "workers": 10}"#;
        fs::write(&config_path, invalid_config).unwrap();

        let result = load_and_validate_config(&config_path).await;

        assert!(result.is_err());
    }
}
```

## Common Pitfalls

### 1. Partial Extraction
**Problem:** Only extracting part of the logic, leaving I/O mixed in.

**Solution:** Extract ALL business logic, even simple conditionals.

### 2. Hidden Dependencies
**Problem:** Pure functions calling other functions that do I/O.

**Solution:** Ensure entire call chain is pure.

### 3. Over-engineering
**Problem:** Creating too many abstractions for simple operations.

**Solution:** Keep simple I/O operations in shell; only extract complex logic.

### 4. Mutable Parameters
**Problem:** Passing mutable references to pure functions.

**Solution:** Take ownership or use immutable references; return new values.

## Step-by-Step Refactoring Process

1. **Identify Target Function**
   - Look for functions mixing I/O and logic
   - Prioritize functions that are hard to test

2. **Map Dependencies**
   - List all I/O operations
   - List all business logic operations
   - Identify data flow between them

3. **Extract Pure Logic**
   - Create new function(s) in `src/core/`
   - Move all business logic
   - Replace I/O with parameters

4. **Create I/O Wrapper**
   - Keep original function signature
   - Load required data
   - Call pure function
   - Handle results and side effects

5. **Update Tests**
   - Write unit tests for pure functions
   - Write integration tests for wrappers
   - Remove unnecessary mocks

6. **Refactor Callers**
   - Update direct callers if needed
   - Consider exposing pure function for reuse

## Measuring Success

### Before Refactoring
- Test requires 10+ lines of setup
- Test uses mock objects
- Test is flaky or slow
- Function is hard to understand
- Bug fixes often break other things

### After Refactoring
- Test is 3-5 lines
- Test needs no mocks
- Test is fast and deterministic
- Function purpose is clear
- Changes are isolated and safe

## Real-World Example from Prodigy

### Original Mixed Function (Before)
```rust
// In src/orchestrator.rs
pub async fn execute_step(
    &mut self,
    step: &WorkflowStep,
    context: &mut Context,
) -> Result<StepResult> {
    log::info!("Executing step: {}", step.name);

    let start = Instant::now();

    // Complex timeout calculation mixed with I/O
    let timeout = if let Some(t) = step.timeout {
        t
    } else if context.is_critical {
        300
    } else {
        self.config.default_timeout
    };

    // Validation mixed with execution
    if step.retries > self.config.max_retries {
        log::error!("Max retries exceeded");
        self.metrics.failed_steps += 1;
        fs::write(
            format!("failures/{}.json", step.id),
            serde_json::to_string(&step)?,
        )?;
        return Err(anyhow!("Max retries exceeded"));
    }

    // Execute command (I/O)
    let output = run_command(&step.command, timeout).await?;

    // Process results (logic mixed with I/O)
    let result = if output.success {
        self.metrics.successful_steps += 1;
        StepResult::Success(output.stdout)
    } else {
        self.metrics.failed_steps += 1;
        if step.allow_failure {
            StepResult::Warning(output.stderr)
        } else {
            StepResult::Failure(output.stderr)
        }
    };

    // Save metrics (I/O)
    self.metrics.total_duration += start.elapsed();
    fs::write(
        "metrics.json",
        serde_json::to_string(&self.metrics)?,
    )?;

    log::info!("Step completed: {:?}", result);
    Ok(result)
}
```

### Refactored Version (After)

```rust
// In src/core/execution/mod.rs - Pure business logic
pub fn calculate_timeout(
    step_timeout: Option<u64>,
    is_critical: bool,
    default_timeout: u64,
) -> u64 {
    step_timeout.unwrap_or_else(|| {
        if is_critical {
            300
        } else {
            default_timeout
        }
    })
}

pub fn validate_retry_limit(
    current_retries: u32,
    max_retries: u32,
) -> Result<(), ValidationError> {
    if current_retries > max_retries {
        Err(ValidationError::MaxRetriesExceeded)
    } else {
        Ok(())
    }
}

pub fn process_command_result(
    output: CommandOutput,
    allow_failure: bool,
) -> StepResult {
    if output.success {
        StepResult::Success(output.stdout)
    } else if allow_failure {
        StepResult::Warning(output.stderr)
    } else {
        StepResult::Failure(output.stderr)
    }
}

pub fn update_metrics(
    metrics: Metrics,
    result: &StepResult,
    duration: Duration,
) -> Metrics {
    Metrics {
        successful_steps: if result.is_success() {
            metrics.successful_steps + 1
        } else {
            metrics.successful_steps
        },
        failed_steps: if result.is_failure() {
            metrics.failed_steps + 1
        } else {
            metrics.failed_steps
        },
        total_duration: metrics.total_duration + duration,
        ..metrics
    }
}

// In src/orchestrator.rs - I/O Shell
pub async fn execute_step(
    &mut self,
    step: &WorkflowStep,
    context: &mut Context,
) -> Result<StepResult> {
    log::info!("Executing step: {}", step.name);
    let start = Instant::now();

    // Use pure function for timeout calculation
    let timeout = core::execution::calculate_timeout(
        step.timeout,
        context.is_critical,
        self.config.default_timeout,
    );

    // Use pure function for validation
    if let Err(e) = core::execution::validate_retry_limit(
        step.retries,
        self.config.max_retries,
    ) {
        log::error!("Validation failed: {:?}", e);
        self.save_failure(step).await?;
        return Err(anyhow!("{:?}", e));
    }

    // I/O: Execute command
    let output = run_command(&step.command, timeout).await?;

    // Use pure function to process result
    let result = core::execution::process_command_result(
        output,
        step.allow_failure,
    );

    // Use pure function to update metrics
    self.metrics = core::execution::update_metrics(
        self.metrics.clone(),
        &result,
        start.elapsed(),
    );

    // I/O: Save metrics
    self.save_metrics().await?;

    log::info!("Step completed: {:?}", result);
    Ok(result)
}

async fn save_failure(&self, step: &WorkflowStep) -> Result<()> {
    fs::write(
        format!("failures/{}.json", step.id),
        serde_json::to_string(&step)?,
    )?;
    Ok(())
}

async fn save_metrics(&self) -> Result<()> {
    fs::write(
        "metrics.json",
        serde_json::to_string(&self.metrics)?,
    )?;
    Ok(())
}
```

This refactoring achieves:
- **Testable** pure functions with no I/O
- **Clear** separation of concerns
- **Reusable** business logic
- **Maintainable** code structure
- **Fast** unit tests without mocks