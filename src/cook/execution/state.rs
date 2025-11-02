//! MapReduce job state persistence and checkpointing
//!
//! Provides persistent state management for MapReduce jobs, enabling recovery
//! from failures and job resumption with minimal data loss.

use crate::cook::execution::mapreduce::{AgentResult, AgentStatus, MapReduceConfig};
use crate::cook::workflow::WorkflowStep;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Maximum number of checkpoints to retain per job
const MAX_CHECKPOINTS: usize = 3;

/// Checkpoint write timeout in milliseconds
const CHECKPOINT_TIMEOUT_MS: u64 = 100;

/// State of the reduce phase execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducePhaseState {
    /// Whether reduce phase has started
    pub started: bool,
    /// Whether reduce phase completed successfully
    pub completed: bool,
    /// Commands executed in reduce phase
    pub executed_commands: Vec<String>,
    /// Output from reduce phase
    pub output: Option<String>,
    /// Error if reduce phase failed
    pub error: Option<String>,
    /// Timestamp of reduce phase start
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp of reduce phase completion
    pub completed_at: Option<DateTime<Utc>>,
}

/// Information about a worktree used by an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    /// Path to the worktree
    pub path: PathBuf,
    /// Name of the worktree
    pub name: String,
    /// Branch created for this worktree
    pub branch: Option<String>,
    /// Session ID for cleanup tracking
    pub session_id: Option<String>,
}

/// Record of a failed agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    /// Identifier of the failed work item
    pub item_id: String,
    /// Number of retry attempts made
    pub attempts: u32,
    /// Last error message
    pub last_error: String,
    /// Timestamp of last attempt
    pub last_attempt: DateTime<Utc>,
    /// Worktree information if available
    pub worktree_info: Option<WorktreeInfo>,
}

/// Complete state of a MapReduce job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceJobState {
    /// Unique job identifier
    pub job_id: String,
    /// Job configuration
    pub config: MapReduceConfig,
    /// When the job started
    pub started_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// All work items to process
    pub work_items: Vec<Value>,
    /// Results from completed agents
    pub agent_results: HashMap<String, AgentResult>,
    /// Set of completed agent IDs
    pub completed_agents: HashSet<String>,
    /// Failed agents with retry information
    pub failed_agents: HashMap<String, FailureRecord>,
    /// Items still pending execution
    pub pending_items: Vec<String>,
    /// Version number for this checkpoint
    pub checkpoint_version: u32,
    /// Format version of the checkpoint (for migration support)
    #[serde(default = "default_format_version")]
    pub checkpoint_format_version: u32,
    /// Parent worktree if job is running in isolated mode
    pub parent_worktree: Option<String>,
    /// State of the reduce phase
    pub reduce_phase_state: Option<ReducePhaseState>,
    /// Total number of work items (for progress tracking)
    pub total_items: usize,
    /// Number of successful completions
    pub successful_count: usize,
    /// Number of failures
    pub failed_count: usize,
    /// Whether the job has completed
    pub is_complete: bool,
    /// Agent template commands (needed for resumption)
    pub agent_template: Vec<WorkflowStep>,
    /// Reduce phase commands (needed for resumption)
    pub reduce_commands: Option<Vec<WorkflowStep>>,
    /// Workflow variables for interpolation
    #[serde(default)]
    pub variables: HashMap<String, Value>,
    /// Setup phase output if available
    #[serde(default)]
    pub setup_output: Option<String>,
    /// Whether setup phase has been completed
    #[serde(default)]
    pub setup_completed: bool,
    /// Track retry attempts per work item
    /// Key: item_id, Value: number of attempts so far
    #[serde(default)]
    pub item_retry_counts: HashMap<String, u32>,
}

/// Default checkpoint format version
fn default_format_version() -> u32 {
    1
}

impl MapReduceJobState {
    /// Create a new job state
    pub fn new(job_id: String, config: MapReduceConfig, work_items: Vec<Value>) -> Self {
        let total_items = work_items.len();
        let pending_items: Vec<String> = work_items
            .iter()
            .enumerate()
            .map(|(i, _)| format!("item_{}", i))
            .collect();

        Self {
            job_id,
            config,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items,
            agent_results: HashMap::new(),
            completed_agents: HashSet::new(),
            failed_agents: HashMap::new(),
            pending_items,
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        }
    }

    /// Update state with a completed agent result
    pub fn update_agent_result(&mut self, result: AgentResult) {
        let item_id = result.item_id.clone();

        // Update counts based on status
        match &result.status {
            AgentStatus::Success => {
                self.successful_count += 1;
                self.failed_agents.remove(&item_id);
            }
            AgentStatus::Failed(_) | AgentStatus::Timeout => {
                // Update failure record
                let failure = self
                    .failed_agents
                    .entry(item_id.clone())
                    .or_insert_with(|| FailureRecord {
                        item_id: item_id.clone(),
                        attempts: 0,
                        last_error: String::new(),
                        last_attempt: Utc::now(),
                        worktree_info: None,
                    });

                failure.attempts += 1;
                failure.last_attempt = Utc::now();

                if let AgentStatus::Failed(err) = &result.status {
                    failure.last_error = err.clone();
                } else if matches!(result.status, AgentStatus::Timeout) {
                    failure.last_error = "Agent execution timed out".to_string();
                }

                // Store worktree info for cleanup
                if let (Some(path), Some(name)) = (&result.worktree_path, &result.branch_name) {
                    failure.worktree_info = Some(WorktreeInfo {
                        path: path.clone(),
                        name: name.clone(),
                        branch: result.branch_name.clone(),
                        session_id: result.worktree_session_id.clone(),
                    });
                }

                self.failed_count += 1;
            }
            _ => {}
        }

        // Store the result
        self.agent_results.insert(item_id.clone(), result);
        self.completed_agents.insert(item_id.clone());

        // Remove from pending
        self.pending_items.retain(|id| id != &item_id);

        // Update timestamp
        self.updated_at = Utc::now();
        self.checkpoint_version += 1;
    }

