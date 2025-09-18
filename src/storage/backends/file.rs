//! File-based storage backend implementation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::{self};
use futures_util::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;

use crate::storage::{
    config::{BackendConfig, FileConfig, StorageConfig},
    error::{StorageError, StorageResult},
    lock::{FileLockGuard, LockBackend, StorageLock, StorageLockGuard},
    traits::*,
    types::*,
};

/// File-based storage backend
pub struct FileBackend {
    config: FileConfig,
    base_dir: PathBuf,
    locks: Arc<RwLock<HashMap<String, StorageLock>>>,
}

impl FileBackend {
    /// Create a new file backend
    pub async fn new(config: &StorageConfig) -> StorageResult<Self> {
        let file_config = match &config.backend_config {
            BackendConfig::File(cfg) => cfg.clone(),
            _ => {
                return Err(StorageError::configuration(
                    "Invalid backend config for file storage",
                ))
            }
        };

        let base_dir = if file_config.use_global {
            dirs::home_dir()
                .ok_or_else(|| StorageError::configuration("Could not determine home directory"))?
                .join(".prodigy")
        } else {
            file_config.base_dir.clone()
        };

        // Ensure base directory exists
        fs::create_dir_all(&base_dir)
            .await
            .map_err(|e| StorageError::Io(e))?;

        Ok(Self {
            config: file_config,
            base_dir,
            locks: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get path for a specific storage domain
    fn get_path(&self, domain: &str, key: &str) -> PathBuf {
        self.base_dir.join(domain).join(key)
    }

    /// Ensure directory exists
    async fn ensure_dir(&self, path: &Path) -> StorageResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| StorageError::Io(e))?;
        }
        Ok(())
    }

    /// Read JSON file
    async fn read_json<T: for<'de> Deserialize<'de>>(&self, path: &Path) -> StorageResult<T> {
        let content = fs::read_to_string(path)
            .await
            .map_err(|e| StorageError::Io(e))?;
        serde_json::from_str(&content).map_err(|e| StorageError::serialization(e))
    }

    /// Write JSON file
    async fn write_json<T: Serialize>(&self, path: &Path, data: &T) -> StorageResult<()> {
        self.ensure_dir(path).await?;
        let content = serde_json::to_string_pretty(data)?;
        fs::write(path, content)
            .await
            .map_err(|e| StorageError::Io(e))?;
        Ok(())
    }
}

#[async_trait]
impl UnifiedStorage for FileBackend {
    fn session_storage(&self) -> &dyn SessionStorage {
        self
    }

    fn event_storage(&self) -> &dyn EventStorage {
        self
    }

    fn checkpoint_storage(&self) -> &dyn CheckpointStorage {
        self
    }

    fn dlq_storage(&self) -> &dyn DLQStorage {
        self
    }

    fn workflow_storage(&self) -> &dyn WorkflowStorage {
        self
    }

    async fn acquire_lock(
        &self,
        key: &str,
        ttl: Duration,
    ) -> StorageResult<Box<dyn StorageLockGuard>> {
        if !self.config.enable_file_locks {
            // Return a no-op lock guard
            let lock = StorageLock::new(key.to_string(), "file-backend".to_string(), ttl);
            let lock_file = self.get_path("locks", &format!("{}.lock", key));
            return Ok(Box::new(FileLockGuard::new(lock, lock_file)));
        }

        let lock_file = self.get_path("locks", &format!("{}.lock", key));
        self.ensure_dir(&lock_file).await?;

        // Try to create lock file exclusively
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_file)
            .await
        {
            Ok(mut file) => {
                let lock = StorageLock::new(key.to_string(), "file-backend".to_string(), ttl);

                // Write lock info to file
                // Write lock token to file
                let lock_info = lock.token.clone();
                file.write_all(lock_info.as_bytes()).await?;

                // Store in memory
                self.locks
                    .write()
                    .await
                    .insert(key.to_string(), lock.clone());

                Ok(Box::new(FileLockGuard::new(lock, lock_file)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(StorageError::conflict(
                format!("Lock already held: {}", key),
            )),
            Err(e) => Err(StorageError::Io(e)),
        }
    }

    async fn health_check(&self) -> StorageResult<HealthStatus> {
        let start = std::time::Instant::now();

        // Try to write a test file
        let test_file = self.base_dir.join(".health_check");
        let result = fs::write(&test_file, "health_check").await;
        let _ = fs::remove_file(&test_file).await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(_) => Ok(HealthStatus {
                healthy: true,
                backend_type: "file".to_string(),
                connection_status: ConnectionStatus::Connected,
                latency_ms,
                errors: vec![],
            }),
            Err(e) => Ok(HealthStatus {
                healthy: false,
                backend_type: "file".to_string(),
                connection_status: ConnectionStatus::Disconnected,
                latency_ms,
                errors: vec![e.to_string()],
            }),
        }
    }

    async fn get_metrics(&self) -> StorageResult<StorageMetrics> {
        // Calculate storage size
        let mut storage_size_bytes = 0u64;
        let mut operations_total = 0u64;

        // Walk directory to calculate size
        if let Ok(mut entries) = fs::read_dir(&self.base_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(metadata) = entry.metadata().await {
                    storage_size_bytes += metadata.len();
                    operations_total += 1;
                }
            }
        }

        Ok(StorageMetrics {
            operations_total,
            operations_failed: 0,
            average_latency_ms: 5.0,
            storage_size_bytes,
            active_connections: 1,
        })
    }
}

