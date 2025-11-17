//! Dead Letter Queue for MapReduce failed items
//!
//! Captures persistently failing items for later analysis and potential
//! manual intervention, while allowing the job to continue processing other items.

use anyhow::{Context, Result};
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::events::EventLogger;

/// Dead Letter Queue for handling failed items
pub struct DeadLetterQueue {
    pub job_id: String,
    items: Arc<RwLock<HashMap<String, DeadLetteredItem>>>,
    storage: Arc<DLQStorage>,
    max_items: usize,
    #[allow(dead_code)]
    retention_days: u32,
    event_logger: Option<Arc<EventLogger>>,
}

/// An item that has been moved to the Dead Letter Queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetteredItem {
    pub item_id: String,
    pub item_data: Value,
    pub first_attempt: DateTime<Utc>,
    pub last_attempt: DateTime<Utc>,
    pub failure_count: u32,
    pub failure_history: Vec<FailureDetail>,
    pub error_signature: String,
    pub worktree_artifacts: Option<WorktreeArtifacts>,
    pub reprocess_eligible: bool,
    pub manual_review_required: bool,
}

/// Details about a single failure attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureDetail {
    pub attempt_number: u32,
    pub timestamp: DateTime<Utc>,
    pub error_type: ErrorType,
    pub error_message: String,
    pub stack_trace: Option<String>,
    pub agent_id: String,
    pub step_failed: String,
    pub duration_ms: u64,
    /// Path to Claude JSON log file for debugging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_log_location: Option<String>,
}

/// Types of errors that can occur
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ErrorType {
    Timeout,
    CommandFailed { exit_code: i32 },
    /// Commit validation failed - required commit was not created
    CommitValidationFailed,
    WorktreeError,
    MergeConflict,
    ValidationFailed,
    ResourceExhausted,
    Unknown,
}

/// Artifacts from the worktree where the failure occurred
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeArtifacts {
    pub worktree_path: PathBuf,
    pub branch_name: String,
    pub uncommitted_changes: Option<String>,
    pub error_logs: Option<String>,
}

/// Storage handler for DLQ persistence
pub struct DLQStorage {
    base_path: PathBuf,
    #[allow(dead_code)]
    compression: bool,
}

/// Analysis of failure patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureAnalysis {
    pub total_items: usize,
    pub pattern_groups: Vec<PatternGroup>,
    pub error_distribution: HashMap<ErrorType, usize>,
    pub temporal_distribution: Vec<(DateTime<Utc>, usize)>,
}

/// A group of failures with similar patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternGroup {
    pub signature: String,
    pub count: usize,
    pub first_occurrence: Option<DateTime<Utc>>,
    pub last_occurrence: Option<DateTime<Utc>>,
    pub sample_items: Vec<DeadLetteredItem>,
}

/// Request to reprocess items from DLQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessRequest {
    pub item_ids: Vec<String>,
    pub max_retries: u32,
    pub delay_ms: u64,
    pub force: bool,
}

/// Filter criteria for listing DLQ items
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DLQFilter {
    pub error_type: Option<ErrorType>,
    pub reprocess_eligible: Option<bool>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub error_signature: Option<String>,
}

/// DLQ event types for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DLQEvent {
    ItemAdded { item: Box<DeadLetteredItem> },
    ItemRemoved { item_id: String },
    ItemsReprocessed { count: usize },
    ItemsEvicted { count: usize },
    AnalysisGenerated { patterns: usize },
}

impl DeadLetterQueue {
    /// Create a new Dead Letter Queue
    pub async fn new(
        job_id: String,
        base_path: PathBuf,
        max_items: usize,
        retention_days: u32,
        event_logger: Option<Arc<EventLogger>>,
    ) -> Result<Self> {
        let storage = Arc::new(DLQStorage::new(base_path)?);

        // Load existing items if any
        let items = storage.load_all(&job_id).await?;

        Ok(Self {
            job_id,
            items: Arc::new(RwLock::new(items)),
            storage,
            max_items,
            retention_days,
            event_logger,
        })
    }

    /// Load an existing Dead Letter Queue (for reading stats)
    pub async fn load(job_id: String, base_path: PathBuf) -> Result<Self> {
        // Use default values for max_items and retention when just loading for stats
        Self::new(job_id, base_path, 1000, 30, None).await
    }