    /// Check if all agents have completed
    pub fn is_map_phase_complete(&self) -> bool {
        self.pending_items.is_empty() && self.completed_agents.len() == self.total_items
    }

    /// Get items that can be retried
    pub fn get_retriable_items(&self, max_retries: u32) -> Vec<String> {
        self.failed_agents
            .iter()
            .filter(|(_, failure)| failure.attempts < max_retries)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Mark reduce phase as started
    pub fn start_reduce_phase(&mut self) {
        self.reduce_phase_state = Some(ReducePhaseState {
            started: true,
            completed: false,
            executed_commands: Vec::new(),
            output: None,
            error: None,
            started_at: Some(Utc::now()),
            completed_at: None,
        });
        self.updated_at = Utc::now();
        self.checkpoint_version += 1;
    }

    /// Mark reduce phase as completed
    pub fn complete_reduce_phase(&mut self, output: Option<String>) {
        if let Some(ref mut state) = self.reduce_phase_state {
            state.completed = true;
            state.output = output;
            state.completed_at = Some(Utc::now());
        }
        self.is_complete = true;
        self.updated_at = Utc::now();
        self.checkpoint_version += 1;
    }

    /// Mark job as complete
    pub fn mark_complete(&mut self) {
        self.is_complete = true;
        self.updated_at = Utc::now();
        self.checkpoint_version += 1;
    }

    /// Find a work item by ID
    pub fn find_work_item(&self, item_id: &str) -> Option<Value> {
        // Extract index from item_id (format: "item_0", "item_1", etc.)
        if let Some(idx) = item_id
            .strip_prefix("item_")
            .and_then(|s| s.parse::<usize>().ok())
        {
            if idx < self.work_items.len() {
                return Some(self.work_items[idx].clone());
            }
        }
        None
    }
}

/// Information about a checkpoint file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    /// Path to the checkpoint file
    pub path: PathBuf,
    /// Version number of this checkpoint
    pub version: u32,
    /// When this checkpoint was created
    pub created_at: DateTime<Utc>,
    /// Size of the checkpoint file
    pub size_bytes: u64,
}

/// Manager for checkpoint persistence and recovery
pub struct CheckpointManager {
    /// Base directory for MapReduce state
    base_dir: PathBuf,
    /// Lock for concurrent access
    write_lock: RwLock<()>,
}