#[async_trait]
impl SessionStorage for FileBackend {
    async fn save(&self, session: &PersistedSession) -> StorageResult<()> {
        let path = self.get_path("sessions", &format!("{}.json", session.id.0));
        self.write_json(&path, session).await
    }

    async fn load(&self, id: &SessionId) -> StorageResult<Option<PersistedSession>> {
        let path = self.get_path("sessions", &format!("{}.json", id.0));
        if path.exists() {
            Ok(Some(self.read_json(&path).await?))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, filter: SessionFilter) -> StorageResult<Vec<SessionId>> {
        let sessions_dir = self.base_dir.join("sessions");
        if !sessions_dir.exists() {
            return Ok(vec![]);
        }

        let mut sessions = Vec::new();
        let mut entries = fs::read_dir(sessions_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".json") {
                    let session_id = SessionId(name.trim_end_matches(".json").to_string());

                    // Apply filter if needed
                    if filter.state.is_some() || filter.after.is_some() || filter.before.is_some() {
                        if let Ok(session) = SessionStorage::load(self, &session_id).await {
                            if let Some(session) = session {
                                if let Some(ref state) = filter.state {
                                    if session.state != *state {
                                        continue;
                                    }
                                }
                                if let Some(after) = filter.after {
                                    if session.started_at < after {
                                        continue;
                                    }
                                }
                                if let Some(before) = filter.before {
                                    if session.started_at > before {
                                        continue;
                                    }
                                }
                            }
                        }
                    }

                    sessions.push(session_id);

                    if let Some(limit) = filter.limit {
                        if sessions.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }

        Ok(sessions)
    }

    async fn delete(&self, id: &SessionId) -> StorageResult<()> {
        let path = self.get_path("sessions", &format!("{}.json", id.0));
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }

    async fn update_state(&self, id: &SessionId, state: SessionState) -> StorageResult<()> {
        if let Some(mut session) = SessionStorage::load(self, id).await? {
            session.state = state;
            session.updated_at = Utc::now();
            SessionStorage::save(self, &session).await
        } else {
            Err(StorageError::not_found(format!(
                "Session not found: {}",
                id.0
            )))
        }
    }

    async fn get_stats(&self, id: &SessionId) -> StorageResult<SessionStats> {
        let session = SessionStorage::load(self, id)
            .await?
            .ok_or_else(|| StorageError::not_found(format!("Session not found: {}", id.0)))?;

        let duration = (session.updated_at - session.started_at)
            .to_std()
            .unwrap_or_default();

        Ok(SessionStats {
            total_duration: duration,
            commands_executed: session.iterations_completed as usize,
            errors_encountered: 0,
            files_modified: session.files_changed as usize,
        })
    }
}

#[async_trait]
impl EventStorage for FileBackend {
    async fn append(&self, events: Vec<EventRecord>) -> StorageResult<()> {
        for event in events {
            let dir = self.get_path("events", &event.job_id);
            self.ensure_dir(&dir).await?;

            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%f");
            let file_path = dir.join(format!("event_{}.jsonl", timestamp));

            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .await?;

            let line = serde_json::to_string(&event)?;
            file.write_all((line + "\n").as_bytes()).await?;
        }
        Ok(())
    }

    async fn query(&self, filter: EventFilter) -> StorageResult<EventStream> {
        let events_dir = self.base_dir.join("events");
        if !events_dir.exists() {
            return Ok(Box::pin(stream::empty()));
        }

        // Collect all matching event files
        let mut files = Vec::new();

        if let Some(ref job_id) = filter.job_id {
            let job_dir = events_dir.join(job_id);
            if job_dir.exists() {
                let mut entries = fs::read_dir(job_dir).await?;
                while let Some(entry) = entries.next_entry().await? {
                    if entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl") {
                        files.push(entry.path());
                    }
                }
            }
        } else {
            // Scan all job directories
            let mut job_entries = fs::read_dir(events_dir).await?;
            while let Some(job_entry) = job_entries.next_entry().await? {
                if job_entry.file_type().await?.is_dir() {
                    let mut entries = fs::read_dir(job_entry.path()).await?;
                    while let Some(entry) = entries.next_entry().await? {
                        if entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl") {
                            files.push(entry.path());
                        }
                    }
                }
            }
        }

        // Sort files by name (which includes timestamp)
        files.sort();

        // Create stream from files
        let limit = filter.limit.unwrap_or(usize::MAX);
        let filter = filter.clone();
        let stream = stream::iter(files)
            .then(move |file| {
                let filter = filter.clone();
                async move {
                    let file = fs::File::open(&file).await?;
                    let reader = BufReader::new(file);
                    let mut lines = reader.lines();
                    let mut events = Vec::new();

                    while let Some(line) = lines.next_line().await? {
                        if let Ok(event) = serde_json::from_str::<EventRecord>(&line) {
                            // Apply filters
                            if let Some(ref event_type) = filter.event_type {
                                if event.event_type != *event_type {
                                    continue;
                                }
                            }
                            if let Some(after) = filter.after {
                                if event.timestamp < after {
                                    continue;
                                }
                            }
                            if let Some(before) = filter.before {
                                if event.timestamp > before {
                                    continue;
                                }
                            }
                            events.push(event);
                        }
                    }
                    Ok::<Vec<EventRecord>, anyhow::Error>(events)
                }
            })
            .map(move |result| match result {
                Ok(events) => stream::iter(events.into_iter().map(Ok).collect::<Vec<_>>()),
                Err(e) => stream::iter(vec![Err(e)]),
            })
            .flatten()
            .take(limit);

        Ok(Box::pin(stream))
    }

