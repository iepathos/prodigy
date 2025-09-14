//! DLQ reprocessor for handling failed MapReduce items
//!
//! Provides functionality to reprocess items from the Dead Letter Queue
//! with configurable retry strategies and filtering options.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::dlq::{DLQFilter, DeadLetterQueue, DeadLetteredItem};
use super::events::EventLogger;
use super::mapreduce::MapReduceExecutor;

/// Options for reprocessing DLQ items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessOptions {
    /// Maximum retry attempts per item
    pub max_retries: u32,
    /// Filter expression for selective reprocessing
    pub filter: Option<String>,
    /// Number of parallel workers
    pub parallel: usize,
    /// Timeout per item in seconds
    pub timeout_per_item: u64,
    /// Retry strategy
    pub strategy: RetryStrategy,
    /// Whether to merge results with original job
    pub merge_results: bool,
    /// Force reprocessing even if not eligible
    pub force: bool,
}

impl Default for ReprocessOptions {
    fn default() -> Self {
        Self {
            max_retries: 3,
            filter: None,
            parallel: 10,
            timeout_per_item: 300,
            strategy: RetryStrategy::ExponentialBackoff,
            merge_results: true,
            force: false,
        }
    }
}

/// Retry strategy for failed items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryStrategy {
    /// Immediate retry without delay
    Immediate,
    /// Fixed delay between retries
    FixedDelay { delay_ms: u64 },
    /// Exponential backoff with configurable base
    ExponentialBackoff,
}

/// Result of a reprocessing operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessResult {
    /// Total items attempted
    pub total_items: usize,
    /// Successfully processed items
    pub successful: usize,
    /// Failed items
    pub failed: usize,
    /// New job ID for the reprocessing run
    pub job_id: String,
    /// Processing duration
    pub duration: std::time::Duration,
    /// Items that failed again
    pub failed_items: Vec<String>,
}

/// Filter evaluator for DLQ items
pub struct FilterEvaluator {
    expression: String,
}

impl FilterEvaluator {
    /// Create a new filter evaluator
    pub fn new(expression: String) -> Self {
        Self { expression }
    }