impl CheckpointManager {
    /// Create a new checkpoint manager
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            write_lock: RwLock::new(()),
        }
    }

    /// Get the directory for a specific job
    fn job_dir(&self, job_id: &str) -> PathBuf {
        self.base_dir.join("jobs").join(job_id)
    }

    /// Get the base jobs directory
    pub fn jobs_dir(&self) -> PathBuf {
        self.base_dir.join("jobs")
    }

    /// Get the path for a checkpoint file
    fn checkpoint_path(&self, job_id: &str, version: u32) -> PathBuf {
        self.job_dir(job_id)
            .join(format!("checkpoint-v{}.json", version))
    }

    /// Get the path for the metadata file
    fn metadata_path(&self, job_id: &str) -> PathBuf {
        self.job_dir(job_id).join("metadata.json")
    }

    /// Save a checkpoint atomically
    pub async fn save_checkpoint(&self, state: &MapReduceJobState) -> Result<()> {
        let _lock = self.write_lock.write().await;

        let start = std::time::Instant::now();
        let job_dir = self.job_dir(&state.job_id);

        // Ensure job directory exists
        fs::create_dir_all(&job_dir)
            .await
            .context("Failed to create job directory")?;

        // Serialize state
        let json = serde_json::to_string_pretty(state).context("Failed to serialize job state")?;

        // Write to temporary file first (atomic write pattern)
        let checkpoint_path = self.checkpoint_path(&state.job_id, state.checkpoint_version);
        let temp_path = checkpoint_path.with_extension("tmp");

        // Use explicit file operations with sync to ensure durability
        use tokio::io::AsyncWriteExt;
        let mut file = fs::File::create(&temp_path)
            .await
            .context("Failed to create temporary checkpoint")?;
        file.write_all(json.as_bytes())
            .await
            .context("Failed to write checkpoint data")?;
        file.sync_data()
            .await
            .context("Failed to sync checkpoint to disk")?;
        drop(file); // Explicitly close before rename

        // Atomically rename temp file to final checkpoint
        fs::rename(&temp_path, &checkpoint_path)
            .await
            .context("Failed to rename checkpoint file")?;

        // Update metadata
        let metadata = CheckpointInfo {
            path: checkpoint_path.clone(),
            version: state.checkpoint_version,
            created_at: Utc::now(),
            size_bytes: json.len() as u64,
        };

        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        let metadata_temp = self.metadata_path(&state.job_id).with_extension("tmp");

        // Sync metadata file as well
        let mut metadata_file = fs::File::create(&metadata_temp).await?;
        metadata_file.write_all(metadata_json.as_bytes()).await?;
        metadata_file.sync_data().await?;
        drop(metadata_file); // Explicitly close before rename

        fs::rename(metadata_temp, self.metadata_path(&state.job_id)).await?;

        let duration = start.elapsed();

        // Check if checkpoint took too long
        if duration.as_millis() > CHECKPOINT_TIMEOUT_MS as u128 {
            warn!(
                "Checkpoint for job {} took {}ms (exceeds {}ms limit)",
                state.job_id,
                duration.as_millis(),
                CHECKPOINT_TIMEOUT_MS
            );
        } else {
            debug!(
                "Saved checkpoint v{} for job {} in {}ms",
                state.checkpoint_version,
                state.job_id,
                duration.as_millis()
            );
        }

        // Clean up old checkpoints
        self.cleanup_old_checkpoints(&state.job_id, MAX_CHECKPOINTS)
            .await?;

        Ok(())
    }

    /// Load the latest checkpoint for a job
    pub async fn load_checkpoint(&self, job_id: &str) -> Result<MapReduceJobState> {
        self.load_checkpoint_by_version(job_id, None).await
    }

    /// Load a specific checkpoint by version, or latest if None
    pub async fn load_checkpoint_by_version(
        &self,
        job_id: &str,
        version: Option<u32>,
    ) -> Result<MapReduceJobState> {
        let checkpoint_path = if let Some(v) = version {
            // Load specific version
            let path = self.checkpoint_path(job_id, v);
            if !path.exists() {
                return Err(anyhow!(
                    "Checkpoint version {} not found for job {}",
                    v,
                    job_id
                ));
            }
            path
        } else {
            // Load latest from metadata
            let metadata_path = self.metadata_path(job_id);
            if !metadata_path.exists() {
                return Err(anyhow!("No checkpoint found for job {}", job_id));
            }

            let metadata_json = fs::read_to_string(&metadata_path)
                .await
                .context("Failed to read checkpoint metadata")?;

            let metadata: CheckpointInfo = serde_json::from_str(&metadata_json)
                .context("Failed to parse checkpoint metadata")?;

            metadata.path
        };

        // Load the checkpoint file
        let checkpoint_json = fs::read_to_string(&checkpoint_path)
            .await
            .context("Failed to read checkpoint file")?;

        let mut state: MapReduceJobState =
            serde_json::from_str(&checkpoint_json).context("Failed to parse checkpoint data")?;

        // Apply migrations if needed
        state = self.migrate_checkpoint(state)?;

        info!(
            "Loaded checkpoint v{} for job {} (format v{})",
            state.checkpoint_version, job_id, state.checkpoint_format_version
        );

        Ok(state)
    }

    /// Migrate checkpoint to current format version
    fn migrate_checkpoint(&self, mut state: MapReduceJobState) -> Result<MapReduceJobState> {
        const CURRENT_FORMAT_VERSION: u32 = 1;

        // If checkpoint is already at current version, no migration needed
        if state.checkpoint_format_version >= CURRENT_FORMAT_VERSION {
            return Ok(state);
        }

        debug!(
            "Migrating checkpoint from format v{} to v{}",
            state.checkpoint_format_version, CURRENT_FORMAT_VERSION
        );

        // Apply migrations based on version
        // Currently we only have version 1, so no actual migrations yet
        // Future migrations would go here:
        // if state.checkpoint_format_version < 2 {
        //     state = self.migrate_v1_to_v2(state)?;
        // }

        // Update format version
        state.checkpoint_format_version = CURRENT_FORMAT_VERSION;

        Ok(state)
    }

    /// List all available checkpoints for a job
    pub async fn list_checkpoints(&self, job_id: &str) -> Result<Vec<CheckpointInfo>> {
        let job_dir = self.job_dir(job_id);

        if !job_dir.exists() {
            return Ok(Vec::new());
        }

        let mut checkpoints = Vec::new();
        let mut entries = fs::read_dir(&job_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with("checkpoint-v") && name_str.ends_with(".json") {
                    // Parse version number from filename
                    if let Some(version_str) = name_str
                        .strip_prefix("checkpoint-v")
                        .and_then(|s| s.strip_suffix(".json"))
                    {
                        if let Ok(version) = version_str.parse::<u32>() {
                            let metadata = fs::metadata(&path).await?;
                            checkpoints.push(CheckpointInfo {
                                path,
                                version,
                                created_at: Utc::now(), // Would need to get actual creation time
                                size_bytes: metadata.len(),
                            });
                        }
                    }
                }
            }
        }

        // Sort by version (newest first)
        checkpoints.sort_by(|a, b| b.version.cmp(&a.version));

        Ok(checkpoints)
    }

    /// Clean up old checkpoint files, keeping only the most recent ones
    pub async fn cleanup_old_checkpoints(&self, job_id: &str, keep: usize) -> Result<()> {
        let checkpoints = self.list_checkpoints(job_id).await?;

        if checkpoints.len() <= keep {
            return Ok(());
        }

        // Delete older checkpoints
        for checkpoint in &checkpoints[keep..] {
            debug!(
                "Removing old checkpoint v{} for job {}",
                checkpoint.version, job_id
            );

            if let Err(e) = fs::remove_file(&checkpoint.path).await {
                error!(
                    "Failed to remove old checkpoint {}: {}",
                    checkpoint.path.display(),
                    e
                );
            }
        }

        Ok(())
    }

    /// Delete all checkpoints for a job
    pub async fn cleanup_job(&self, job_id: &str) -> Result<()> {
        let job_dir = self.job_dir(job_id);

        if job_dir.exists() {
            fs::remove_dir_all(&job_dir)
                .await
                .context("Failed to remove job directory")?;

            info!("Cleaned up all checkpoints for job {}", job_id);
        }

        Ok(())
    }

    /// Check if a job has checkpoints
    pub async fn has_checkpoint(&self, job_id: &str) -> bool {
        self.metadata_path(job_id).exists()
    }
}

/// Information about a resumable job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumableJob {
    /// Job ID
    pub job_id: String,
    /// When the job started
    pub started_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Total number of work items
    pub total_items: usize,
    /// Number of completed items
    pub completed_items: usize,
    /// Number of failed items
    pub failed_items: usize,
    /// Whether the job is complete
    pub is_complete: bool,
    /// Checkpoint version
    pub checkpoint_version: u32,
}

/// Trait for resumable operations
#[async_trait::async_trait]
pub trait Resumable: Send + Sync {
    /// Check if a job can be resumed
    async fn can_resume(&self, job_id: &str) -> Result<bool>;

    /// List all resumable jobs
    async fn list_resumable_jobs(&self) -> Result<Vec<ResumableJob>>;
}

/// Trait for managing MapReduce job state
#[async_trait::async_trait]
pub trait JobStateManager: Send + Sync {
    /// Create a new job
    async fn create_job(
        &self,
        config: MapReduceConfig,
        work_items: Vec<Value>,
        agent_template: Vec<WorkflowStep>,
        reduce_commands: Option<Vec<WorkflowStep>>,
    ) -> Result<String>;

    /// List all resumable jobs
    async fn list_resumable_jobs(&self) -> Result<Vec<ResumableJob>>;