    /// Add a failed item to the DLQ
    pub async fn add(&self, item: DeadLetteredItem) -> Result<()> {
        // Check capacity and evict if necessary
        {
            let items = self.items.read().await;
            if items.len() >= self.max_items {
                drop(items); // Release read lock before evicting
                self.evict_oldest().await?;
            }
        }

        // Store to disk first
        self.storage.persist(&self.job_id, &item).await?;

        // Update in-memory cache
        self.items
            .write()
            .await
            .insert(item.item_id.clone(), item.clone());

        // Log event
        if let Some(logger) = &self.event_logger {
            logger
                .log_dlq_event_with_job(
                    self.job_id.clone(),
                    DLQEvent::ItemAdded {
                        item: Box::new(item),
                    },
                )
                .await?;
        }

        Ok(())
    }

    /// Reprocess items from the DLQ
    pub async fn reprocess(&self, item_ids: Vec<String>) -> Result<Vec<Value>> {
        let mut reprocessable = Vec::new();

        for item_id in item_ids {
            let item = {
                let items = self.items.read().await;
                items.get(&item_id).cloned()
            };

            if let Some(item) = item {
                if item.reprocess_eligible {
                    reprocessable.push(item.item_data.clone());
                    self.remove(&item_id).await?;
                }
            }
        }

        // Log event
        if let Some(logger) = &self.event_logger {
            logger
                .log_dlq_event_with_job(
                    self.job_id.clone(),
                    DLQEvent::ItemsReprocessed {
                        count: reprocessable.len(),
                    },
                )
                .await?;
        }

        Ok(reprocessable)
    }

    /// Remove an item from the DLQ
    pub async fn remove(&self, item_id: &str) -> Result<()> {
        // Remove from storage
        self.storage.remove(&self.job_id, item_id).await?;

        // Remove from memory
        self.items.write().await.remove(item_id);

        // Log event
        if let Some(logger) = &self.event_logger {
            logger
                .log_dlq_event_with_job(
                    self.job_id.clone(),
                    DLQEvent::ItemRemoved {
                        item_id: item_id.to_string(),
                    },
                )
                .await?;
        }

        Ok(())
    }

