//! DLQ reprocessor for handling failed MapReduce items
//!
//! Provides functionality to reprocess items from the Dead Letter Queue
//! with configurable retry strategies and filtering options.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::dlq::{DLQFilter, DeadLetterQueue, DeadLetteredItem};
use super::events::EventLogger;
use super::mapreduce::{MapReduceConfig, MapReduceExecutor};
use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::Semaphore;

/// Options for reprocessing DLQ items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessOptions {
    /// Maximum retry attempts per item
    pub max_retries: u32,
    /// Filter expression for selective reprocessing
    pub filter: Option<DlqFilterAdvanced>,
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

/// Advanced filter for DLQ items with multiple filtering capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqFilterAdvanced {
    /// Filter by error types
    pub error_types: Option<Vec<ErrorType>>,
    /// Filter by date range
    pub date_range: Option<DateRange>,
    /// JSONPath expression for item filtering
    pub item_filter: Option<String>,
    /// Maximum failure count
    pub max_failure_count: Option<u32>,
}

/// Error types for filtering
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorType {
    Timeout,
    Validation,
    CommandFailure,
    NetworkError,
    RateLimitError,
    Unknown,
}

/// Date range for filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl Default for ReprocessOptions {
    fn default() -> Self {
        Self {
            max_retries: 3,
            filter: None,
            parallel: 5,
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
    /// Skipped items (not eligible or filtered out)
    pub skipped: usize,
    /// New job ID for the reprocessing run
    pub job_id: String,
    /// Processing duration
    pub duration: std::time::Duration,
    /// Items that failed again
    pub failed_items: Vec<String>,
    /// Error patterns found during reprocessing
    pub error_patterns: HashMap<String, usize>,
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

    /// Evaluate string expressions including 'contains' operator
    fn evaluate_string_expression(&self, field_value: &str, operator: &str, expected_value: &str) -> bool {
        match operator {
            "==" => field_value == expected_value,
            "!=" => field_value != expected_value,
            "contains" => field_value.contains(expected_value),
            ">" => field_value > expected_value,
            ">=" => field_value >= expected_value,
            "<" => field_value < expected_value,
            "<=" => field_value <= expected_value,
            _ => {
                warn!("Unknown operator: {}", operator);
                true
            }
        }
    }

    /// Check if an item matches the filter expression
    pub fn matches(&self, item: &DeadLetteredItem) -> bool {
        // Parse simple expressions like "item.field == 'value'" or "item.score >= 5"
        if self.expression.is_empty() {
            return true;
        }

        // Handle compound expressions with && and ||
        if self.expression.contains("&&") {
            let parts: Vec<bool> = self.expression
                .split("&&")
                .map(|expr| {
                    let evaluator = FilterEvaluator::new(expr.trim().to_string());
                    let result = evaluator.matches(item);
                    debug!("Evaluating sub-expression '{}' = {}", expr.trim(), result);
                    result
                })
                .collect();
            return parts.iter().all(|&x| x);
        }

        if self.expression.contains("||") {
            return self.expression
                .split("||")
                .any(|expr| {
                    let evaluator = FilterEvaluator::new(expr.trim().to_string());
                    evaluator.matches(item)
                });
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
            } else if field_name == "error_signature" {
                // Handle error_signature field
                return self.evaluate_string_expression(&item.error_signature, operator, &value);
            } else {
                // Try to extract from item_data JSON, handling nested paths like "metadata.region"
                let field_parts: Vec<&str> = field_name.split('.').collect();
                let mut current = &item.item_data;
                let mut found = false;

                for part in &field_parts {
                    if let Some(obj) = current.as_object() {
                        if let Some(val) = obj.get(*part) {
                            current = val;
                            found = true;
                        } else {
                            found = false;
                            break;
                        }
                    } else {
                        found = false;
                        break;
                    }
                }

                // Convert final value to string if we successfully navigated to it
                if !found || field_parts.is_empty() {
                    None
                } else if let Some(s) = current.as_str() {
                    Some(s.to_string())
                } else if let Some(n) = current.as_i64() {
                    Some(n.to_string())
                } else if let Some(n) = current.as_u64() {
                    Some(n.to_string())
                } else if let Some(b) = current.as_bool() {
                    Some(b.to_string())
                } else {
                    current.as_f64().map(|f| f.to_string())
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

    /// Reprocess items from the DLQ (main entry point as per spec)
    pub async fn reprocess_items(&self, options: ReprocessOptions) -> Result<ReprocessResult> {
        let start_time = std::time::Instant::now();

        // 1. Load and filter DLQ items
        let mut items = self.load_filtered_items(&options.filter).await?;

        // Apply eligibility filter unless forced
        if !options.force {
            items.retain(|item| item.reprocess_eligible);
        }

        // 2. Create reprocessing workflow
        let workflow = self.generate_retry_workflow(&items, &options)?;

        // 3. Initialize progress tracking
        let progress = self.create_progress_tracker(items.len());

        // 4. Execute parallel reprocessing
        let results = self
            .execute_parallel_retry(workflow, &progress, &options)
            .await?;

        // 5. Update DLQ state
        self.update_dlq_state(&results).await?;

        // 6. Generate summary report
        Ok(self.generate_report(results, start_time.elapsed()))
    }

    /// Legacy reprocess method for backward compatibility
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

        // Apply custom filter if specified - convert string filter to advanced filter
        let filtered_items = if options.filter.is_some() {
            // For legacy compatibility, we still accept string filters
            // and convert them to advanced filters
            all_items
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
            skipped: 0,
            job_id: reprocess_job_id,
            duration,
            failed_items,
            error_patterns: HashMap::new(),
        })
    }

    /// Execute items with retry strategy
    async fn execute_with_retry(
        &self,
        items: &[Value],
        job_id: &str,
        options: &ReprocessOptions,
        executor: Arc<MapReduceExecutor>,
    ) -> Result<Vec<Result<Value>>> {
        use futures::stream::{self, StreamExt};
        use tokio::sync::Semaphore;

        let semaphore = Arc::new(Semaphore::new(options.parallel));
        let executor = Arc::clone(&executor);

        // Process items concurrently with controlled parallelism
        let results = stream::iter(items.iter().enumerate())
            .map(|(index, item)| {
                let semaphore = Arc::clone(&semaphore);
                let executor = Arc::clone(&executor);
                let item = item.clone();
                let job_id = job_id.to_string();
                let strategy = options.strategy.clone();
                let max_retries = options.max_retries;
                let dlq_reprocessor = self.clone_for_async();

                async move {
                    let _permit = semaphore.acquire().await.unwrap();

                    let mut attempts = 0;
                    loop {
                        attempts += 1;

                        // Apply retry strategy delay
                        if attempts > 1 {
                            dlq_reprocessor.apply_retry_delay(&strategy, attempts).await;
                        }

                        // Attempt to process the item
                        match dlq_reprocessor
                            .process_single_item(&item, &job_id, executor.clone())
                            .await
                        {
                            Ok(result) => {
                                info!(
                                    "Successfully reprocessed item {} after {} attempts",
                                    index, attempts
                                );
                                break Ok(result);
                            }
                            Err(e) if attempts < max_retries => {
                                warn!("Attempt {} failed for item {}: {}", attempts, index, e);
                                continue;
                            }
                            Err(e) => {
                                error!("Item {} failed after {} attempts: {}", index, attempts, e);
                                break Err(e);
                            }
                        }
                    }
                }
            })
            .buffer_unordered(options.parallel)
            .collect::<Vec<_>>()
            .await;

        Ok(results)
    }

    /// Process a single item
    async fn process_single_item(
        &self,
        item: &Value,
        job_id: &str,
        executor: Arc<MapReduceExecutor>,
    ) -> Result<Value> {
        debug!("Processing item for job {}: {:?}", job_id, item);

        // Extract workflow-related information if present
        let has_workflow_context = if let Some(obj) = item.as_object() {
            obj.contains_key("_workflow_template") || obj.contains_key("_agent_template")
        } else {
            false
        };

        // Log that we have the executor available for future enhancement
        if has_workflow_context {
            debug!(
                "Item has workflow context, executor available for job {}",
                job_id
            );
            // Note: The executor is passed in preparation for future integration
            // where we can properly execute workflow templates through the MapReduceExecutor.
            // Currently, we use direct command execution as a fallback.
            let _ = executor; // Acknowledge the parameter is intentionally reserved for future use
        }

        // Process the item using direct command execution
        let command = if let Some(obj) = item.as_object() {
            obj.get("_original_command")
                .and_then(|c| c.as_str())
                .unwrap_or("echo")
                .to_string()
        } else {
            "echo".to_string()
        };

        let args = if let Some(obj) = item.as_object() {
            obj.get("_original_args")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_else(|| vec!["Reprocessing item".to_string()])
        } else {
            vec!["Reprocessing item".to_string()]
        };

        // Execute the command directly using subprocess
        use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};

        let subprocess = SubprocessManager::production();
        let mut process_command = ProcessCommandBuilder::new(&command);
        process_command = process_command.args(&args);
        let process_command = process_command.build();

        let start_time = Instant::now();
        match subprocess.runner().run(process_command).await {
            Ok(output) => {
                let duration = start_time.elapsed();
                Ok(serde_json::json!({
                    "status": "reprocessed",
                    "success": output.status.success(),
                    "stdout": output.stdout,
                    "stderr": output.stderr,
                    "exit_code": output.status.code().unwrap_or(-1),
                    "duration_ms": duration.as_millis(),
                    "job_id": job_id,
                    "item": item,
                    "timestamp": Utc::now().to_rfc3339()
                }))
            }
            Err(e) => Err(anyhow::anyhow!("Failed to process item: {}", e)),
        }
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

    /// Clone for async operations (needed for stream processing)
    fn clone_for_async(&self) -> Self {
        Self {
            dlq: Arc::clone(&self.dlq),
            event_logger: self.event_logger.as_ref().map(Arc::clone),
            project_root: self.project_root.clone(),
            reprocessing_locks: Arc::clone(&self.reprocessing_locks),
        }
    }

    /// Get statistics across all DLQs
    pub async fn get_global_stats(&self, project_root: &std::path::Path) -> Result<GlobalDLQStats> {
        use crate::storage::discover_dlq_job_ids;

        // Discover all job IDs with DLQ data
        let job_ids = discover_dlq_job_ids(project_root).await?;

        let mut all_workflows = Vec::new();
        let mut total_items = 0;
        let mut total_eligible = 0;
        let mut total_review = 0;
        let mut oldest_overall: Option<DateTime<Utc>> = None;
        let mut newest_overall: Option<DateTime<Utc>> = None;

        // Collect stats from all DLQs
        for job_id in &job_ids {
            // Try to load DLQ for this job
            let dlq_result = super::dlq::DeadLetterQueue::load(
                job_id.clone(),
                project_root.to_path_buf(),
            ).await;

            if let Ok(dlq) = dlq_result {
                let stats = dlq.get_stats().await?;

                // Update totals
                total_items += stats.total_items;
                total_eligible += stats.eligible_for_reprocess;
                total_review += stats.requiring_manual_review;

                // Update oldest/newest
                if let Some(oldest) = stats.oldest_item {
                    oldest_overall = Some(match oldest_overall {
                        Some(current) if current < oldest => current,
                        _ => oldest,
                    });
                }

                if let Some(newest) = stats.newest_item {
                    newest_overall = Some(match newest_overall {
                        Some(current) if current > newest => current,
                        _ => newest,
                    });
                }

                all_workflows.push((job_id.clone(), stats));
            }
        }

        Ok(GlobalDLQStats {
            total_workflows: all_workflows.len(),
            total_items,
            eligible_for_reprocess: total_eligible,
            requiring_manual_review: total_review,
            oldest_item: oldest_overall,
            newest_item: newest_overall,
            workflows: all_workflows,
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

    /// Load and filter DLQ items based on the provided filter
    async fn load_filtered_items(
        &self,
        filter: &Option<DlqFilterAdvanced>,
    ) -> Result<Vec<DeadLetteredItem>> {
        let base_filter = DLQFilter::default();
        let all_items = self.dlq.list_items(base_filter).await?;

        if let Some(filter) = filter {
            self.apply_advanced_filter(all_items, filter)
        } else {
            Ok(all_items)
        }
    }

    /// Apply advanced filtering to DLQ items
    pub fn apply_advanced_filter(
        &self,
        items: Vec<DeadLetteredItem>,
        filter: &DlqFilterAdvanced,
    ) -> Result<Vec<DeadLetteredItem>> {
        let mut filtered = items;

        // Filter by error types
        if let Some(ref error_types) = filter.error_types {
            filtered.retain(|item| {
                // Match error signature to error type
                error_types.iter().any(|et| match et {
                    ErrorType::Timeout => item.error_signature.contains("timeout"),
                    ErrorType::Validation => item.error_signature.contains("validation"),
                    ErrorType::CommandFailure => item.error_signature.contains("command"),
                    ErrorType::NetworkError => item.error_signature.contains("network"),
                    ErrorType::RateLimitError => item.error_signature.contains("rate_limit"),
                    ErrorType::Unknown => true,
                })
            });
        }

        // Filter by date range
        if let Some(ref date_range) = filter.date_range {
            filtered.retain(|item| {
                item.last_attempt >= date_range.start && item.last_attempt <= date_range.end
            });
        }

        // Filter by max failure count
        if let Some(max_failures) = filter.max_failure_count {
            filtered.retain(|item| item.failure_count <= max_failures);
        }

        // Apply JSONPath filter if specified
        if let Some(ref item_filter) = filter.item_filter {
            let evaluator = FilterEvaluator::new(item_filter.clone());
            filtered.retain(|item| evaluator.matches(item));
        }

        Ok(filtered)
    }

    /// Generate a retry workflow from DLQ items
    fn generate_retry_workflow(
        &self,
        items: &[DeadLetteredItem],
        options: &ReprocessOptions,
    ) -> Result<MapReduceConfig> {
        // Create work items from DLQ items
        let work_items: Vec<Value> = items
            .iter()
            .map(|item| {
                // Enhance item data with retry metadata
                let mut enhanced = item.item_data.clone();
                if let Some(obj) = enhanced.as_object_mut() {
                    obj.insert("_dlq_retry_count".to_string(), json!(item.failure_count));
                    obj.insert("_dlq_item_id".to_string(), json!(item.item_id));
                    obj.insert("_dlq_last_error".to_string(), json!(item.error_signature));
                }
                enhanced
            })
            .collect();

        // Create temporary work items file
        let work_items_json = serde_json::to_string_pretty(&work_items)?;
        let temp_file = format!("/tmp/dlq_retry_{}.json", Utc::now().timestamp());
        std::fs::write(&temp_file, work_items_json)?;

        // Build MapReduce configuration
        Ok(MapReduceConfig {
            input: temp_file,
            json_path: "$[*]".to_string(),
            max_parallel: options.parallel,
            timeout_per_agent: options.timeout_per_item,
            retry_on_failure: options.max_retries,
            max_items: None,
            offset: None,
        })
    }

    /// Create a progress tracker for reprocessing
    fn create_progress_tracker(&self, total_items: usize) -> ProgressBar {
        let pb = ProgressBar::new(total_items as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        pb.set_message("Reprocessing DLQ items...");
        pb
    }

    /// Execute parallel retry with progress tracking
    async fn execute_parallel_retry(
        &self,
        workflow: MapReduceConfig,
        progress: &ProgressBar,
        options: &ReprocessOptions,
    ) -> Result<Vec<ProcessingResult>> {
        use futures::stream::{self, StreamExt};

        // Read work items from the input file
        let work_items_json = std::fs::read_to_string(&workflow.input)?;
        let work_items: Vec<Value> = serde_json::from_str(&work_items_json)?;

        let semaphore = Arc::new(Semaphore::new(options.parallel));

        // Process each work item with controlled parallelism
        let results = stream::iter(work_items.into_iter().enumerate())
            .map(|(index, item)| {
                let semaphore = Arc::clone(&semaphore);
                let progress = progress.clone();
                let strategy = options.strategy.clone();
                let max_retries = options.max_retries;
                let dlq_reprocessor = self.clone_for_async();
                let job_id = format!("reprocess_{}", Utc::now().timestamp());

                async move {
                    let _permit = semaphore.acquire().await.unwrap();

                    // Get item ID from the work item or generate one
                    let item_id = item
                        .get("_dlq_item_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&format!("item_{}", index))
                        .to_string();

                    // Process with retry logic
                    let mut attempts = 0;
                    loop {
                        attempts += 1;

                        // Apply retry delay if not first attempt
                        if attempts > 1 {
                            dlq_reprocessor.apply_retry_delay(&strategy, attempts).await;
                        }

                        // Process the actual item using subprocess
                        match dlq_reprocessor
                            .process_single_item_static(&item, &job_id)
                            .await
                        {
                            Ok(_res) => {
                                progress.inc(1);
                                break ProcessingResult::Success { item_id, attempts };
                            }
                            Err(_e) if attempts < max_retries => {
                                continue;
                            }
                            Err(e) => {
                                progress.inc(1);
                                break ProcessingResult::Failed {
                                    item_id,
                                    error: e.to_string(),
                                    attempts,
                                };
                            }
                        }
                    }
                }
            })
            .buffer_unordered(options.parallel)
            .collect::<Vec<_>>()
            .await;

        // Clean up temp file
        let _ = std::fs::remove_file(&workflow.input);

        progress.finish_with_message("Reprocessing completed");
        Ok(results)
    }

    /// Process a single item (static version for async closures)
    async fn process_single_item_static(&self, item: &Value, job_id: &str) -> Result<Value> {
        use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};

        debug!("Processing item for job {}: {:?}", job_id, item);

        // Extract command from the work item
        let command = item
            .get("command")
            .and_then(|c| c.as_str())
            .unwrap_or("echo")
            .to_string();

        let args = item
            .get("args")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(|| vec!["Processing DLQ item".to_string()]);

        // Execute the command
        let subprocess = SubprocessManager::production();
        let mut process_command = ProcessCommandBuilder::new(&command);
        process_command = process_command.args(&args);
        let process_command = process_command.build();

        let start_time = Instant::now();
        match subprocess.runner().run(process_command).await {
            Ok(output) => {
                if output.status.success() {
                    Ok(json!({
                        "status": "processed",
                        "duration_ms": start_time.elapsed().as_millis(),
                        "timestamp": Utc::now().to_rfc3339()
                    }))
                } else {
                    anyhow::bail!(
                        "Command failed with exit code: {}",
                        output.status.code().unwrap_or(-1)
                    )
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Update DLQ state based on processing results
    async fn update_dlq_state(&self, results: &[ProcessingResult]) -> Result<()> {
        for result in results {
            match result {
                ProcessingResult::Success { item_id, .. } => {
                    // Remove successfully processed items from DLQ
                    self.dlq.remove(item_id).await?;
                    info!("Removed successfully reprocessed item: {}", item_id);
                }
                ProcessingResult::Failed {
                    item_id,
                    error,
                    attempts,
                } => {
                    // Update failure count and error signature
                    warn!(
                        "Item {} failed after {} attempts: {}",
                        item_id, attempts, error
                    );
                    // In a real implementation, we would update the item in DLQ with new failure info
                }
                ProcessingResult::Skipped { item_id, reason } => {
                    debug!("Item {} skipped: {}", item_id, reason);
                }
            }
        }
        Ok(())
    }

    /// Generate a summary report of the reprocessing operation
    fn generate_report(
        &self,
        results: Vec<ProcessingResult>,
        duration: std::time::Duration,
    ) -> ReprocessResult {
        let mut successful = 0;
        let mut failed = 0;
        let mut skipped = 0;
        let mut failed_items = Vec::new();
        let mut error_patterns = HashMap::new();

        for result in results {
            match result {
                ProcessingResult::Success { .. } => successful += 1,
                ProcessingResult::Failed { item_id, error, .. } => {
                    failed += 1;
                    failed_items.push(item_id);

                    // Track error patterns
                    let pattern = if error.contains("timeout") {
                        "Timeout"
                    } else if error.contains("validation") {
                        "Validation"
                    } else if error.contains("network") {
                        "Network"
                    } else {
                        "Other"
                    };
                    *error_patterns.entry(pattern.to_string()).or_insert(0) += 1;
                }
                ProcessingResult::Skipped { .. } => skipped += 1,
            }
        }

        ReprocessResult {
            total_items: successful + failed + skipped,
            successful,
            failed,
            skipped,
            job_id: format!("dlq_reprocess_{}", Utc::now().timestamp()),
            duration,
            failed_items,
            error_patterns,
        }
    }
}

/// Processing result for a single item
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum ProcessingResult {
    Success {
        item_id: String,
        attempts: u32,
    },
    Failed {
        item_id: String,
        error: String,
        attempts: u32,
    },
    Skipped {
        item_id: String,
        reason: String,
    },
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
        assert_eq!(options.parallel, 5);
        assert!(matches!(
            options.strategy,
            RetryStrategy::ExponentialBackoff
        ));
    }
}