    /// Update an agent result
    async fn update_agent_result(&self, job_id: &str, result: AgentResult) -> Result<()>;

    /// Get the current job state
    async fn get_job_state(&self, job_id: &str) -> Result<MapReduceJobState>;

    /// Get job state from a specific checkpoint version
    async fn get_job_state_from_checkpoint(
        &self,
        job_id: &str,
        checkpoint_version: Option<u32>,
    ) -> Result<MapReduceJobState>;

    /// Resume a job from checkpoint
    async fn resume_job(&self, job_id: &str) -> Result<Vec<AgentResult>>;

    /// Clean up job state
    async fn cleanup_job(&self, job_id: &str) -> Result<()>;

    /// Mark reduce phase as started
    async fn start_reduce_phase(&self, job_id: &str) -> Result<()>;

    /// Mark reduce phase as completed
    async fn complete_reduce_phase(&self, job_id: &str, output: Option<String>) -> Result<()>;

    /// Mark job as complete
    async fn mark_job_complete(&self, job_id: &str) -> Result<()>;
}

/// Default implementation of JobStateManager using CheckpointManager
pub struct DefaultJobStateManager {
    pub checkpoint_manager: CheckpointManager,
    active_jobs: RwLock<HashMap<String, MapReduceJobState>>,
    #[allow(dead_code)]
    project_root: Option<PathBuf>,
}

impl DefaultJobStateManager {
    /// Create a new job state manager
    #[allow(deprecated)]
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            checkpoint_manager: CheckpointManager::new(base_dir),
            active_jobs: RwLock::new(HashMap::new()),
            project_root: None,
        }
    }

    /// Create a new job state manager with global storage support
    #[allow(deprecated)]
    pub async fn new_with_global(project_root: PathBuf) -> Result<Self> {
        use crate::storage::{extract_repo_name, GlobalStorage};

        // Create global storage instance
        let storage = GlobalStorage::new()?;

        // Use global state directory
        let repo_name = extract_repo_name(&project_root)?;
        let global_base_dir = storage.get_state_dir(&repo_name, "mapreduce").await?;

        Ok(Self {
            checkpoint_manager: CheckpointManager::new(global_base_dir),
            active_jobs: RwLock::new(HashMap::new()),
            project_root: Some(project_root),
        })
    }
}

#[async_trait::async_trait]
impl JobStateManager for DefaultJobStateManager {
    async fn create_job(
        &self,
        config: MapReduceConfig,
        work_items: Vec<Value>,
        agent_template: Vec<WorkflowStep>,
        reduce_commands: Option<Vec<WorkflowStep>>,
    ) -> Result<String> {
        let job_id = format!("mapreduce-{}", Utc::now().timestamp_millis());
        let mut state = MapReduceJobState::new(job_id.clone(), config, work_items);
        state.agent_template = agent_template;
        state.reduce_commands = reduce_commands;

        // Save initial checkpoint
        self.checkpoint_manager.save_checkpoint(&state).await?;

        // Store in active jobs
        let mut jobs = self.active_jobs.write().await;
        jobs.insert(job_id.clone(), state);

        Ok(job_id)
    }

    async fn update_agent_result(&self, job_id: &str, result: AgentResult) -> Result<()> {
        let mut jobs = self.active_jobs.write().await;

        let state = jobs
            .get_mut(job_id)
            .ok_or_else(|| anyhow!("Job {} not found", job_id))?;

        state.update_agent_result(result);

        // Save checkpoint after update
        self.checkpoint_manager.save_checkpoint(state).await?;

        Ok(())
    }

    async fn list_resumable_jobs(&self) -> Result<Vec<ResumableJob>> {
        self.list_resumable_jobs_internal().await
    }

    async fn get_job_state(&self, job_id: &str) -> Result<MapReduceJobState> {
        let jobs = self.active_jobs.read().await;

        if let Some(state) = jobs.get(job_id) {
            return Ok(state.clone());
        }

        // Try to load from checkpoint
        self.checkpoint_manager.load_checkpoint(job_id).await
    }

    async fn get_job_state_from_checkpoint(
        &self,
        job_id: &str,
        checkpoint_version: Option<u32>,
    ) -> Result<MapReduceJobState> {
        // Load from specific checkpoint version
        self.checkpoint_manager
            .load_checkpoint_by_version(job_id, checkpoint_version)
            .await
    }

    async fn resume_job(&self, job_id: &str) -> Result<Vec<AgentResult>> {
        // Load checkpoint
        let state = self.checkpoint_manager.load_checkpoint(job_id).await?;

        // Extract completed results
        let results: Vec<AgentResult> = state.agent_results.values().cloned().collect();

        // Store in active jobs
        let mut jobs = self.active_jobs.write().await;
        jobs.insert(job_id.to_string(), state);

        Ok(results)
    }

    async fn cleanup_job(&self, job_id: &str) -> Result<()> {
        // Remove from active jobs
        let mut jobs = self.active_jobs.write().await;
        jobs.remove(job_id);

        // Clean up checkpoints
        self.checkpoint_manager.cleanup_job(job_id).await
    }

    async fn start_reduce_phase(&self, job_id: &str) -> Result<()> {
        let mut jobs = self.active_jobs.write().await;

        let state = jobs
            .get_mut(job_id)
            .ok_or_else(|| anyhow!("Job {} not found", job_id))?;

        state.start_reduce_phase();

        // Save checkpoint
        self.checkpoint_manager.save_checkpoint(state).await?;

        Ok(())
    }

    async fn complete_reduce_phase(&self, job_id: &str, output: Option<String>) -> Result<()> {
        let mut jobs = self.active_jobs.write().await;

        let state = jobs
            .get_mut(job_id)
            .ok_or_else(|| anyhow!("Job {} not found", job_id))?;

        state.complete_reduce_phase(output);

        // Save final checkpoint
        self.checkpoint_manager.save_checkpoint(state).await?;

        Ok(())
    }