    /// List items in the DLQ with optional filtering
    pub async fn list_items(&self, filter: DLQFilter) -> Result<Vec<DeadLetteredItem>> {
        let items = self.items.read().await;

        let mut result: Vec<DeadLetteredItem> = items
            .values()
            .filter(|item| {
                // Apply filters
                if let Some(ref error_type) = filter.error_type {
                    if !item
                        .failure_history
                        .iter()
                        .any(|f| &f.error_type == error_type)
                    {
                        return false;
                    }
                }

                if let Some(eligible) = filter.reprocess_eligible {
                    if item.reprocess_eligible != eligible {
                        return false;
                    }
                }

                if let Some(after) = filter.after {
                    if item.last_attempt < after {
                        return false;
                    }
                }

                if let Some(before) = filter.before {
                    if item.last_attempt > before {
                        return false;
                    }
                }

                if let Some(ref sig) = filter.error_signature {
                    if !item.error_signature.contains(sig) {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        // Sort by last attempt time (most recent first)
        result.sort_by(|a, b| b.last_attempt.cmp(&a.last_attempt));

        Ok(result)
    }

    /// Get a specific item from the DLQ
    pub async fn get_item(&self, item_id: &str) -> Result<Option<DeadLetteredItem>> {
        let items = self.items.read().await;
        Ok(items.get(item_id).cloned())
    }

    /// Analyze failure patterns in the DLQ
    pub async fn analyze_patterns(&self) -> Result<FailureAnalysis> {
        let items = self.items.read().await;

        // Group by error signature
        let mut patterns: HashMap<String, Vec<DeadLetteredItem>> = HashMap::new();
        let mut error_distribution: HashMap<ErrorType, usize> = HashMap::new();

        for item in items.values() {
            patterns
                .entry(item.error_signature.clone())
                .or_default()
                .push(item.clone());

            // Count error types
            for failure in &item.failure_history {
                *error_distribution
                    .entry(failure.error_type.clone())
                    .or_insert(0) += 1;
            }
        }

        // Build pattern groups
        let pattern_groups: Vec<PatternGroup> = patterns
            .into_iter()
            .map(|(sig, items)| PatternGroup {
                signature: sig,
                count: items.len(),
                first_occurrence: items.iter().map(|i| i.first_attempt).min(),
                last_occurrence: items.iter().map(|i| i.last_attempt).max(),
                sample_items: items.into_iter().take(3).collect(),
            })
            .collect();

        // Build temporal distribution (hourly buckets)
        let mut temporal_buckets: HashMap<DateTime<Utc>, usize> = HashMap::new();
        for item in items.values() {
            let hour = item
                .last_attempt
                .date_naive()
                .and_hms_opt(item.last_attempt.hour(), 0, 0)
                .and_then(|dt| dt.and_local_timezone(Utc).single())
                .unwrap_or(item.last_attempt);
            *temporal_buckets.entry(hour).or_insert(0) += 1;
        }

        let mut temporal_distribution: Vec<(DateTime<Utc>, usize)> =
            temporal_buckets.into_iter().collect();
        temporal_distribution.sort_by_key(|&(dt, _)| dt);

        // Log analysis event
        if let Some(logger) = &self.event_logger {
            logger
                .log_dlq_event_with_job(
                    self.job_id.clone(),
                    DLQEvent::AnalysisGenerated {
                        patterns: pattern_groups.len(),
                    },
                )
                .await?;
        }

        Ok(FailureAnalysis {
            total_items: items.len(),
            pattern_groups,
            error_distribution,
            temporal_distribution,
        })
    }

    /// Export DLQ items to a file
    pub async fn export_items(&self, path: &Path) -> Result<()> {
        let items = self.items.read().await;
        let items_vec: Vec<&DeadLetteredItem> = items.values().collect();

        let json = serde_json::to_string_pretty(&items_vec)?;
        fs::write(path, json).await?;

        info!("Exported {} DLQ items to {:?}", items.len(), path);
        Ok(())
    }

    /// Purge items older than specified date
    pub async fn purge_old_items(&self, older_than: DateTime<Utc>) -> Result<usize> {
        let items_to_purge: Vec<String> = {
            let items = self.items.read().await;
            items
                .iter()
                .filter(|(_, item)| item.last_attempt < older_than)
                .map(|(id, _)| id.clone())
                .collect()
        };

        let count = items_to_purge.len();
        for item_id in items_to_purge {
            self.remove(&item_id).await?;
        }

        info!("Purged {} items older than {}", count, older_than);
        Ok(count)
    }

    /// Create an error signature from error details
    pub fn create_error_signature(error_type: &ErrorType, error_message: &str) -> String {
        // Create a simplified signature by removing variable parts
        let simplified_message = error_message
            .split_whitespace()
            .filter(|word| !word.contains('/') && !word.chars().all(|c| c.is_numeric()))
            .take(10)
            .collect::<Vec<_>>()
            .join(" ");

        format!("{:?}::{}", error_type, simplified_message)
    }

    /// Check if an item should be moved to DLQ
    pub fn should_move_to_dlq(failure_count: u32, max_retries: u32) -> bool {
        failure_count > max_retries
    }

    /// Get DLQ statistics
    pub async fn get_stats(&self) -> Result<DLQStats> {
        let items = self.items.read().await;

        let eligible_for_reprocess = items
            .values()
            .filter(|item| item.reprocess_eligible)
            .count();

        let requiring_manual_review = items
            .values()
            .filter(|item| item.manual_review_required)
            .count();

        // Categorize errors
        let mut error_categories = HashMap::new();
        for item in items.values() {
            // Use error signature as the category
            *error_categories
                .entry(item.error_signature.clone())
                .or_insert(0) += 1;
        }

        Ok(DLQStats {
            total_items: items.len(),
            eligible_for_reprocess,
            requiring_manual_review,
            oldest_item: items.values().map(|i| i.first_attempt).min(),
            newest_item: items.values().map(|i| i.last_attempt).max(),
            error_categories,
        })
    }

    /// Evict oldest items when capacity is reached
    async fn evict_oldest(&self) -> Result<()> {
        let items_to_evict: Vec<String> = {
            let items = self.items.read().await;
            let mut sorted_items: Vec<(&String, &DeadLetteredItem)> = items.iter().collect();
            sorted_items.sort_by_key(|(_, item)| item.last_attempt);

            // Evict 10% of max capacity
            let evict_count = (self.max_items / 10).max(1);
            sorted_items
                .into_iter()
                .take(evict_count)
                .map(|(id, _)| id.clone())
                .collect()
        };

        let count = items_to_evict.len();
        for item_id in items_to_evict {
            self.remove(&item_id).await?;
        }

        warn!(
            "Evicted {} oldest items from DLQ due to capacity limit",
            count
        );

        // Log event
        if let Some(logger) = &self.event_logger {
            logger
                .log_dlq_event_with_job(self.job_id.clone(), DLQEvent::ItemsEvicted { count })
                .await?;
        }

        Ok(())
    }
}

/// Statistics about the DLQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DLQStats {
    pub total_items: usize,
    pub eligible_for_reprocess: usize,
    pub requiring_manual_review: usize,
    pub oldest_item: Option<DateTime<Utc>>,
    pub newest_item: Option<DateTime<Utc>>,
    pub error_categories: HashMap<String, usize>,
}

impl DLQStorage {
    /// Create new DLQ storage
    pub fn new(base_path: PathBuf) -> Result<Self> {
        Ok(Self {
            base_path,
            compression: false, // Can be enabled later if needed
        })
    }

    /// Get the path for a job's DLQ directory
    fn job_dir(&self, job_id: &str) -> PathBuf {
        self.base_path.join("mapreduce").join("dlq").join(job_id)
    }

    /// Get the path for a specific item
    fn item_path(&self, job_id: &str, item_id: &str) -> PathBuf {
        self.job_dir(job_id)
            .join("items")
            .join(format!("{}.json", item_id))
    }

    /// Persist an item to storage
    pub async fn persist(&self, job_id: &str, item: &DeadLetteredItem) -> Result<()> {
        let item_path = self.item_path(job_id, &item.item_id);

        // Ensure directory exists
        if let Some(parent) = item_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Write item
        let json = serde_json::to_string_pretty(item)?;
        fs::write(&item_path, json)
            .await
            .with_context(|| format!("Failed to persist DLQ item to {:?}", item_path))?;

        // Update index
        self.update_index(job_id).await?;

        debug!("Persisted DLQ item {} to {:?}", item.item_id, item_path);
        Ok(())
    }

    /// Remove an item from storage
    pub async fn remove(&self, job_id: &str, item_id: &str) -> Result<()> {
        let item_path = self.item_path(job_id, item_id);

        if item_path.exists() {
            fs::remove_file(&item_path).await?;
            self.update_index(job_id).await?;
            debug!("Removed DLQ item {} from storage", item_id);
        }

        Ok(())
    }

    /// Load all items for a job
    pub async fn load_all(&self, job_id: &str) -> Result<HashMap<String, DeadLetteredItem>> {
        let items_dir = self.job_dir(job_id).join("items");
        let mut items = HashMap::new();

        if !items_dir.exists() {
            return Ok(items);
        }

        let mut entries = fs::read_dir(&items_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match fs::read_to_string(&path).await {
                    Ok(content) => match serde_json::from_str::<DeadLetteredItem>(&content) {
                        Ok(item) => {
                            items.insert(item.item_id.clone(), item);
                        }
                        Err(e) => {
                            error!("Failed to parse DLQ item from {:?}: {}", path, e);
                        }
                    },
                    Err(e) => {
                        error!("Failed to read DLQ item from {:?}: {}", path, e);
                    }
                }
            }
        }

        info!("Loaded {} DLQ items for job {}", items.len(), job_id);
        Ok(items)
    }

    /// Update the DLQ index file
    async fn update_index(&self, job_id: &str) -> Result<()> {
        let index_path = self.job_dir(job_id).join("index.json");
        let items_dir = self.job_dir(job_id).join("items");

        let mut item_ids = Vec::new();
        if items_dir.exists() {
            let mut entries = fs::read_dir(&items_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".json") {
                        item_ids.push(name.trim_end_matches(".json").to_string());
                    }
                }
            }
        }

        let index = serde_json::json!({
            "job_id": job_id,
            "item_count": item_ids.len(),
            "item_ids": item_ids,
            "updated_at": Utc::now(),
        });

        if let Some(parent) = index_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(index_path, serde_json::to_string_pretty(&index)?).await?;
        Ok(())
    }
}

/// Manager trait for DLQ operations
#[async_trait::async_trait]
pub trait DLQManager: Send + Sync {
    /// Add a failed item to the DLQ
    async fn add_failed_item(
        &self,
        item_id: String,
        item_data: Value,
        failure: FailureDetail,
    ) -> Result<()>;

    /// List items in the DLQ
    async fn list_items(&self, filter: DLQFilter) -> Result<Vec<DeadLetteredItem>>;

    /// Get a specific item
    async fn get_item(&self, item_id: &str) -> Result<Option<DeadLetteredItem>>;

    /// Reprocess items
    async fn reprocess_items(&self, request: ReprocessRequest) -> Result<usize>;

    /// Purge old items
    async fn purge_old_items(&self, older_than: DateTime<Utc>) -> Result<usize>;

    /// Export items to file
    async fn export_items(&self, path: &Path) -> Result<()>;

    /// Get DLQ statistics
    async fn get_stats(&self) -> Result<DLQStats>;
}
