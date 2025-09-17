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

        fs::write(&temp_path, &json)
            .await
            .context("Failed to write temporary checkpoint")?;

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
        fs::write(&metadata_temp, metadata_json).await?;
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
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            checkpoint_manager: CheckpointManager::new(base_dir),
            active_jobs: RwLock::new(HashMap::new()),
            project_root: None,
        }
    }

    /// Create a new job state manager with global storage support
    pub async fn new_with_global(project_root: PathBuf) -> Result<Self> {
        use crate::storage::GlobalStorage;

        // Check if we should use global storage
        if GlobalStorage::should_use_global() {
            // Create global storage instance
            let storage = GlobalStorage::new(&project_root)?;

            // Use global state directory
            let global_base_dir = storage.get_state_dir("mapreduce").await?;

            Ok(Self {
                checkpoint_manager: CheckpointManager::new(global_base_dir),
                active_jobs: RwLock::new(HashMap::new()),
                project_root: Some(project_root),
            })
        } else {
            // Fall back to local storage
            let local_dir = project_root.join(".prodigy").join("mapreduce");
            Ok(Self::new(local_dir))
        }
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

impl DefaultJobStateManager {
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: None,
            offset: None,
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
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: None,
            offset: None,
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
    async fn test_job_state_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = MapReduceConfig {
            input: "test.json".to_string(),
            json_path: String::new(),
            max_parallel: 5,
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: None,
            offset: None,
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
        };

        manager.update_agent_result(&job_id, result).await.unwrap();

        // Get state
        let state = manager.get_job_state(&job_id).await.unwrap();
        assert_eq!(state.successful_count, 1);

        // Clean up
        manager.cleanup_job(&job_id).await.unwrap();
    }
}