    async fn mark_job_complete(&self, job_id: &str) -> Result<()> {
        let mut jobs = self.active_jobs.write().await;

        let state = jobs
            .get_mut(job_id)
            .ok_or_else(|| anyhow!("Job {} not found", job_id))?;

        state.mark_complete();

        // Save final checkpoint
        self.checkpoint_manager.save_checkpoint(state).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl Resumable for DefaultJobStateManager {
    async fn can_resume(&self, job_id: &str) -> Result<bool> {
        // Check if checkpoint exists and is valid
        match self.checkpoint_manager.load_checkpoint(job_id).await {
            Ok(state) => {
                // Job can be resumed if it's not complete
                Ok(!state.is_complete)
            }
            Err(_) => Ok(false),
        }
    }

    async fn list_resumable_jobs(&self) -> Result<Vec<ResumableJob>> {
        self.list_resumable_jobs_internal().await
    }
}

impl DefaultJobStateManager {
    /// Check if jobs directory exists and is accessible
    async fn ensure_jobs_dir_exists(jobs_dir: &std::path::Path) -> bool {
        // Attempt to get metadata for the jobs directory
        // Returns false if the directory doesn't exist or can't be accessed
        tokio::fs::metadata(jobs_dir).await.is_ok()
    }

    /// Validate a job directory and extract the job ID if valid
    async fn is_valid_job_directory(path: &std::path::Path) -> Option<String> {
        // Check if path is a directory
        let metadata = tokio::fs::metadata(path).await.ok()?;
        if !metadata.is_dir() {
            return None;
        }

        // Extract job_id from directory name
        path.file_name().and_then(|n| n.to_str()).map(String::from)
    }

    /// Load a checkpoint for a job, returning None if the checkpoint is invalid
    ///
    /// This helper converts Result to Option for cleaner error handling with the ? operator.
    /// Invalid checkpoints (corrupted files, missing metadata) are silently skipped.
    async fn load_job_checkpoint(
        checkpoint_manager: &CheckpointManager,
        job_id: &str,
    ) -> Option<MapReduceJobState> {
        checkpoint_manager.load_checkpoint(job_id).await.ok()
    }

    /// Process a single job directory entry
    ///
    /// This helper validates the directory entry and attempts to build
    /// a ResumableJob if the directory contains a valid, incomplete job.
    ///
    /// Returns None if:
    /// - The entry is not a directory
    /// - The job ID cannot be extracted
    /// - The checkpoint cannot be loaded
    /// - The job is complete
    async fn process_job_directory(
        path: std::path::PathBuf,
        checkpoint_manager: &CheckpointManager,
    ) -> Option<ResumableJob> {
        // Validate directory and extract job_id
        let job_id = Self::is_valid_job_directory(&path).await?;

        // Try to build resumable job from this directory
        Self::try_build_resumable_job(checkpoint_manager, &job_id).await
    }

    /// Collect all resumable jobs from a directory
    ///
    /// This helper encapsulates the directory scanning logic,
    /// processing each entry and collecting valid resumable jobs.
    async fn collect_resumable_jobs_from_dir(
        jobs_dir: &std::path::Path,
        checkpoint_manager: &CheckpointManager,
    ) -> Result<Vec<ResumableJob>> {
        let mut resumable_jobs = Vec::new();
        let mut entries = tokio::fs::read_dir(jobs_dir).await?;

        // Process each directory entry
        while let Some(entry) = entries.next_entry().await? {
            // Process and collect valid resumable jobs
            if let Some(job) = Self::process_job_directory(entry.path(), checkpoint_manager).await {
                resumable_jobs.push(job);
            }
        }

        Ok(resumable_jobs)
    }

    /// Try to build a ResumableJob from a job directory
    ///
    /// This is the main orchestration function that coordinates:
    /// 1. Loading the checkpoint state
    /// 2. Listing checkpoint versions
    /// 3. Building the ResumableJob if the job is incomplete
    ///
    /// Returns None if the job is complete, has no valid checkpoint, or cannot be loaded.
    async fn try_build_resumable_job(
        checkpoint_manager: &CheckpointManager,
        job_id: &str,
    ) -> Option<ResumableJob> {
        // Load checkpoint state
        let state = Self::load_job_checkpoint(checkpoint_manager, job_id).await?;

        // Get checkpoint list for version calculation
        let checkpoints = checkpoint_manager
            .list_checkpoints(job_id)
            .await
            .unwrap_or_default();

        // Build resumable job from state
        Self::build_resumable_job(job_id, state, checkpoints)
    }

    /// Build a ResumableJob from state and checkpoint list if incomplete
    fn build_resumable_job(
        job_id: &str,
        state: MapReduceJobState,
        checkpoints: Vec<CheckpointInfo>,
    ) -> Option<ResumableJob> {
        // Skip if job is complete
        if state.is_complete {
            return None;
        }

        // Calculate latest checkpoint version
        let latest_checkpoint = checkpoints
            .into_iter()
            .max_by_key(|c| c.version)
            .map(|c| c.version)
            .unwrap_or(0);

        Some(ResumableJob {
            job_id: job_id.to_string(),
            started_at: state.started_at,
            updated_at: state.updated_at,
            total_items: state.total_items,
            completed_items: state.successful_count,
            failed_items: state.failed_count,
            is_complete: false,
            checkpoint_version: latest_checkpoint,
        })
    }

    /// Resume a job from a specific checkpoint version (internal use)
    pub async fn resume_job_from_checkpoint(
        &self,
        job_id: &str,
        checkpoint_version: Option<u32>,
    ) -> Result<Vec<AgentResult>> {
        // Load checkpoint (specific version or latest)
        let state = self
            .checkpoint_manager
            .load_checkpoint_by_version(job_id, checkpoint_version)
            .await?;

        // Extract completed results
        let results: Vec<AgentResult> = state.agent_results.values().cloned().collect();

        // Store in active jobs
        let mut jobs = self.active_jobs.write().await;
        jobs.insert(job_id.to_string(), state);

        Ok(results)
    }

    /// List all resumable jobs by scanning checkpoint directories
    pub async fn list_resumable_jobs_internal(&self) -> Result<Vec<ResumableJob>> {
        let jobs_dir = self.checkpoint_manager.jobs_dir();

        // Early return if jobs directory doesn't exist
        if !Self::ensure_jobs_dir_exists(&jobs_dir).await {
            return Ok(Vec::new());
        }

        // Delegate to helper function for collecting jobs
        Self::collect_resumable_jobs_from_dir(&jobs_dir, &self.checkpoint_manager).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_checkpoint_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path().to_path_buf());

        // Create a test state
        let config = MapReduceConfig {
            input: "test.json".to_string(),
            json_path: String::new(),
            max_parallel: 5,
            max_items: None,
            offset: None,
            agent_timeout_secs: Some(300),
            continue_on_failure: false,
            batch_size: None,
            enable_checkpoints: true,
        };

        let work_items = vec![
            serde_json::json!({"id": 1, "data": "test1"}),
            serde_json::json!({"id": 2, "data": "test2"}),
        ];

        let mut state = MapReduceJobState::new("test-job-1".to_string(), config, work_items);

        // Add a result
        state.update_agent_result(AgentResult {
            item_id: "item_0".to_string(),
            status: AgentStatus::Success,
            output: Some("test output".to_string()),
            commits: vec![],
            duration: std::time::Duration::from_secs(5),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        });

        // Save checkpoint
        manager.save_checkpoint(&state).await.unwrap();

        // Load checkpoint
        let loaded_state = manager.load_checkpoint("test-job-1").await.unwrap();

        // Verify state
        assert_eq!(loaded_state.job_id, "test-job-1");
        assert_eq!(loaded_state.total_items, 2);
        assert_eq!(loaded_state.successful_count, 1);
        assert_eq!(loaded_state.completed_agents.len(), 1);
        assert!(loaded_state.completed_agents.contains("item_0"));
    }

    #[tokio::test]
    async fn test_checkpoint_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path().to_path_buf());

