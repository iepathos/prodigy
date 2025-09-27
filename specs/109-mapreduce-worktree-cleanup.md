---
number: 109
title: MapReduce Worktree Cleanup Implementation
category: reliability
priority: critical
status: draft
dependencies: []
created: 2025-09-27
---

# Specification 109: MapReduce Worktree Cleanup Implementation

## Context

The current MapReduce implementation creates git worktrees for each agent but never cleans them up, leading to resource leaks and disk space bloat. Analysis shows that worktrees are created in `/Users/{user}/.prodigy/worktrees/{project-name}/` but remain after job completion, potentially accumulating indefinitely with each MapReduce execution.

Current problems:
- Agent worktrees persist after job completion
- No cleanup mechanism for failed agents
- Potential disk space exhaustion over time
- Orphaned worktrees from interrupted jobs
- No limit on number of concurrent worktrees

The MapReduce agent lifecycle creates worktrees but the cleanup phase is missing, resulting in resource leaks that can impact system performance and stability.

## Objective

Implement comprehensive worktree cleanup mechanisms for MapReduce jobs that ensure proper resource management, prevent disk space bloat, and handle cleanup for both successful and failed executions.

## Requirements

### Functional Requirements

#### Automatic Cleanup
- Clean up agent worktrees after successful map phase completion
- Clean up worktrees for failed agents immediately upon failure detection
- Clean up all job-related worktrees when job is cancelled or interrupted
- Support cleanup during graceful shutdown of MapReduce coordinator

#### Manual Cleanup Operations
- Provide CLI command to clean orphaned worktrees from previous jobs
- Support force cleanup for stuck or corrupted worktrees
- Allow selective cleanup by job ID or age threshold
- Support dry-run mode for cleanup operations

#### Resource Monitoring
- Track active worktrees per job and globally
- Monitor disk space usage of worktree directories
- Warn when approaching worktree limits or disk space thresholds
- Provide metrics on cleanup success/failure rates

### Non-Functional Requirements
- Cleanup operations should complete within 30 seconds per worktree
- Support concurrent cleanup of multiple worktrees
- Cleanup should be atomic (either fully clean or leave intact)
- Robust error handling for cleanup failures
- Minimal impact on ongoing MapReduce operations

## Acceptance Criteria

- [ ] Agent worktrees are automatically cleaned up after successful completion
- [ ] Failed agent worktrees are cleaned up immediately upon failure
- [ ] Job interruption triggers cleanup of all associated worktrees
- [ ] CLI command `prodigy worktree clean --mapreduce` removes orphaned worktrees
- [ ] Cleanup operations are logged with appropriate detail level
- [ ] Cleanup failures are handled gracefully without affecting job execution
- [ ] Resource monitoring provides visibility into worktree usage
- [ ] Performance benchmarks show minimal overhead from cleanup operations

## Technical Details

### Implementation Approach

#### 1. Agent Lifecycle Integration

Extend the existing agent lifecycle to include proper cleanup phases:

```rust
pub struct MapReduceAgent {
    // ... existing fields
    cleanup_on_drop: bool,
    worktree_path: Option<PathBuf>,
}

impl Drop for MapReduceAgent {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            if let Some(path) = &self.worktree_path {
                let _ = cleanup_worktree_async(path.clone());
            }
        }
    }
}
```

#### 2. Cleanup Coordinator

Create a dedicated cleanup service that manages worktree lifecycle:

```rust
pub struct WorktreeCleanupCoordinator {
    active_worktrees: Arc<Mutex<HashMap<JobId, Vec<WorktreeHandle>>>>,
    cleanup_queue: Arc<Mutex<VecDeque<CleanupTask>>>,
    cleanup_executor: ThreadPool,
}

pub enum CleanupTask {
    Immediate { worktree_path: PathBuf, job_id: JobId },
    Scheduled { worktree_path: PathBuf, delay: Duration },
    Batch { worktree_paths: Vec<PathBuf> },
}

impl WorktreeCleanupCoordinator {
    pub async fn schedule_cleanup(&self, task: CleanupTask) -> Result<(), CleanupError>;
    pub async fn force_cleanup_job(&self, job_id: &JobId) -> Result<usize, CleanupError>;
    pub async fn cleanup_orphaned_worktrees(&self, max_age: Duration) -> Result<usize, CleanupError>;
}
```