    async fn aggregate(&self, job_id: &str) -> StorageResult<EventStats> {
        let filter = EventFilter {
            job_id: Some(job_id.to_string()),
            ..Default::default()
        };

        let mut stream = self.query(filter).await?;
        let mut stats = EventStats {
            total_events: 0,
            events_by_type: HashMap::new(),
            success_count: 0,
            failure_count: 0,
            average_duration: None,
            first_event: None,
            last_event: None,
        };

        while let Some(event) = stream.next().await {
            if let Ok(event) = event {
                stats.total_events += 1;
                *stats
                    .events_by_type
                    .entry(event.event_type.clone())
                    .or_insert(0) += 1;

                if stats.first_event.is_none() || event.timestamp < stats.first_event.unwrap() {
                    stats.first_event = Some(event.timestamp);
                }
                if stats.last_event.is_none() || event.timestamp > stats.last_event.unwrap() {
                    stats.last_event = Some(event.timestamp);
                }

                // Check for success/failure in event data
                if let Some(success) = event.data.get("success").and_then(|v| v.as_bool()) {
                    if success {
                        stats.success_count += 1;
                    } else {
                        stats.failure_count += 1;
                    }
                }
            }
        }

        Ok(stats)
    }

    async fn subscribe(&self, _filter: EventFilter) -> StorageResult<EventSubscription> {
        // File backend doesn't support real-time subscriptions
        // Return a dummy subscription that never receives events
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(tx); // Close immediately

        Ok(EventSubscription {
            id: uuid::Uuid::new_v4().to_string(),
            filter: _filter,
            receiver: rx,
        })
    }