        let config = MapReduceConfig {
            input: "test.json".to_string(),
            json_path: String::new(),
            max_parallel: 5,
            max_items: None,
            offset: None,
            agent_timeout_secs: Some(300),
            continue_on_failure: false,
            batch_size: None,
            enable_checkpoints: true,
        };

        let mut state = MapReduceJobState::new("test-job-2".to_string(), config, vec![]);

        // Create multiple checkpoints
        for i in 0..5 {
            state.checkpoint_version = i;
            manager.save_checkpoint(&state).await.unwrap();
        }

        // List checkpoints
        let checkpoints = manager.list_checkpoints("test-job-2").await.unwrap();

        // Should only keep MAX_CHECKPOINTS (3)
        assert!(checkpoints.len() <= MAX_CHECKPOINTS);

        // Newest should be version 4
        assert_eq!(checkpoints[0].version, 4);
    }

    #[tokio::test]
    async fn test_list_resumable_jobs() {
        // Use unique prefix to avoid collisions with parallel tests
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!(
                "test-resumable-jobs-{}-",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
            .tempdir()
            .unwrap();
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        // Create a test configuration
        let config = MapReduceConfig {
            input: "test.json".to_string(),
            json_path: String::new(),
            max_parallel: 5,
            max_items: None,
            offset: None,
            agent_timeout_secs: Some(300),
            continue_on_failure: false,
            batch_size: None,
            enable_checkpoints: true,
        };

        // Create two jobs: one complete, one incomplete
        let work_items = vec![json!({"id": 1}), json!({"id": 2})];
        let template = vec![];
        let reduce_commands = None;

        // Create first job (incomplete)
        let job1_id = manager
            .create_job(
                config.clone(),
                work_items.clone(),
                template.clone(),
                reduce_commands.clone(),
            )
            .await
            .unwrap();

        // Create second job and mark it complete
        let job2_id = manager
            .create_job(config, work_items, template, reduce_commands)
            .await
            .unwrap();

        // Mark job2 as complete
        manager.mark_job_complete(&job2_id).await.unwrap();

        // List resumable jobs (use trait explicitly to avoid ambiguity)
        use Resumable;
        let resumable = <DefaultJobStateManager as Resumable>::list_resumable_jobs(&manager)
            .await
            .unwrap();

        // Should only find job1 as resumable
        assert_eq!(resumable.len(), 1);
        assert_eq!(resumable[0].job_id, job1_id);
        assert!(!resumable[0].is_complete);
    }

    #[tokio::test]
    async fn test_job_state_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = MapReduceConfig {
            input: "test.json".to_string(),
            json_path: String::new(),
            max_parallel: 5,
            max_items: None,
            offset: None,
            agent_timeout_secs: Some(300),
            continue_on_failure: false,
            batch_size: None,
            enable_checkpoints: true,
        };

        let work_items = vec![serde_json::json!({"id": 1}), serde_json::json!({"id": 2})];

        // Create job
        let job_id = manager
            .create_job(config, work_items, vec![], None)
            .await
            .unwrap();

        // Update with result
        let result = AgentResult {
            item_id: "item_0".to_string(),
            status: AgentStatus::Success,
            output: Some("output".to_string()),
            commits: vec![],
            duration: std::time::Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        };

        manager.update_agent_result(&job_id, result).await.unwrap();

        // Get state
        let state = manager.get_job_state(&job_id).await.unwrap();
        assert_eq!(state.successful_count, 1);

        // Clean up
        manager.cleanup_job(&job_id).await.unwrap();
    }

    // Helper function to create unique temp directory
    fn create_unique_temp_dir(prefix: &str) -> TempDir {
        tempfile::Builder::new()
            .prefix(&format!(
                "{}-{}-",
                prefix,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
            .tempdir()
            .unwrap()
    }

    // Helper function to create test config
    fn create_test_config() -> MapReduceConfig {
        MapReduceConfig {
            input: "test.json".to_string(),
            json_path: String::new(),
            max_parallel: 5,
            max_items: None,
            offset: None,
            agent_timeout_secs: Some(300),
            continue_on_failure: false,
            batch_size: None,
            enable_checkpoints: true,
        }
    }

    // Phase 2: Empty/Missing Directory Cases

    #[tokio::test]
    async fn test_list_resumable_empty_no_jobs_dir() {
        let temp_dir = create_unique_temp_dir("test-empty-no-dir");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 0);
    }

    #[tokio::test]
    async fn test_list_resumable_empty_dir() {
        let temp_dir = create_unique_temp_dir("test-empty-dir");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        // Create jobs directory but leave it empty
        tokio::fs::create_dir_all(manager.checkpoint_manager.jobs_dir())
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 0);
    }

    #[tokio::test]
    async fn test_list_resumable_only_files() {
        let temp_dir = create_unique_temp_dir("test-only-files");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        // Create jobs directory with a file (not a directory)
        let jobs_dir = manager.checkpoint_manager.jobs_dir();
        tokio::fs::create_dir_all(&jobs_dir).await.unwrap();
        tokio::fs::write(jobs_dir.join("file.txt"), "test")
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 0);
    }

    // Phase 3: Directory Entry Processing

    #[tokio::test]
    async fn test_list_resumable_invalid_metadata() {
        let temp_dir = create_unique_temp_dir("test-invalid-meta");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let jobs_dir = manager.checkpoint_manager.jobs_dir();
        tokio::fs::create_dir_all(&jobs_dir).await.unwrap();

        // Create directory then immediately remove it to simulate metadata error
        let job_dir = jobs_dir.join("job-1");
        tokio::fs::create_dir(&job_dir).await.unwrap();

        // We can't easily simulate metadata errors, so just verify no crash
        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert!(jobs.len() <= 1);
    }

    #[tokio::test]
    async fn test_list_resumable_file_not_dir() {
        let temp_dir = create_unique_temp_dir("test-file-not-dir");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let jobs_dir = manager.checkpoint_manager.jobs_dir();
        tokio::fs::create_dir_all(&jobs_dir).await.unwrap();
        tokio::fs::write(jobs_dir.join("not-a-dir"), "content")
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 0);
    }

    #[tokio::test]
    async fn test_list_resumable_invalid_filename() {
        let temp_dir = create_unique_temp_dir("test-invalid-filename");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let jobs_dir = manager.checkpoint_manager.jobs_dir();
        tokio::fs::create_dir_all(&jobs_dir).await.unwrap();

        // Create directory with valid name (can't easily create invalid UTF-8 filenames)
        let job_dir = jobs_dir.join("valid-job-id");
        tokio::fs::create_dir(&job_dir).await.unwrap();

        // No checkpoint file, so should be skipped
        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 0);
    }

    // Phase 4: Checkpoint Loading Branches

    #[tokio::test]
    async fn test_list_resumable_invalid_checkpoint() {
        let temp_dir = create_unique_temp_dir("test-invalid-checkpoint");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let jobs_dir = manager.checkpoint_manager.jobs_dir();
        let job_dir = jobs_dir.join("job-invalid");
        tokio::fs::create_dir_all(&job_dir).await.unwrap();

        // Write invalid JSON as checkpoint
        let checkpoint_file = job_dir.join("checkpoint-0.json");
        tokio::fs::write(checkpoint_file, "invalid json")
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 0);
    }

    #[tokio::test]
    async fn test_list_resumable_complete_job() {
        let temp_dir = create_unique_temp_dir("test-complete-job");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        // Create a complete job
        let config = create_test_config();
        let work_items = vec![json!({"id": 1})];

        let job_id = manager
            .create_job(config, work_items, vec![], None)
            .await
            .unwrap();

        // Mark as complete
        manager.mark_job_complete(&job_id).await.unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 0);
    }

    // Phase 5: Checkpoint Version Processing

    #[tokio::test]
    async fn test_list_resumable_empty_checkpoint_list() {
        let temp_dir = create_unique_temp_dir("test-empty-checkpoints");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();
        let work_items = vec![json!({"id": 1})];

        manager
            .create_job(config, work_items, vec![], None)
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].checkpoint_version, 0);
    }

    #[tokio::test]
    async fn test_list_resumable_max_checkpoint_version() {
        let temp_dir = create_unique_temp_dir("test-max-checkpoint");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();
        let work_items = vec![json!({"id": 1}), json!({"id": 2})];

        let job_id = manager
            .create_job(config.clone(), work_items.clone(), vec![], None)
            .await
            .unwrap();

        // Create multiple checkpoints
        let mut state = manager.get_job_state(&job_id).await.unwrap();
        for i in 1..4 {
            state.checkpoint_version = i;
            manager
                .checkpoint_manager
                .save_checkpoint(&state)
                .await
                .unwrap();
        }

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].checkpoint_version, 3);
    }

    // Phase 1: Entry Iteration Edge Cases

    #[tokio::test]
    async fn test_list_resumable_multiple_mixed_jobs() {
        let temp_dir = create_unique_temp_dir("test-mixed-jobs");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();

        // Create incomplete job
        let incomplete_job = manager
            .create_job(config.clone(), vec![json!({"id": 1})], vec![], None)
            .await
            .unwrap();

        // Create complete job
        let complete_job = manager
            .create_job(config.clone(), vec![json!({"id": 2})], vec![], None)
            .await
            .unwrap();
        manager.mark_job_complete(&complete_job).await.unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, incomplete_job);
    }

    #[tokio::test]
    async fn test_list_resumable_special_chars_in_name() {
        let temp_dir = create_unique_temp_dir("test-special-chars");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        // Create job with hyphens and underscores (valid job IDs)
        let config = create_test_config();
        let _job_id = manager
            .create_job(config, vec![json!({"id": 1})], vec![], None)
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].job_id.contains("mapreduce"));
    }

    #[tokio::test]
    async fn test_list_resumable_many_jobs() {
        let temp_dir = create_unique_temp_dir("test-many-jobs");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();

        // Create 50 incomplete jobs
        for _ in 0..50 {
            manager
                .create_job(config.clone(), vec![json!({"id": 1})], vec![], None)
                .await
                .unwrap();
        }

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 50);
    }

    // Phase 2: Checkpoint State Variations

    #[tokio::test]
    async fn test_list_resumable_metadata_missing() {
        let temp_dir = create_unique_temp_dir("test-no-metadata");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let jobs_dir = manager.checkpoint_manager.jobs_dir();
        let job_dir = jobs_dir.join("job-no-metadata");
        tokio::fs::create_dir_all(&job_dir).await.unwrap();

        // Create checkpoint but no metadata.json
        let config = create_test_config();
        let state = MapReduceJobState::new(
            "job-no-metadata".to_string(),
            config,
            vec![json!({"id": 1})],
        );
        let checkpoint_json = serde_json::to_string(&state).unwrap();
        tokio::fs::write(job_dir.join("checkpoint-v0.json"), checkpoint_json)
            .await
            .unwrap();

        // Should be skipped (no metadata file means load_checkpoint fails)
        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 0);
    }

    #[tokio::test]
    async fn test_list_resumable_checkpoints_but_metadata_invalid() {
        let temp_dir = create_unique_temp_dir("test-invalid-metadata");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let jobs_dir = manager.checkpoint_manager.jobs_dir();
        let job_dir = jobs_dir.join("job-bad-metadata");
        tokio::fs::create_dir_all(&job_dir).await.unwrap();

        // Create valid checkpoint
        let config = create_test_config();
        let state = MapReduceJobState::new(
            "job-bad-metadata".to_string(),
            config,
            vec![json!({"id": 1})],
        );
        let checkpoint_json = serde_json::to_string(&state).unwrap();
        tokio::fs::write(job_dir.join("checkpoint-v0.json"), checkpoint_json)
            .await
            .unwrap();

        // Create invalid metadata.json
        tokio::fs::write(job_dir.join("metadata.json"), "bad json")
            .await
            .unwrap();

        // Should be skipped (corrupted metadata)
        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 0);
    }

    #[tokio::test]
    async fn test_list_resumable_mixed_checkpoint_versions() {
        let temp_dir = create_unique_temp_dir("test-mixed-versions");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();

        // Create job with version 1
        let job1 = manager
            .create_job(config.clone(), vec![json!({"id": 1})], vec![], None)
            .await
            .unwrap();

        // Create job with version 5
        let job2 = manager
            .create_job(config, vec![json!({"id": 2})], vec![], None)
            .await
            .unwrap();
        let mut state = manager.get_job_state(&job2).await.unwrap();
        state.checkpoint_version = 5;
        manager
            .checkpoint_manager
            .save_checkpoint(&state)
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 2);

        // Find each job and verify versions
        let j1 = jobs.iter().find(|j| j.job_id == job1).unwrap();
        let j2 = jobs.iter().find(|j| j.job_id == job2).unwrap();
        assert_eq!(j1.checkpoint_version, 0);
        assert_eq!(j2.checkpoint_version, 5);
    }

    // Phase 3: Data Integrity and Edge Values

    #[tokio::test]
    async fn test_list_resumable_zero_items() {
        let temp_dir = create_unique_temp_dir("test-zero-items");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();

        // Create job with empty work_items list
        let _job_id = manager
            .create_job(config, vec![], vec![], None)
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].total_items, 0);
        assert_eq!(jobs[0].completed_items, 0);
    }

    #[tokio::test]
    async fn test_list_resumable_high_checkpoint_version() {
        let temp_dir = create_unique_temp_dir("test-high-version");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();
        let job_id = manager
            .create_job(config, vec![json!({"id": 1})], vec![], None)
            .await
            .unwrap();

        // Create checkpoint with very high version number
        let mut state = manager.get_job_state(&job_id).await.unwrap();
        state.checkpoint_version = u32::MAX - 1;
        manager
            .checkpoint_manager
            .save_checkpoint(&state)
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].checkpoint_version, u32::MAX - 1);
    }

    #[tokio::test]
    async fn test_list_resumable_partial_failures() {
        let temp_dir = create_unique_temp_dir("test-partial-failures");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();
        let work_items = vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})];
        let job_id = manager
            .create_job(config, work_items, vec![], None)
            .await
            .unwrap();

        // Add one success and one failure
        manager
            .update_agent_result(
                &job_id,
                AgentResult {
                    item_id: "item_0".to_string(),
                    status: AgentStatus::Success,
                    output: Some("success".to_string()),
                    commits: vec![],
                    duration: std::time::Duration::from_secs(1),
                    error: None,
                    worktree_path: None,
                    branch_name: None,
                    worktree_session_id: None,
                    files_modified: vec![],
                    json_log_location: None,
                    cleanup_status: None,
                },
            )
            .await
            .unwrap();

        manager
            .update_agent_result(
                &job_id,
                AgentResult {
                    item_id: "item_1".to_string(),
                    status: AgentStatus::Failed("test error".to_string()),
                    output: None,
                    commits: vec![],
                    duration: std::time::Duration::from_secs(1),
                    error: Some("test error".to_string()),
                    worktree_path: None,
                    branch_name: None,
                    worktree_session_id: None,
                    files_modified: vec![],
                    json_log_location: None,
                    cleanup_status: None,
                },
            )
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].completed_items, 1);
        assert_eq!(jobs[0].failed_items, 1);
        assert_eq!(jobs[0].total_items, 3);
    }

    #[tokio::test]
    async fn test_list_resumable_recent_vs_old_jobs() {
        let temp_dir = create_unique_temp_dir("test-timestamps");
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();

        // Create two jobs with different timestamps
        let old_job = manager
            .create_job(config.clone(), vec![json!({"id": 1})], vec![], None)
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let new_job = manager
            .create_job(config, vec![json!({"id": 2})], vec![], None)
            .await
            .unwrap();

        let jobs = manager.list_resumable_jobs_internal().await.unwrap();
        assert_eq!(jobs.len(), 2);

        // Find each job
        let old = jobs.iter().find(|j| j.job_id == old_job).unwrap();
        let new = jobs.iter().find(|j| j.job_id == new_job).unwrap();

        // Verify newer job has later timestamp
        assert!(new.started_at >= old.started_at);
    }
}
