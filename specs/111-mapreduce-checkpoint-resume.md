---
number: 111
title: MapReduce Checkpoint/Resume Functionality
category: reliability
priority: critical
status: draft
dependencies: [109]
created: 2025-09-27
---

# Specification 111: MapReduce Checkpoint/Resume Functionality

## Context

The current MapReduce implementation lacks robust checkpoint saving and resume capability, making it impossible to recover from interruptions or failures without starting from scratch. This creates significant problems for long-running jobs processing large datasets.

Current gaps:
- No checkpoint saving during map phase execution
- Resume functionality exists but is not fully implemented
- No checkpoint validation or corruption detection
- Missing checkpoint cleanup for completed jobs
- No progress preservation across agent failures
- Checkpoint state is not comprehensive enough for reliable resume

Long-running MapReduce jobs (processing hundreds or thousands of items) need reliable checkpoint and resume functionality to handle interruptions, system failures, and resource constraints without losing progress.

## Objective

Implement comprehensive checkpoint/resume functionality for MapReduce workflows that enables reliable recovery from interruptions, preserves progress across failures, and provides robust state management for long-running jobs.

## Requirements

### Functional Requirements

#### Checkpoint Creation
- Save checkpoints at configurable intervals during map phase
- Create checkpoints after each completed agent batch
- Save checkpoint before and after phase transitions
- Include complete work item processing state
- Store agent execution results and metadata

#### Resume Capability
- Resume from any valid checkpoint
- Validate checkpoint integrity before resume
- Handle partial agent completion during resume
- Preserve variable state and context
- Continue from exact interruption point

#### State Management
- Track completed, in-progress, and pending work items
- Maintain agent assignment and result mapping
- Store phase transition state and variables
- Preserve error state and DLQ items
- Track resource allocation and cleanup state

#### Checkpoint Operations
- CLI commands to list and manage checkpoints
- Automatic checkpoint cleanup for completed jobs
- Checkpoint validation and repair tools
- Export/import functionality for checkpoint data
- Checkpoint compression and optimization

### Non-Functional Requirements
- Checkpoint creation should add < 5% overhead to execution time
- Resume should start within 30 seconds of command execution
- Checkpoint files should be compressed and space-efficient
- Support for concurrent checkpointing without blocking execution
- Automatic cleanup of old checkpoints to prevent disk bloat

## Acceptance Criteria

- [ ] MapReduce jobs create checkpoints every N completed items (configurable)
- [ ] `prodigy resume <job_id>` resumes from latest valid checkpoint
- [ ] Checkpoint validation detects corruption and offers repair options
- [ ] Resume preserves exact execution state including variables and results
- [ ] CLI commands provide checkpoint management and inspection tools
- [ ] Checkpoint overhead is measurably < 5% of total execution time
- [ ] Failed resume attempts provide clear error messages and guidance
- [ ] Automatic checkpoint cleanup prevents disk space issues

## Technical Details

### Implementation Approach

#### 1. Enhanced Checkpoint Structure