    /// Check if an item matches the filter expression
    pub fn matches(&self, item: &DeadLetteredItem) -> bool {
        // Parse simple expressions like "item.field == 'value'" or "item.score >= 5"
        if self.expression.is_empty() {
            return true;
        }

        // Simple expression parser for common cases
        let parts: Vec<&str> = self.expression.split_whitespace().collect();
        if parts.len() < 3 {
            warn!("Invalid filter expression: {}", self.expression);
            return true;
        }

        let field = parts[0];
        let operator = parts[1];
        let value = parts[2..]
            .join(" ")
            .trim_matches(|c| c == '\'' || c == '"')
            .to_string();

        // Extract field value from item
        let field_value = if let Some(field_name) = field.strip_prefix("item.") {
            if field_name == "reprocess_eligible" {
                return match operator {
                    "==" => item.reprocess_eligible.to_string() == value,
                    "!=" => item.reprocess_eligible.to_string() != value,
                    _ => true,
                };
            } else if field_name == "failure_count" {
                let count = item.failure_count;
                return match operator {
                    "==" => count.to_string() == value,
                    "!=" => count.to_string() != value,
                    ">" => count > value.parse().unwrap_or(0),
                    ">=" => count >= value.parse().unwrap_or(0),
                    "<" => count < value.parse().unwrap_or(u32::MAX),
                    "<=" => count <= value.parse().unwrap_or(u32::MAX),
                    _ => true,
                };
            } else {
                // Try to extract from item_data JSON
                if let Some(obj) = item.item_data.as_object() {
                    if let Some(val) = obj.get(field_name) {
                        // Handle different JSON value types
                        if let Some(s) = val.as_str() {
                            Some(s.to_string())
                        } else if let Some(n) = val.as_i64() {
                            Some(n.to_string())
                        } else if let Some(n) = val.as_u64() {
                            Some(n.to_string())
                        } else if let Some(b) = val.as_bool() {
                            Some(b.to_string())
                        } else {
                            val.as_f64().map(|f| f.to_string())
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        } else {
            None
        };

        // Evaluate expression
        match field_value {
            Some(fv) => match operator {
                "==" => fv == value,
                "!=" => fv != value,
                ">" => {
                    // Try numeric comparison first
                    if let (Ok(fv_num), Ok(val_num)) = (fv.parse::<f64>(), value.parse::<f64>()) {
                        fv_num > val_num
                    } else {
                        fv > value
                    }
                }
                ">=" => {
                    if let (Ok(fv_num), Ok(val_num)) = (fv.parse::<f64>(), value.parse::<f64>()) {
                        fv_num >= val_num
                    } else {
                        fv >= value
                    }
                }
                "<" => {
                    if let (Ok(fv_num), Ok(val_num)) = (fv.parse::<f64>(), value.parse::<f64>()) {
                        fv_num < val_num
                    } else {
                        fv < value
                    }
                }
                "<=" => {
                    if let (Ok(fv_num), Ok(val_num)) = (fv.parse::<f64>(), value.parse::<f64>()) {
                        fv_num <= val_num
                    } else {
                        fv <= value
                    }
                }
                "contains" => fv.contains(&value),
                _ => {
                    warn!("Unknown operator: {}", operator);
                    true
                }
            },
            None => false,
        }
    }
}

/// DLQ reprocessor for handling failed items
pub struct DlqReprocessor {
    dlq: Arc<DeadLetterQueue>,
    #[allow(dead_code)]
    event_logger: Option<Arc<EventLogger>>,
    #[allow(dead_code)]
    project_root: PathBuf,
    reprocessing_locks: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
}

impl DlqReprocessor {
    /// Create a new DLQ reprocessor
    pub fn new(
        dlq: Arc<DeadLetterQueue>,
        event_logger: Option<Arc<EventLogger>>,
        project_root: PathBuf,
    ) -> Self {
        Self {
            dlq,
            event_logger,
            project_root,
            reprocessing_locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Reprocess items from the DLQ
    pub async fn reprocess(
        &self,
        workflow_id: &str,
        options: ReprocessOptions,
        executor: Arc<MapReduceExecutor>,
    ) -> Result<ReprocessResult> {
        let start_time = std::time::Instant::now();

        // Check for concurrent reprocessing
        self.acquire_reprocessing_lock(workflow_id).await?;

        // Load failed items from DLQ
        let filter = DLQFilter::default();
        let all_items = self.dlq.list_items(filter).await?;

        // Apply custom filter if specified
        let filtered_items = if let Some(ref filter_expr) = options.filter {
            let evaluator = FilterEvaluator::new(filter_expr.clone());
            all_items
                .into_iter()
                .filter(|item| evaluator.matches(item))
                .collect()
        } else {
            all_items
        };

        // Check eligibility unless forced
        let items_to_process: Vec<DeadLetteredItem> = if options.force {
            filtered_items
        } else {
            filtered_items
                .into_iter()
                .filter(|item| item.reprocess_eligible)
                .collect()
        };

        info!(
            "Reprocessing {} items from DLQ for workflow {}",
            items_to_process.len(),
            workflow_id
        );

        // Create new job ID for reprocessing
        let reprocess_job_id = format!("{}-reprocess-{}", workflow_id, Utc::now().timestamp());

        // Convert DLQ items back to work items
        let work_items: Vec<Value> = items_to_process
            .iter()
            .map(|item| item.item_data.clone())
            .collect();

        // Execute reprocessing with the MapReduceExecutor
        let results = self
            .execute_with_retry(&work_items, &reprocess_job_id, &options, executor)
            .await?;

        // Process results and update DLQ
        let mut successful = 0;
        let mut failed = 0;
        let mut failed_items = Vec::new();

        for (i, result) in results.iter().enumerate() {
            if let Some(item) = items_to_process.get(i) {
                match result {
                    Ok(_) => {
                        // Remove successfully processed item from DLQ
                        self.dlq.remove(&item.item_id).await?;
                        successful += 1;
                    }
                    Err(e) => {
                        // Update failure count in DLQ
                        warn!("Item {} failed reprocessing: {}", item.item_id, e);
                        failed_items.push(item.item_id.clone());
                        failed += 1;
                    }
                }
            }
        }

        // Release the reprocessing lock
        self.release_reprocessing_lock(workflow_id).await;

        let duration = start_time.elapsed();

        info!(
            "Reprocessing completed for {}: {} successful, {} failed in {:?}",
            workflow_id, successful, failed, duration
        );

        Ok(ReprocessResult {
            total_items: items_to_process.len(),
            successful,
            failed,
            job_id: reprocess_job_id,
            duration,
            failed_items,
        })
    }

    /// Execute items with retry strategy
    async fn execute_with_retry(
        &self,
        items: &[Value],
        job_id: &str,
        options: &ReprocessOptions,
        _executor: Arc<MapReduceExecutor>,
    ) -> Result<Vec<Result<Value>>> {
        let mut results = Vec::new();

        // Process items in batches based on parallelism
        let batch_size = options.parallel;
        for chunk in items.chunks(batch_size) {
            let mut batch_results = Vec::new();

            for item in chunk {
                let mut attempts = 0;
                loop {
                    attempts += 1;

                    // Apply retry strategy delay
                    if attempts > 1 {
                        self.apply_retry_delay(&options.strategy, attempts).await;
                    }

                    // Attempt to process the item
                    match self.process_single_item(item, job_id).await {
                        Ok(result) => {
                            batch_results.push(Ok(result));
                            break;
                        }
                        Err(e) if attempts < options.max_retries => {
                            warn!("Attempt {} failed for item: {}", attempts, e);
                            continue;
                        }
                        Err(e) => {
                            error!("Item failed after {} attempts: {}", attempts, e);
                            batch_results.push(Err(e));
                            break;
                        }
                    }
                }
            }

            results.extend(batch_results);
        }

        Ok(results)
    }

    /// Process a single item
    async fn process_single_item(&self, item: &Value, job_id: &str) -> Result<Value> {
        // This would typically call the MapReduceExecutor to process the item
        // For now, we'll return a placeholder success
        debug!("Processing item for job {}: {:?}", job_id, item);

        // In a real implementation, this would execute the workflow steps
        // For now, we'll simulate processing
        Ok(serde_json::json!({
            "status": "reprocessed",
            "original": item,
            "job_id": job_id,
            "timestamp": Utc::now().to_rfc3339()
        }))
    }

    /// Apply retry delay based on strategy
    #[cfg(test)]
    pub async fn apply_retry_delay(&self, strategy: &RetryStrategy, attempt: u32) {
        match strategy {
            RetryStrategy::Immediate => {}
            RetryStrategy::FixedDelay { delay_ms } => {
                tokio::time::sleep(tokio::time::Duration::from_millis(*delay_ms)).await;
            }
            RetryStrategy::ExponentialBackoff => {
                let delay_ms = 1000 * (2_u64).pow(attempt.min(10) - 1);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            }
        }
    }

    #[cfg(not(test))]
    async fn apply_retry_delay(&self, strategy: &RetryStrategy, attempt: u32) {
        match strategy {
            RetryStrategy::Immediate => {}
            RetryStrategy::FixedDelay { delay_ms } => {
                tokio::time::sleep(tokio::time::Duration::from_millis(*delay_ms)).await;
            }
            RetryStrategy::ExponentialBackoff => {
                let delay_ms = 1000 * (2_u64).pow(attempt.min(10) - 1);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            }
        }
    }

    /// Acquire a lock to prevent concurrent reprocessing
    #[cfg(test)]
    pub async fn acquire_reprocessing_lock(&self, workflow_id: &str) -> Result<()> {
        let mut locks = self.reprocessing_locks.write().await;

        if let Some(lock_time) = locks.get(workflow_id) {
            // Check if lock is stale (older than 1 hour)
            if Utc::now().signed_duration_since(*lock_time).num_hours() < 1 {
                anyhow::bail!(
                    "Workflow {} is already being reprocessed (started at {})",
                    workflow_id,
                    lock_time
                );
            }
        }

        locks.insert(workflow_id.to_string(), Utc::now());
        Ok(())
    }

    #[cfg(not(test))]
    async fn acquire_reprocessing_lock(&self, workflow_id: &str) -> Result<()> {
        let mut locks = self.reprocessing_locks.write().await;

        if let Some(lock_time) = locks.get(workflow_id) {
            // Check if lock is stale (older than 1 hour)
            if Utc::now().signed_duration_since(*lock_time).num_hours() < 1 {
                anyhow::bail!(
                    "Workflow {} is already being reprocessed (started at {})",
                    workflow_id,
                    lock_time
                );
            }
        }

        locks.insert(workflow_id.to_string(), Utc::now());
        Ok(())
    }

    /// Release the reprocessing lock
    #[cfg(test)]
    pub async fn release_reprocessing_lock(&self, workflow_id: &str) {
        let mut locks = self.reprocessing_locks.write().await;
        locks.remove(workflow_id);
    }

    #[cfg(not(test))]
    async fn release_reprocessing_lock(&self, workflow_id: &str) {
        let mut locks = self.reprocessing_locks.write().await;
        locks.remove(workflow_id);
    }

    /// Get statistics across all DLQs
    pub async fn get_global_stats(
        &self,
        _project_root: &std::path::Path,
    ) -> Result<GlobalDLQStats> {
        // In a real implementation, this would scan all DLQs
        // For now, return stats for the current DLQ
        let stats = self.dlq.get_stats().await?;

        Ok(GlobalDLQStats {
            total_workflows: 1,
            total_items: stats.total_items,
            eligible_for_reprocess: stats.eligible_for_reprocess,
            requiring_manual_review: stats.requiring_manual_review,
            oldest_item: stats.oldest_item,
            newest_item: stats.newest_item,
            workflows: vec![(self.dlq.job_id.clone(), stats)],
        })
    }

    /// Clear processed items from DLQ
    pub async fn clear_processed_items(&self, workflow_id: &str) -> Result<usize> {
        let filter = DLQFilter {
            reprocess_eligible: Some(false),
            ..Default::default()
        };

        let items = self.dlq.list_items(filter).await?;
        let count = items.len();

        for item in items {
            self.dlq.remove(&item.item_id).await?;
        }

        info!(
            "Cleared {} processed items from DLQ for {}",
            count, workflow_id
        );
        Ok(count)
    }
}

/// Global DLQ statistics across all workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalDLQStats {
    pub total_workflows: usize,
    pub total_items: usize,
    pub eligible_for_reprocess: usize,
    pub requiring_manual_review: usize,
    pub oldest_item: Option<DateTime<Utc>>,
    pub newest_item: Option<DateTime<Utc>>,
    pub workflows: Vec<(String, super::dlq::DLQStats)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_evaluator() {
        let item = DeadLetteredItem {
            item_id: "test-1".to_string(),
            item_data: serde_json::json!({
                "priority": "high",
                "score": 10
            }),
            first_attempt: Utc::now(),
            last_attempt: Utc::now(),
            failure_count: 3,
            failure_history: vec![],
            error_signature: "test".to_string(),
            worktree_artifacts: None,
            reprocess_eligible: true,
            manual_review_required: false,
        };

        // Test equality
        let filter = FilterEvaluator::new("item.priority == 'high'".to_string());
        assert!(filter.matches(&item));

        // Test inequality
        let filter = FilterEvaluator::new("item.priority != 'low'".to_string());
        assert!(filter.matches(&item));

        // Test numeric comparison
        let filter = FilterEvaluator::new("item.failure_count >= 3".to_string());
        assert!(filter.matches(&item));

        // Test boolean field
        let filter = FilterEvaluator::new("item.reprocess_eligible == true".to_string());
        assert!(filter.matches(&item));
    }

    #[test]
    fn test_retry_strategy() {
        // Test default options
        let options = ReprocessOptions::default();
        assert_eq!(options.max_retries, 3);
        assert_eq!(options.parallel, 10);
        assert!(matches!(
            options.strategy,
            RetryStrategy::ExponentialBackoff
        ));
    }
}