#### 3. CLI Integration

Extend existing worktree commands with MapReduce-specific cleanup:

```rust
#[derive(Parser)]
pub struct WorktreeCleanCommand {
    /// Clean MapReduce-specific worktrees
    #[clap(long)]
    mapreduce: bool,

    /// Clean worktrees older than specified duration
    #[clap(long, value_parser = parse_duration)]
    older_than: Option<Duration>,

    /// Force cleanup even if worktree appears active
    #[clap(long)]
    force: bool,

    /// Show what would be cleaned without actually cleaning
    #[clap(long)]
    dry_run: bool,

    /// Specific job ID to clean
    #[clap(long)]
    job_id: Option<String>,
}
```

#### 4. Resource Monitoring

Implement monitoring for worktree resource usage:

```rust
pub struct WorktreeResourceMonitor {
    disk_usage_threshold: u64,
    max_worktrees_per_job: usize,
    max_total_worktrees: usize,
}

pub struct WorktreeMetrics {
    pub active_worktrees: usize,
    pub total_disk_usage: u64,
    pub cleanup_operations: usize,
    pub cleanup_failures: usize,
    pub average_cleanup_time: Duration,
}

impl WorktreeResourceMonitor {
    pub fn check_limits(&self) -> Result<(), ResourceLimitError>;
    pub fn get_metrics(&self) -> WorktreeMetrics;
    pub fn cleanup_recommendation(&self) -> Option<CleanupRecommendation>;
}
```

### Integration Points

#### MapReduce Coordinator Integration
```rust
impl PhaseCoordinator {
    pub async fn execute_workflow(
        &self,
        environment: ExecutionEnvironment,
        subprocess_manager: Arc<SubprocessManager>,
    ) -> MapReduceResult<PhaseResult> {
        let cleanup_coordinator = WorktreeCleanupCoordinator::new();

        // Register cleanup on job completion
        let _cleanup_guard = cleanup_coordinator.register_job(&self.job_id);

        // ... existing workflow execution

        // Explicit cleanup on success
        cleanup_coordinator.cleanup_job(&self.job_id).await?;

        Ok(result)
    }
}
```

#### Error Handling Integration
```rust
impl MapPhaseExecutor {
    async fn execute_agent(&self, agent: MapReduceAgent) -> Result<AgentResult, AgentError> {
        // Register worktree for cleanup
        let worktree_path = agent.worktree_path.clone();
        let cleanup_guard = self.cleanup_coordinator.register_worktree(worktree_path);

        let result = agent.execute().await;

        match result {
            Ok(success) => {
                // Schedule cleanup after short delay for result collection
                cleanup_guard.schedule_cleanup(Duration::from_secs(30)).await?;
                Ok(success)
            }
            Err(error) => {
                // Immediate cleanup on failure
                cleanup_guard.immediate_cleanup().await?;
                Err(error)
            }
        }
    }
}
```

### Configuration Options

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeCleanupConfig {
    /// Enable automatic cleanup of worktrees
    pub auto_cleanup: bool,

    /// Delay before cleaning up successful agent worktrees
    pub cleanup_delay_secs: u64,

    /// Maximum number of worktrees per job
    pub max_worktrees_per_job: usize,

    /// Maximum total worktrees across all jobs
    pub max_total_worktrees: usize,

    /// Disk usage threshold for triggering cleanup warnings
    pub disk_usage_threshold_mb: u64,

    /// Enable resource monitoring
    pub enable_monitoring: bool,

    /// Cleanup timeout per worktree
    pub cleanup_timeout_secs: u64,
}