    async fn count(&self, filter: EventFilter) -> StorageResult<usize> {
        let mut stream = self.query(filter).await?;
        let mut count = 0;

        while let Some(event) = stream.next().await {
            if event.is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }

    async fn archive(&self, before: DateTime<Utc>) -> StorageResult<usize> {
        // Archive old events by moving them to an archive directory
        let events_dir = self.base_dir.join("events");
        let archive_dir = self.base_dir.join("events_archive");

        if !events_dir.exists() {
            return Ok(0);
        }

        fs::create_dir_all(&archive_dir).await?;

        let mut archived_count = 0;
        let mut job_entries = fs::read_dir(events_dir).await?;

        while let Some(job_entry) = job_entries.next_entry().await? {
            if job_entry.file_type().await?.is_dir() {
                let job_id = job_entry.file_name();
                let job_archive_dir = archive_dir.join(&job_id);
                fs::create_dir_all(&job_archive_dir).await?;

                let mut entries = fs::read_dir(job_entry.path()).await?;
                while let Some(entry) = entries.next_entry().await? {
                    if let Ok(metadata) = entry.metadata().await {
                        if let Ok(modified) = metadata.modified() {
                            let modified_time: DateTime<Utc> = modified.into();
                            if modified_time < before {
                                let dest = job_archive_dir.join(entry.file_name());
                                fs::rename(entry.path(), dest).await?;
                                archived_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(archived_count)
    }
}

#[async_trait]
impl CheckpointStorage for FileBackend {
    async fn save(&self, checkpoint: &WorkflowCheckpoint) -> StorageResult<()> {
        let path = self.get_path("checkpoints", &format!("{}.json", checkpoint.id));
        self.write_json(&path, checkpoint).await
    }

    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowCheckpoint>> {
        let path = self.get_path("checkpoints", &format!("{}.json", id));
        if path.exists() {
            Ok(Some(self.read_json(&path).await?))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, filter: CheckpointFilter) -> StorageResult<Vec<CheckpointInfo>> {
        let checkpoints_dir = self.base_dir.join("checkpoints");
        if !checkpoints_dir.exists() {
            return Ok(vec![]);
        }

        let mut checkpoints = Vec::new();
        let mut entries = fs::read_dir(checkpoints_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".json") {
                    let id = name.trim_end_matches(".json");

                    if let Ok(checkpoint) = CheckpointStorage::load(self, id).await {
                        if let Some(checkpoint) = checkpoint {
                            // Apply filter
                            if let Some(ref workflow_id) = filter.workflow_id {
                                if checkpoint.workflow_id != *workflow_id {
                                    continue;
                                }
                            }
                            if let Some(after) = filter.after {
                                if checkpoint.created_at < after {
                                    continue;
                                }
                            }
                            if let Some(before) = filter.before {
                                if checkpoint.created_at > before {
                                    continue;
                                }
                            }

                            let metadata = entry.metadata().await?;
                            checkpoints.push(CheckpointInfo {
                                id: checkpoint.id,
                                workflow_id: checkpoint.workflow_id,
                                created_at: checkpoint.created_at,
                                step_index: checkpoint.step_index,
                                size_bytes: metadata.len() as usize,
                            });

                            if let Some(limit) = filter.limit {
                                if checkpoints.len() >= limit {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort by creation time, newest first
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(checkpoints)
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        let path = self.get_path("checkpoints", &format!("{}.json", id));
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }

    async fn get_latest(&self, workflow_id: &str) -> StorageResult<Option<WorkflowCheckpoint>> {
        let filter = CheckpointFilter {
            workflow_id: Some(workflow_id.to_string()),
            limit: Some(1),
            ..Default::default()
        };

        let checkpoints = CheckpointStorage::list(self, filter).await?;
        if let Some(info) = checkpoints.first() {
            CheckpointStorage::load(self, &info.id).await
        } else {
            Ok(None)
        }
    }

    async fn cleanup(&self, keep_last: usize) -> StorageResult<usize> {
        let checkpoints = CheckpointStorage::list(self, Default::default()).await?;

        if checkpoints.len() <= keep_last {
            return Ok(0);
        }

        let mut deleted = 0;
        for checkpoint in checkpoints.iter().skip(keep_last) {
            CheckpointStorage::delete(self, &checkpoint.id).await?;
            deleted += 1;
        }

        Ok(deleted)
    }
}

#[async_trait]
impl DLQStorage for FileBackend {
    async fn enqueue(&self, item: DLQItem) -> StorageResult<()> {
        let dir = self.get_path("dlq", &item.job_id);
        self.ensure_dir(&dir).await?;

        let path = dir.join(format!("{}.json", item.id));
        self.write_json(&path, &item).await
    }

    async fn dequeue(&self, limit: usize) -> StorageResult<Vec<DLQItem>> {
        let dlq_dir = self.base_dir.join("dlq");
        if !dlq_dir.exists() {
            return Ok(vec![]);
        }

        let mut items = Vec::new();
        let mut job_entries = fs::read_dir(dlq_dir).await?;

        while let Some(job_entry) = job_entries.next_entry().await? {
            if job_entry.file_type().await?.is_dir() && items.len() < limit {
                let mut entries = fs::read_dir(job_entry.path()).await?;

                while let Some(entry) = entries.next_entry().await? {
                    if items.len() >= limit {
                        break;
                    }

                    if let Some(name) = entry.file_name().to_str() {
                        if name.ends_with(".json") {
                            if let Ok(item) = self.read_json::<DLQItem>(&entry.path()).await {
                                items.push(item);
                                // Remove the file after dequeuing
                                fs::remove_file(entry.path()).await?;
                            }
                        }
                    }
                }
            }
        }

        Ok(items)
    }

    async fn list(&self, filter: DLQFilter) -> StorageResult<Vec<DLQItem>> {
        let dlq_dir = self.base_dir.join("dlq");
        if !dlq_dir.exists() {
            return Ok(vec![]);
        }

        let mut items = Vec::new();

        if let Some(ref job_id) = filter.job_id {
            let job_dir = dlq_dir.join(job_id);
            if job_dir.exists() {
                let mut entries = fs::read_dir(job_dir).await?;

                while let Some(entry) = entries.next_entry().await? {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.ends_with(".json") {
                            if let Ok(item) = self.read_json::<DLQItem>(&entry.path()).await {
                                // Apply filters
                                if let Some(after) = filter.after {
                                    if item.enqueued_at < after {
                                        continue;
                                    }
                                }
                                if let Some(before) = filter.before {
                                    if item.enqueued_at > before {
                                        continue;
                                    }
                                }
                                if let Some(min_retry) = filter.min_retry_count {
                                    if item.retry_count < min_retry {
                                        continue;
                                    }
                                }
                                if let Some(max_retry) = filter.max_retry_count {
                                    if item.retry_count > max_retry {
                                        continue;
                                    }
                                }

                                items.push(item);

                                if let Some(limit) = filter.limit {
                                    if items.len() >= limit {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(items)
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        let dlq_dir = self.base_dir.join("dlq");

        // Search for the item across all job directories
        let mut job_entries = fs::read_dir(dlq_dir).await?;

        while let Some(job_entry) = job_entries.next_entry().await? {
            if job_entry.file_type().await?.is_dir() {
                let item_path = job_entry.path().join(format!("{}.json", id));
                if item_path.exists() {
                    fs::remove_file(item_path).await?;
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    async fn mark_processed(&self, id: &str) -> StorageResult<()> {
        // For file backend, marking as processed means deleting
        DLQStorage::delete(self, id).await
    }

    async fn get_stats(&self, job_id: &str) -> StorageResult<DLQStats> {
        let filter = DLQFilter {
            job_id: Some(job_id.to_string()),
            ..Default::default()
        };

        let items = DLQStorage::list(self, filter).await?;

        let mut stats = DLQStats {
            total_items: items.len(),
            items_by_retry_count: HashMap::new(),
            oldest_item: None,
            newest_item: None,
            average_retry_count: 0.0,
        };

        if !items.is_empty() {
            let mut total_retries = 0u32;

            for item in &items {
                *stats
                    .items_by_retry_count
                    .entry(item.retry_count)
                    .or_insert(0) += 1;
                total_retries += item.retry_count;

                if stats.oldest_item.is_none() || item.enqueued_at < stats.oldest_item.unwrap() {
                    stats.oldest_item = Some(item.enqueued_at);
                }
                if stats.newest_item.is_none() || item.enqueued_at > stats.newest_item.unwrap() {
                    stats.newest_item = Some(item.enqueued_at);
                }
            }

            stats.average_retry_count = total_retries as f64 / items.len() as f64;
        }

        Ok(stats)
    }

    async fn purge(&self, older_than: Duration) -> StorageResult<usize> {
        let cutoff = Utc::now() - chrono::Duration::from_std(older_than).unwrap();
        let dlq_dir = self.base_dir.join("dlq");

        if !dlq_dir.exists() {
            return Ok(0);
        }

        let mut purged = 0;
        let mut job_entries = fs::read_dir(dlq_dir).await?;

        while let Some(job_entry) = job_entries.next_entry().await? {
            if job_entry.file_type().await?.is_dir() {
                let mut entries = fs::read_dir(job_entry.path()).await?;

                while let Some(entry) = entries.next_entry().await? {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.ends_with(".json") {
                            if let Ok(item) = self.read_json::<DLQItem>(&entry.path()).await {
                                if item.enqueued_at < cutoff {
                                    fs::remove_file(entry.path()).await?;
                                    purged += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(purged)
    }
}

#[async_trait]
impl WorkflowStorage for FileBackend {
    async fn save(&self, workflow: &WorkflowDefinition) -> StorageResult<()> {
        let path = self.get_path("workflows", &format!("{}.json", workflow.id));
        self.write_json(&path, workflow).await
    }

    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowDefinition>> {
        let path = self.get_path("workflows", &format!("{}.json", id));
        if path.exists() {
            Ok(Some(self.read_json(&path).await?))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, filter: WorkflowFilter) -> StorageResult<Vec<WorkflowInfo>> {
        let workflows_dir = self.base_dir.join("workflows");
        if !workflows_dir.exists() {
            return Ok(vec![]);
        }

        let mut workflows = Vec::new();
        let mut entries = fs::read_dir(workflows_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".json") {
                    let id = name.trim_end_matches(".json");

                    if let Ok(workflow) = WorkflowStorage::load(self, id).await {
                        if let Some(workflow) = workflow {
                            // Apply filter
                            if let Some(ref filter_name) = filter.name {
                                if !workflow.name.contains(filter_name) {
                                    continue;
                                }
                            }
                            if let Some(ref tag) = filter.tag {
                                if !workflow.metadata.tags.contains(tag) {
                                    continue;
                                }
                            }
                            if let Some(ref author) = filter.author {
                                if workflow.metadata.author.as_ref() != Some(author) {
                                    continue;
                                }
                            }

                            workflows.push(WorkflowInfo {
                                id: workflow.id,
                                name: workflow.name,
                                version: workflow.version,
                                created_at: workflow.created_at,
                                execution_count: 0,
                            });

                            if let Some(limit) = filter.limit {
                                if workflows.len() >= limit {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(workflows)
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        let path = self.get_path("workflows", &format!("{}.json", id));
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }

    async fn update_metadata(&self, id: &str, metadata: WorkflowMetadata) -> StorageResult<()> {
        if let Some(mut workflow) = WorkflowStorage::load(self, id).await? {
            workflow.metadata = metadata;
            workflow.updated_at = Utc::now();
            WorkflowStorage::save(self, &workflow).await
        } else {
            Err(StorageError::not_found(format!(
                "Workflow not found: {}",
                id
            )))
        }
    }

    async fn get_history(&self, _id: &str) -> StorageResult<Vec<WorkflowExecution>> {
        // File backend doesn't track execution history by default
        Ok(vec![])
    }
}

// Stub implementation for LockBackend
#[async_trait]
impl LockBackend for FileBackend {
    async fn try_acquire(
        &self,
        key: &str,
        holder: &str,
        ttl: Duration,
    ) -> StorageResult<Box<dyn StorageLockGuard>> {
        let lock_file = self.get_path("locks", &format!("{}.lock", key));
        self.ensure_dir(&lock_file).await?;

        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_file)
            .await
        {
            Ok(mut file) => {
                let lock = StorageLock::new(key.to_string(), holder.to_string(), ttl);
                // Write lock token to file
                let lock_info = lock.token.clone();
                file.write_all(lock_info.as_bytes()).await?;
                Ok(Box::new(FileLockGuard::new(lock, lock_file)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(StorageError::conflict(
                format!("Lock already held: {}", key),
            )),
            Err(e) => Err(StorageError::Io(e)),
        }
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        let lock_file = self.get_path("locks", &format!("{}.lock", key));
        Ok(lock_file.exists())
    }

    async fn force_release(&self, key: &str) -> StorageResult<()> {
        let lock_file = self.get_path("locks", &format!("{}.lock", key));
        if lock_file.exists() {
            fs::remove_file(lock_file).await?;
        }
        Ok(())
    }

    async fn list_locks(&self) -> StorageResult<Vec<StorageLock>> {
        let locks_dir = self.base_dir.join("locks");
        if !locks_dir.exists() {
            return Ok(vec![]);
        }

        let mut locks = Vec::new();
        let mut entries = fs::read_dir(locks_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".lock") {
                    // Add placeholder lock for now
                    locks.push(StorageLock::new(
                        name.trim_end_matches(".lock").to_string(),
                        "file-backend".to_string(),
                        Duration::from_secs(3600),
                    ));
                }
            }
        }

        Ok(locks)
    }
}