Extend the existing checkpoint system with comprehensive state:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceCheckpoint {
    /// Basic checkpoint metadata
    pub metadata: CheckpointMetadata,

    /// Complete execution state
    pub execution_state: ExecutionState,

    /// Work item processing status
    pub work_item_state: WorkItemState,

    /// Agent execution state
    pub agent_state: AgentState,

    /// Variable and context state
    pub variable_state: VariableState,

    /// Resource allocation state
    pub resource_state: ResourceState,

    /// Error and DLQ state
    pub error_state: ErrorState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    pub checkpoint_id: String,
    pub job_id: String,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub phase: PhaseType,
    pub total_work_items: usize,
    pub completed_items: usize,
    pub checkpoint_reason: CheckpointReason,
    pub integrity_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckpointReason {
    Interval,
    PhaseTransition,
    Manual,
    BeforeShutdown,
    BatchComplete,
    ErrorRecovery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionState {
    pub current_phase: PhaseType,
    pub phase_start_time: DateTime<Utc>,
    pub setup_results: Option<PhaseResult>,
    pub map_results: Option<MapPhaseResults>,
    pub reduce_results: Option<PhaseResult>,
    pub workflow_variables: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItemState {
    pub pending_items: Vec<WorkItem>,
    pub in_progress_items: HashMap<String, WorkItemProgress>,
    pub completed_items: Vec<CompletedWorkItem>,
    pub failed_items: Vec<FailedWorkItem>,
    pub current_batch: Option<WorkItemBatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub active_agents: HashMap<String, AgentInfo>,
    pub agent_assignments: HashMap<String, Vec<String>>, // agent_id -> work_item_ids
    pub agent_results: HashMap<String, AgentResult>,
    pub resource_allocation: HashMap<String, ResourceAllocation>,
}
```

#### 2. Checkpoint Manager

Create a dedicated service for checkpoint operations:

```rust
pub struct CheckpointManager {
    storage: Arc<dyn CheckpointStorage>,
    config: CheckpointConfig,
    job_id: String,
    compression: Option<CompressionConfig>,
}

impl CheckpointManager {
    pub async fn create_checkpoint(
        &self,
        execution_state: &ExecutionState,
        reason: CheckpointReason,
    ) -> Result<CheckpointId, CheckpointError> {
        let checkpoint = self.build_checkpoint(execution_state, reason).await?;

        // Validate checkpoint before saving
        self.validate_checkpoint(&checkpoint)?;

        // Save checkpoint with atomic operation
        let checkpoint_id = self.storage.save_checkpoint(&checkpoint).await?;

        // Update checkpoint index
        self.update_checkpoint_index(&checkpoint_id, &checkpoint.metadata).await?;

        // Cleanup old checkpoints if needed
        self.cleanup_old_checkpoints().await?;

        Ok(checkpoint_id)
    }

    pub async fn resume_from_checkpoint(
        &self,
        checkpoint_id: Option<CheckpointId>,
    ) -> Result<ResumeState, CheckpointError> {
        let checkpoint_id = match checkpoint_id {
            Some(id) => id,
            None => self.find_latest_checkpoint().await?
                .ok_or(CheckpointError::NoCheckpointFound)?,
        };

        let checkpoint = self.storage.load_checkpoint(&checkpoint_id).await?;

        // Validate checkpoint integrity
        self.validate_checkpoint_integrity(&checkpoint)?;

        // Build resume state
        let resume_state = self.build_resume_state(checkpoint).await?;

        Ok(resume_state)
    }

    pub async fn list_checkpoints(&self) -> Result<Vec<CheckpointInfo>, CheckpointError> {
        self.storage.list_checkpoints(&self.job_id).await
    }

    pub async fn delete_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<(), CheckpointError> {
        self.storage.delete_checkpoint(checkpoint_id).await?;
        self.update_checkpoint_index_remove(checkpoint_id).await?;
        Ok(())
    }

    pub async fn validate_checkpoint(&self, checkpoint: &MapReduceCheckpoint) -> Result<(), CheckpointError> {
        // Validate structure
        self.validate_checkpoint_structure(checkpoint)?;

        // Validate data consistency
        self.validate_data_consistency(checkpoint)?;

        // Validate integrity hash
        self.validate_integrity_hash(checkpoint)?;

        Ok(())
    }

    async fn cleanup_old_checkpoints(&self) -> Result<(), CheckpointError> {
        if let Some(retention) = &self.config.retention_policy {
            let checkpoints = self.list_checkpoints().await?;
            let to_delete = retention.select_for_deletion(&checkpoints);

            for checkpoint_id in to_delete {
                self.delete_checkpoint(&checkpoint_id).await?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CheckpointConfig {
    pub interval_items: Option<usize>,
    pub interval_duration: Option<Duration>,
    pub enable_compression: bool,
    pub retention_policy: Option<RetentionPolicy>,
    pub validate_on_save: bool,
    pub validate_on_load: bool,
}

#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    pub max_checkpoints: Option<usize>,
    pub max_age: Option<Duration>,
    pub keep_final: bool,
}

impl RetentionPolicy {
    fn select_for_deletion(&self, checkpoints: &[CheckpointInfo]) -> Vec<CheckpointId> {
        let mut to_delete = Vec::new();
        let mut sorted = checkpoints.to_vec();
        sorted.sort_by_key(|c| c.created_at);

        // Apply max_checkpoints limit
        if let Some(max) = self.max_checkpoints {
            if sorted.len() > max {
                let excess = sorted.len() - max;
                for checkpoint in sorted.iter().take(excess) {
                    if !self.keep_final || !checkpoint.is_final {
                        to_delete.push(checkpoint.id.clone());
                    }
                }
            }
        }

        // Apply max_age limit
        if let Some(max_age) = self.max_age {
            let cutoff = Utc::now() - max_age;
            for checkpoint in &sorted {
                if checkpoint.created_at < cutoff {
                    if !self.keep_final || !checkpoint.is_final {
                        to_delete.push(checkpoint.id.clone());
                    }
                }
            }
        }

        to_delete
    }
}
```

#### 3. Resume State Builder

Create resume state from checkpoint data:

```rust
#[derive(Debug)]
pub struct ResumeState {
    pub execution_state: ExecutionState,
    pub work_items: WorkItemState,
    pub agents: AgentState,
    pub variables: VariableState,
    pub resources: ResourceState,
    pub resume_strategy: ResumeStrategy,
}

#[derive(Debug)]
pub enum ResumeStrategy {
    ContinueFromCheckpoint,
    RestartCurrentPhase,
    RestartFromMapPhase,
    ValidateAndContinue,
}

impl CheckpointManager {
    async fn build_resume_state(&self, checkpoint: MapReduceCheckpoint) -> Result<ResumeState, CheckpointError> {
        let strategy = self.determine_resume_strategy(&checkpoint)?;

        // Validate that resume is possible
        self.validate_resume_preconditions(&checkpoint, &strategy)?;

        // Prepare work item state for resume
        let work_items = self.prepare_work_item_state(&checkpoint, &strategy)?;

        // Prepare agent state
        let agents = self.prepare_agent_state(&checkpoint, &strategy)?;

        // Restore resource allocations
        let resources = self.prepare_resource_state(&checkpoint, &strategy)?;

        Ok(ResumeState {
            execution_state: checkpoint.execution_state,
            work_items,
            agents,
            variables: checkpoint.variable_state,
            resources,
            resume_strategy: strategy,
        })
    }

    fn determine_resume_strategy(&self, checkpoint: &MapReduceCheckpoint) -> Result<ResumeStrategy, CheckpointError> {
        match checkpoint.metadata.phase {
            PhaseType::Setup => Ok(ResumeStrategy::RestartCurrentPhase),
            PhaseType::Map => {
                if checkpoint.work_item_state.in_progress_items.is_empty() {
                    Ok(ResumeStrategy::ContinueFromCheckpoint)
                } else {
                    Ok(ResumeStrategy::ValidateAndContinue)
                }
            }
            PhaseType::Reduce => Ok(ResumeStrategy::ContinueFromCheckpoint),
        }
    }

    fn prepare_work_item_state(
        &self,
        checkpoint: &MapReduceCheckpoint,
        strategy: &ResumeStrategy,
    ) -> Result<WorkItemState, CheckpointError> {
        let mut work_items = checkpoint.work_item_state.clone();

        match strategy {
            ResumeStrategy::ContinueFromCheckpoint => {
                // Keep state as-is
                Ok(work_items)
            }
            ResumeStrategy::ValidateAndContinue => {
                // Move in-progress items back to pending for re-processing
                for (_, progress) in work_items.in_progress_items.drain() {
                    work_items.pending_items.push(progress.work_item);
                }
                Ok(work_items)
            }
            ResumeStrategy::RestartCurrentPhase => {
                // Reset all progress for current phase
                work_items.pending_items.extend(
                    work_items.in_progress_items.drain().map(|(_, p)| p.work_item)
                );
                work_items.completed_items.clear();
                Ok(work_items)
            }
            ResumeStrategy::RestartFromMapPhase => {
                // Reset everything from map phase
                work_items.pending_items = checkpoint.work_item_state.pending_items.clone();
                work_items.pending_items.extend(
                    checkpoint.work_item_state.completed_items.iter().map(|c| c.work_item.clone())
                );
                work_items.in_progress_items.clear();
                work_items.completed_items.clear();
                Ok(work_items)
            }
        }
    }
}
```

#### 4. Checkpoint Storage

Implement efficient checkpoint storage:

```rust
#[async_trait]
pub trait CheckpointStorage: Send + Sync {
    async fn save_checkpoint(&self, checkpoint: &MapReduceCheckpoint) -> Result<CheckpointId, CheckpointError>;
    async fn load_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<MapReduceCheckpoint, CheckpointError>;
    async fn list_checkpoints(&self, job_id: &str) -> Result<Vec<CheckpointInfo>, CheckpointError>;
    async fn delete_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<(), CheckpointError>;
    async fn checkpoint_exists(&self, checkpoint_id: &CheckpointId) -> Result<bool, CheckpointError>;
}

pub struct FileCheckpointStorage {
    base_path: PathBuf,
    compression: Option<CompressionConfig>,
}

impl FileCheckpointStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            compression: Some(CompressionConfig::default()),
        }
    }

    fn checkpoint_path(&self, checkpoint_id: &CheckpointId) -> PathBuf {
        self.base_path.join(format!("{}.checkpoint", checkpoint_id))
    }

    fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>, CheckpointError> {
        if let Some(config) = &self.compression {
            config.compress(data)
        } else {
            Ok(data.to_vec())
        }
    }

    fn decompress_data(&self, data: &[u8]) -> Result<Vec<u8>, CheckpointError> {
        if let Some(config) = &self.compression {
            config.decompress(data)
        } else {
            Ok(data.to_vec())
        }
    }
}

#[async_trait]
impl CheckpointStorage for FileCheckpointStorage {
    async fn save_checkpoint(&self, checkpoint: &MapReduceCheckpoint) -> Result<CheckpointId, CheckpointError> {
        let checkpoint_id = CheckpointId::new();
        let path = self.checkpoint_path(&checkpoint_id);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Serialize checkpoint
        let data = serde_json::to_vec(checkpoint)?;

        // Compress if enabled
        let compressed_data = self.compress_data(&data)?;

        // Write atomically using temporary file
        let temp_path = path.with_extension("checkpoint.tmp");
        fs::write(&temp_path, &compressed_data).await?;
        fs::rename(&temp_path, &path).await?;

        Ok(checkpoint_id)
    }

    async fn load_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<MapReduceCheckpoint, CheckpointError> {
        let path = self.checkpoint_path(checkpoint_id);

        if !path.exists() {
            return Err(CheckpointError::CheckpointNotFound(checkpoint_id.clone()));
        }

        // Read compressed data
        let compressed_data = fs::read(&path).await?;

        // Decompress
        let data = self.decompress_data(&compressed_data)?;

        // Deserialize
        let checkpoint: MapReduceCheckpoint = serde_json::from_slice(&data)?;

        Ok(checkpoint)
    }
}

#[derive(Debug, Clone)]
pub struct CompressionConfig {
    pub algorithm: CompressionAlgorithm,
    pub level: CompressionLevel,
}

#[derive(Debug, Clone)]
pub enum CompressionAlgorithm {
    Gzip,
    Zstd,
    Lz4,
}

#[derive(Debug, Clone)]
pub enum CompressionLevel {
    Fast,
    Balanced,
    Best,
}

impl CompressionConfig {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CheckpointError> {
        match self.algorithm {
            CompressionAlgorithm::Gzip => {
                use flate2::{write::GzEncoder, Compression};
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(data)?;
                Ok(encoder.finish()?)
            }
            CompressionAlgorithm::Zstd => {
                Ok(zstd::encode_all(data, 3)?)
            }
            CompressionAlgorithm::Lz4 => {
                Ok(lz4_flex::compress_prepend_size(data))
            }
        }
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CheckpointError> {
        match self.algorithm {
            CompressionAlgorithm::Gzip => {
                use flate2::read::GzDecoder;
                let mut decoder = GzDecoder::new(data);
                let mut result = Vec::new();
                decoder.read_to_end(&mut result)?;
                Ok(result)
            }
            CompressionAlgorithm::Zstd => {
                Ok(zstd::decode_all(data)?)
            }
            CompressionAlgorithm::Lz4 => {
                Ok(lz4_flex::decompress_size_prepended(data)?)
            }
        }
    }
}
```

#### 5. CLI Integration

Extend CLI with checkpoint management commands:

```rust
#[derive(Parser)]
pub struct CheckpointCommand {
    #[clap(subcommand)]
    pub action: CheckpointAction,
}

#[derive(Subcommand)]
pub enum CheckpointAction {
    /// List checkpoints for a job
    List {
        /// Job ID to list checkpoints for
        job_id: String,
        /// Show detailed information
        #[clap(long)]
        detailed: bool,
    },
    /// Show checkpoint details
    Show {
        /// Checkpoint ID
        checkpoint_id: String,
        /// Output format
        #[clap(long, default_value = "human")]
        format: OutputFormat,
    },
    /// Delete a checkpoint
    Delete {
        /// Checkpoint ID
        checkpoint_id: String,
        /// Force deletion without confirmation
        #[clap(long)]
        force: bool,
    },
    /// Validate checkpoint integrity
    Validate {
        /// Checkpoint ID
        checkpoint_id: String,
        /// Attempt to repair if corrupt
        #[clap(long)]
        repair: bool,
    },
    /// Clean up old checkpoints
    Cleanup {
        /// Job ID to clean
        job_id: Option<String>,
        /// Maximum age to keep
        #[clap(long)]
        max_age: Option<String>,
        /// Maximum number to keep
        #[clap(long)]
        max_count: Option<usize>,
        /// Dry run - show what would be deleted
        #[clap(long)]
        dry_run: bool,
    },
}

pub async fn handle_checkpoint_command(cmd: CheckpointCommand) -> anyhow::Result<()> {
    match cmd.action {
        CheckpointAction::List { job_id, detailed } => {
            handle_list_checkpoints(job_id, detailed).await
        }
        CheckpointAction::Show { checkpoint_id, format } => {
            handle_show_checkpoint(checkpoint_id, format).await
        }
        CheckpointAction::Delete { checkpoint_id, force } => {
            handle_delete_checkpoint(checkpoint_id, force).await
        }
        CheckpointAction::Validate { checkpoint_id, repair } => {
            handle_validate_checkpoint(checkpoint_id, repair).await
        }
        CheckpointAction::Cleanup { job_id, max_age, max_count, dry_run } => {
            handle_cleanup_checkpoints(job_id, max_age, max_count, dry_run).await
        }
    }
}
```

### Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(CheckpointId),

    #[error("Checkpoint validation failed: {0}")]
    ValidationFailed(String),

    #[error("Checkpoint corruption detected: {0}")]
    CorruptionDetected(String),

    #[error("Resume preconditions not met: {0}")]
    ResumePreconditionsFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("No checkpoint found for job")]
    NoCheckpointFound,
}

impl CheckpointError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CheckpointError::ValidationFailed(_) | CheckpointError::CorruptionDetected(_)
        )
    }

    pub fn suggests_repair(&self) -> bool {
        matches!(self, CheckpointError::CorruptionDetected(_))
    }
}
```

## Testing Strategy

### Unit Tests
- Test checkpoint serialization/deserialization with various data structures
- Test compression algorithms and integrity validation
- Test resume state building logic
- Test retention policy implementation
- Test checkpoint storage operations

### Integration Tests
- Test end-to-end checkpoint creation during MapReduce execution
- Test resume functionality with different interruption scenarios
- Test CLI checkpoint management commands
- Test checkpoint cleanup and retention policies
- Test concurrent checkpoint operations

### Performance Tests
- Benchmark checkpoint creation overhead vs. execution time
- Test checkpoint compression ratios and performance
- Test resume time vs. checkpoint size
- Test concurrent checkpoint operations under load

### Reliability Tests
- Test checkpoint integrity validation and repair
- Test resume behavior with corrupted checkpoints
- Test checkpoint storage under disk pressure
- Test recovery from incomplete checkpoint operations

## Migration Strategy

### Phase 1: Enhanced Checkpoint Structure
1. Implement comprehensive checkpoint data structure
2. Add checkpoint validation and integrity checking
3. Implement checkpoint compression

### Phase 2: Resume Functionality
1. Implement resume state builder
2. Add resume strategy logic
3. Integrate with MapReduce coordinator

### Phase 3: Storage and Management
1. Implement efficient checkpoint storage
2. Add retention policies and cleanup
3. Implement CLI management commands

### Phase 4: Optimization and Monitoring
1. Optimize checkpoint performance
2. Add checkpoint monitoring and metrics
3. Implement advanced repair capabilities

## Documentation Requirements

- Update MapReduce documentation with checkpoint/resume workflow
- Document CLI checkpoint management commands
- Create troubleshooting guide for checkpoint issues
- Document best practices for checkpoint configuration
- Add examples demonstrating resume scenarios

## Risk Assessment

### High Risk
- **Resume Consistency**: Resume might not preserve exact execution state
- **Checkpoint Overhead**: Frequent checkpointing might significantly impact performance
- **Data Corruption**: Checkpoint corruption could make jobs unrecoverable

### Medium Risk
- **Storage Growth**: Checkpoints might consume significant disk space
- **Resume Complexity**: Complex resume scenarios might be error-prone
- **Version Compatibility**: Checkpoint format changes might break resume

### Mitigation Strategies
- Implement comprehensive validation at checkpoint creation and resume
- Provide configurable checkpoint intervals to balance overhead vs. recovery
- Include checkpoint versioning and migration tools
- Implement robust error handling with clear recovery guidance
- Add monitoring and alerting for checkpoint health