impl Default for WorktreeCleanupConfig {
    fn default() -> Self {
        Self {
            auto_cleanup: true,
            cleanup_delay_secs: 30,
            max_worktrees_per_job: 50,
            max_total_worktrees: 200,
            disk_usage_threshold_mb: 1024, // 1GB
            enable_monitoring: true,
            cleanup_timeout_secs: 30,
        }
    }
}
```

### Error Recovery

```rust
#[derive(Debug, thiserror::Error)]
pub enum CleanupError {
    #[error("Failed to remove worktree at {path}: {source}")]
    RemovalFailed { path: PathBuf, source: std::io::Error },

    #[error("Worktree cleanup timeout after {timeout:?}")]
    Timeout { timeout: Duration },

    #[error("Worktree is still active and cannot be cleaned")]
    WorktreeActive,

    #[error("Git operation failed: {0}")]
    GitError(String),

    #[error("Permission denied for worktree cleanup: {path}")]
    PermissionDenied { path: PathBuf },
}

impl CleanupError {
    pub fn is_recoverable(&self) -> bool {
        matches!(self, CleanupError::Timeout { .. } | CleanupError::WorktreeActive)
    }

    pub fn should_retry(&self) -> bool {
        matches!(self, CleanupError::Timeout { .. })
    }
}
```

## Testing Strategy

### Unit Tests
- Test worktree creation and cleanup lifecycle
- Test cleanup coordinator task scheduling and execution
- Test resource monitoring threshold detection
- Test error handling for various cleanup failure scenarios
- Test CLI command parsing and validation

### Integration Tests
- Test end-to-end MapReduce job with automatic cleanup
- Test cleanup behavior during job interruption
- Test CLI cleanup commands with various options
- Test resource limit enforcement
- Test cleanup performance under high worktree counts

### Performance Tests
- Benchmark cleanup time vs. worktree size
- Test concurrent cleanup operations
- Measure overhead of resource monitoring
- Test cleanup under resource pressure

### Reliability Tests
- Test cleanup robustness with corrupted worktrees
- Test cleanup during system resource exhaustion
- Test cleanup recovery after process restart
- Test cleanup behavior with permission issues

## Migration Strategy

### Phase 1: Core Cleanup Infrastructure
1. Implement `WorktreeCleanupCoordinator` and basic cleanup mechanisms
2. Add cleanup hooks to agent lifecycle
3. Implement resource monitoring foundation

### Phase 2: MapReduce Integration
1. Integrate cleanup coordinator with MapReduce phases
2. Add automatic cleanup triggers
3. Implement error handling and recovery

### Phase 3: CLI and Monitoring
1. Extend CLI commands for manual cleanup operations
2. Add comprehensive resource monitoring
3. Implement cleanup recommendations and warnings

### Phase 4: Configuration and Optimization
1. Add configurable cleanup policies
2. Optimize cleanup performance
3. Add advanced monitoring and alerting

## Documentation Requirements

- Update MapReduce documentation with cleanup behavior
- Document CLI cleanup commands and options
- Create troubleshooting guide for cleanup issues
- Document resource monitoring and configuration options
- Add examples of cleanup best practices

## Risk Assessment

### High Risk
- **Cleanup Interference**: Aggressive cleanup might interfere with result collection
- **Resource Limits**: Incorrect limit calculation could prevent job execution
- **Data Loss**: Premature cleanup could lose important debugging information

### Medium Risk
- **Performance Impact**: Cleanup operations might slow down job execution
- **Configuration Complexity**: Too many cleanup options might confuse users
- **Error Propagation**: Cleanup failures might mask more important errors

### Mitigation Strategies
- Implement delayed cleanup with configurable grace periods
- Provide clear logging for all cleanup operations
- Make cleanup optional with sensible defaults
- Separate cleanup errors from job execution errors
- Include rollback mechanisms for failed cleanup operations