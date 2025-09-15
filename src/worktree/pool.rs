//! Worktree pool management for parallel execution
//!
//! Provides sophisticated worktree pooling with allocation strategies,
//! resource limits, and automatic cleanup policies.

use super::{WorktreeManager, WorktreeSession};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, warn};

/// Configuration for worktree pool management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreePoolConfig {
    /// Maximum parallel worktrees
    #[serde(default = "default_parallel_worktrees")]
    pub parallel_worktrees: usize,

    /// Worktree allocation strategy
    #[serde(default)]
    pub allocation_strategy: AllocationStrategy,

    /// Worktree cleanup policy
    #[serde(default)]
    pub cleanup_policy: CleanupPolicy,

    /// Resource limits per worktree
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_limits: Option<ResourceLimits>,

    /// Enable worktree caching
    #[serde(default)]
    pub enable_cache: bool,
}

impl Default for WorktreePoolConfig {
    fn default() -> Self {
        Self {
            parallel_worktrees: default_parallel_worktrees(),
            allocation_strategy: AllocationStrategy::default(),
            cleanup_policy: CleanupPolicy::default(),
            resource_limits: None,
            enable_cache: false,
        }
    }
}

fn default_parallel_worktrees() -> usize {
    10
}

/// Worktree allocation strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AllocationStrategy {
    /// Create new worktree for each task
    #[default]
    OnDemand,
    /// Pre-create pool of worktrees
    Pooled { size: usize },
    /// Reuse worktrees when possible
    Reuse,
    /// Dedicated worktrees for named tasks
    Dedicated,
}

/// Cleanup policy for worktrees
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupPolicy {
    /// Cleanup idle worktrees after timeout (in seconds)
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,

    /// Maximum worktree age (in seconds)
    #[serde(default = "default_max_age")]
    pub max_age_secs: u64,

    /// Cleanup on workflow completion
    #[serde(default = "default_cleanup_on_complete")]
    pub cleanup_on_complete: bool,

    /// Keep failed worktrees for debugging
    #[serde(default)]
    pub keep_failed: bool,
}

impl Default for CleanupPolicy {
    fn default() -> Self {
        Self {
            idle_timeout_secs: default_idle_timeout(),
            max_age_secs: default_max_age(),
            cleanup_on_complete: default_cleanup_on_complete(),
            keep_failed: false,
        }
    }
}

fn default_idle_timeout() -> u64 {
    300 // 5 minutes
}

fn default_max_age() -> u64 {
    3600 // 1 hour
}

fn default_cleanup_on_complete() -> bool {
    true
}

/// Resource limits for worktrees
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum disk space per worktree (MB)
    pub max_disk_mb: Option<usize>,

    /// Maximum memory per worktree (MB)
    pub max_memory_mb: Option<usize>,

    /// Maximum CPU percentage
    pub max_cpu_percent: Option<f32>,
}

/// Status of a pooled worktree
#[derive(Debug, Clone)]
pub enum WorktreeStatus {
    Available,
    InUse { task: String },
    Named { name: String },
    Cleaning,
    Failed { error: String },
}

/// Resource usage for a worktree
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub disk_mb: usize,
    pub memory_mb: usize,
    pub cpu_percent: f32,
}

/// A pooled worktree instance
#[derive(Debug, Clone)]
pub struct PooledWorktree {
    pub id: String,
    pub path: PathBuf,
    pub branch: String,
    pub created_at: Instant,
    pub last_used: Instant,
    pub use_count: usize,
    pub status: WorktreeStatus,
    pub resource_usage: ResourceUsage,
    pub session: Option<WorktreeSession>,
}

/// Worktree request types
#[derive(Debug, Clone)]
pub enum WorktreeRequest {
    /// Anonymous worktree from pool
    Anonymous,
    /// Named worktree (creates or reuses)
    Named(String),
    /// Reusable worktree matching criteria
    Reusable(ReuseCriteria),
}

/// Criteria for reusing worktrees
#[derive(Debug, Clone)]
pub struct ReuseCriteria {
    pub branch_prefix: Option<String>,
    pub max_age: Option<Duration>,
    pub max_use_count: Option<usize>,
}

/// Metrics for worktree pool
#[derive(Debug, Clone, Default)]
pub struct WorktreeMetrics {
    pub total: usize,
    pub in_use: usize,
    pub available: usize,
    pub named: usize,
    pub total_created: usize,
    pub total_reused: usize,
}

/// Handle to a worktree with automatic release
pub struct WorktreeHandle {
    worktree: PooledWorktree,
    pool: Arc<WorktreePool>,
    released: Arc<AtomicBool>,
}

impl WorktreeHandle {
    fn new(worktree: PooledWorktree, pool: Arc<WorktreePool>) -> Self {
        Self {
            worktree,
            pool,
            released: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.worktree.path
    }

    pub fn branch(&self) -> &str {
        &self.worktree.branch
    }

    pub fn session(&self) -> Option<&WorktreeSession> {
        self.worktree.session.as_ref()
    }

    pub async fn release(self) {
        if !self.released.swap(true, Ordering::SeqCst) {
            self.pool.release_worktree(self.worktree.clone()).await;
        }
    }
}

impl Drop for WorktreeHandle {
    fn drop(&mut self) {
        if !self.released.load(Ordering::SeqCst) {
            let pool = self.pool.clone();
            let worktree = self.worktree.clone();
            let released = self.released.clone();

            tokio::spawn(async move {
                if !released.swap(true, Ordering::SeqCst) {
                    pool.release_worktree(worktree).await;
                }
            });
        }
    }
}

/// Worktree pool manager
pub struct WorktreePool {
    config: WorktreePoolConfig,
    manager: Arc<WorktreeManager>,
    available: Arc<RwLock<VecDeque<PooledWorktree>>>,
    in_use: Arc<RwLock<HashMap<String, PooledWorktree>>>,
    named: Arc<RwLock<HashMap<String, PooledWorktree>>>,
    semaphore: Arc<Semaphore>,
    metrics: WorktreePoolMetrics,
}

struct WorktreePoolMetrics {
    total_created: AtomicUsize,
    total_reused: AtomicUsize,
}

impl WorktreePool {
    /// Create a new worktree pool
    pub fn new(config: WorktreePoolConfig, manager: Arc<WorktreeManager>) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.parallel_worktrees));

        Self {
            config,
            manager,
            available: Arc::new(RwLock::new(VecDeque::new())),
            in_use: Arc::new(RwLock::new(HashMap::new())),
            named: Arc::new(RwLock::new(HashMap::new())),
            semaphore,
            metrics: WorktreePoolMetrics {
                total_created: AtomicUsize::new(0),
                total_reused: AtomicUsize::new(0),
            },
        }
    }

    /// Acquire a worktree from the pool
    pub async fn acquire(&self, request: WorktreeRequest) -> Result<WorktreeHandle> {
        // Wait for available slot
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| anyhow!("Failed to acquire semaphore: {}", e))?;

        match request {
            WorktreeRequest::Named(name) => self.acquire_named_worktree(name).await,
            WorktreeRequest::Anonymous => self.acquire_anonymous_worktree().await,
            WorktreeRequest::Reusable(criteria) => self.acquire_reusable_worktree(criteria).await,
        }
    }

    async fn acquire_anonymous_worktree(&self) -> Result<WorktreeHandle> {
        // Check for available worktree in pool
        let mut available = self.available.write().await;
        if let Some(mut worktree) = available.pop_front() {
            // Update status
            worktree.status = WorktreeStatus::InUse {
                task: "anonymous".to_string(),
            };
            worktree.last_used = Instant::now();
            worktree.use_count += 1;

            // Move to in-use
            self.in_use
                .write()
                .await
                .insert(worktree.id.clone(), worktree.clone());

            self.metrics.total_reused.fetch_add(1, Ordering::Relaxed);
            debug!("Reusing worktree {} from pool", worktree.id);

            return Ok(WorktreeHandle::new(worktree, Arc::new(self.clone())));
        }
        drop(available);

        // Create new worktree if under limit
        let in_use_count = self.in_use.read().await.len();
        if in_use_count < self.config.parallel_worktrees {
            self.create_new_worktree().await
        } else {
            // Wait for one to become available
            self.wait_for_available().await
        }
    }

    async fn acquire_named_worktree(&self, name: String) -> Result<WorktreeHandle> {
        let mut named = self.named.write().await;

        if let Some(worktree) = named.get(&name) {
            if matches!(worktree.status, WorktreeStatus::Available) {
                // Reuse existing named worktree
                let mut worktree = worktree.clone();
                worktree.status = WorktreeStatus::Named { name: name.clone() };
                worktree.last_used = Instant::now();

                self.metrics.total_reused.fetch_add(1, Ordering::Relaxed);
                debug!("Reusing named worktree '{}'", name);

                return Ok(WorktreeHandle::new(worktree, Arc::new(self.clone())));
            } else {
                return Err(anyhow!("Named worktree '{}' is in use", name));
            }
        }

        // Create new named worktree
        let worktree = self.create_named_worktree(name.clone()).await?;
        named.insert(name, worktree.clone());
        Ok(WorktreeHandle::new(worktree, Arc::new(self.clone())))
    }

    async fn acquire_reusable_worktree(&self, criteria: ReuseCriteria) -> Result<WorktreeHandle> {
        let mut available = self.available.write().await;

        // Find a suitable worktree matching criteria
        let position = available.iter().position(|w| {
            let age_ok = criteria
                .max_age
                .map(|max_age| w.created_at.elapsed() <= max_age)
                .unwrap_or(true);

            let use_count_ok = criteria
                .max_use_count
                .map(|max_count| w.use_count <= max_count)
                .unwrap_or(true);

            let branch_ok = criteria
                .branch_prefix
                .as_ref()
                .map(|prefix| w.branch.starts_with(prefix))
                .unwrap_or(true);

            age_ok && use_count_ok && branch_ok
        });

        if let Some(pos) = position {
            let mut worktree = available.remove(pos).unwrap();
            worktree.status = WorktreeStatus::InUse {
                task: "reusable".to_string(),
            };
            worktree.last_used = Instant::now();
            worktree.use_count += 1;

            self.in_use
                .write()
                .await
                .insert(worktree.id.clone(), worktree.clone());

            self.metrics.total_reused.fetch_add(1, Ordering::Relaxed);
            return Ok(WorktreeHandle::new(worktree, Arc::new(self.clone())));
        }

        // No suitable worktree found, create new one
        drop(available);
        self.create_new_worktree().await
    }

    async fn create_new_worktree(&self) -> Result<WorktreeHandle> {
        // Check resource limits before creating
        if let Some(limits) = &self.config.resource_limits {
            self.check_resource_limits(limits).await?;
        }

        let session = self.manager.create_session().await?;
        let id = uuid::Uuid::new_v4().to_string();

        // Monitor initial resource usage
        let resource_usage = self.measure_resource_usage(&session.path).await;

        let worktree = PooledWorktree {
            id: id.clone(),
            path: session.path.clone(),
            branch: session.branch.clone(),
            created_at: Instant::now(),
            last_used: Instant::now(),
            use_count: 1,
            status: WorktreeStatus::InUse {
                task: "new".to_string(),
            },
            resource_usage,
            session: Some(session),
        };

        self.in_use
            .write()
            .await
            .insert(id.clone(), worktree.clone());
        self.metrics.total_created.fetch_add(1, Ordering::Relaxed);

        info!("Created new worktree {}", id);
        Ok(WorktreeHandle::new(worktree, Arc::new(self.clone())))
    }

    async fn create_named_worktree(&self, name: String) -> Result<PooledWorktree> {
        let session = self.manager.create_session().await?;
        let id = format!("named-{}", name);

        let worktree = PooledWorktree {
            id: id.clone(),
            path: session.path.clone(),
            branch: session.branch.clone(),
            created_at: Instant::now(),
            last_used: Instant::now(),
            use_count: 1,
            status: WorktreeStatus::Named { name },
            resource_usage: ResourceUsage::default(),
            session: Some(session),
        };

        self.metrics.total_created.fetch_add(1, Ordering::Relaxed);
        info!("Created named worktree {}", id);

        Ok(worktree)
    }

    async fn wait_for_available(&self) -> Result<WorktreeHandle> {
        // Simple wait loop - in production this should use a condition variable
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            let mut available = self.available.write().await;
            if let Some(mut worktree) = available.pop_front() {
                worktree.status = WorktreeStatus::InUse {
                    task: "waited".to_string(),
                };
                worktree.last_used = Instant::now();
                worktree.use_count += 1;

                self.in_use
                    .write()
                    .await
                    .insert(worktree.id.clone(), worktree.clone());

                return Ok(WorktreeHandle::new(worktree, Arc::new(self.clone())));
            }
        }
    }

    /// Release a worktree back to the pool
    pub async fn release_worktree(&self, mut worktree: PooledWorktree) {
        // Check if worktree should be cleaned up
        let should_cleanup = match &self.config.cleanup_policy {
            policy if policy.keep_failed => {
                matches!(worktree.status, WorktreeStatus::Failed { .. })
            }
            _ => false,
        };

        if should_cleanup {
            self.cleanup_worktree(&worktree).await;
            return;
        }

        // Update status and return to available pool
        worktree.status = WorktreeStatus::Available;

        // Remove from in-use
        self.in_use.write().await.remove(&worktree.id);

        // Add to available pool if reusable
        if self.config.enable_cache {
            self.available.write().await.push_back(worktree);
        } else {
            // Clean up immediately if caching disabled
            self.cleanup_worktree(&worktree).await;
        }
    }

    async fn cleanup_worktree(&self, worktree: &PooledWorktree) {
        if let Some(session) = &worktree.session {
            if let Err(e) = self.manager.cleanup_session(&session.name, false).await {
                warn!("Failed to cleanup worktree {}: {}", worktree.id, e);
            }
        }
    }

    /// Clean up idle worktrees
    pub async fn cleanup_idle(&self) {
        let mut available = self.available.write().await;
        let now = Instant::now();
        let idle_timeout = Duration::from_secs(self.config.cleanup_policy.idle_timeout_secs);

        available.retain(|w| {
            let idle_duration = now - w.last_used;
            if idle_duration > idle_timeout {
                info!("Cleaning up idle worktree: {}", w.id);
                false
            } else {
                true
            }
        });
    }

    /// Get pool metrics
    pub async fn get_metrics(&self) -> WorktreeMetrics {
        WorktreeMetrics {
            total: self.config.parallel_worktrees,
            in_use: self.in_use.read().await.len(),
            available: self.available.read().await.len(),
            named: self.named.read().await.len(),
            total_created: self.metrics.total_created.load(Ordering::Relaxed),
            total_reused: self.metrics.total_reused.load(Ordering::Relaxed),
        }
    }

    /// Clean up all worktrees
    pub async fn cleanup_all(&self) {
        // Clean up in-use worktrees
        let in_use = self.in_use.write().await;
        for worktree in in_use.values() {
            self.cleanup_worktree(worktree).await;
        }
        drop(in_use);

        // Clean up available worktrees
        let available = self.available.write().await;
        for worktree in available.iter() {
            self.cleanup_worktree(worktree).await;
        }
        drop(available);

        // Clean up named worktrees
        let named = self.named.write().await;
        for worktree in named.values() {
            self.cleanup_worktree(worktree).await;
        }
    }

    /// Check if resource limits would be exceeded
    async fn check_resource_limits(&self, limits: &ResourceLimits) -> Result<()> {
        // Get current total resource usage
        let in_use = self.in_use.read().await;
        let mut total_disk = 0;
        let mut total_memory = 0;
        let mut total_cpu = 0.0;

        for worktree in in_use.values() {
            total_disk += worktree.resource_usage.disk_mb;
            total_memory += worktree.resource_usage.memory_mb;
            total_cpu += worktree.resource_usage.cpu_percent;
        }

        // Check against limits
        if let Some(max_disk) = limits.max_disk_mb {
            if total_disk >= max_disk {
                return Err(anyhow!(
                    "Disk usage limit exceeded: {} MB / {} MB",
                    total_disk,
                    max_disk
                ));
            }
        }

        if let Some(max_memory) = limits.max_memory_mb {
            if total_memory >= max_memory {
                return Err(anyhow!(
                    "Memory usage limit exceeded: {} MB / {} MB",
                    total_memory,
                    max_memory
                ));
            }
        }

        if let Some(max_cpu) = limits.max_cpu_percent {
            if total_cpu >= max_cpu {
                return Err(anyhow!(
                    "CPU usage limit exceeded: {:.1}% / {:.1}%",
                    total_cpu,
                    max_cpu
                ));
            }
        }

        Ok(())
    }

    /// Measure resource usage for a worktree path
    async fn measure_resource_usage(&self, path: &Path) -> ResourceUsage {
        // Simple implementation - measure disk usage
        let disk_mb = self.measure_disk_usage(path).await.unwrap_or(0);

        ResourceUsage {
            disk_mb,
            memory_mb: 0,     // Would need process monitoring for accurate memory
            cpu_percent: 0.0, // Would need process monitoring for accurate CPU
        }
    }

    /// Measure disk usage of a directory in MB
    async fn measure_disk_usage(&self, path: &Path) -> Result<usize> {
        use tokio::process::Command;

        let output = Command::new("du")
            .args(["-sm", path.to_str().unwrap_or(".")])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(0);
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let size_mb = output_str
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        Ok(size_mb)
    }
}

// Clone implementation for WorktreePool
impl Clone for WorktreePool {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            manager: self.manager.clone(),
            available: self.available.clone(),
            in_use: self.in_use.clone(),
            named: self.named.clone(),
            semaphore: self.semaphore.clone(),
            metrics: WorktreePoolMetrics {
                total_created: AtomicUsize::new(self.metrics.total_created.load(Ordering::Relaxed)),
                total_reused: AtomicUsize::new(self.metrics.total_reused.load(Ordering::Relaxed)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::SubprocessManager;
    use tempfile::TempDir;
    use tokio::process::Command;

    #[tokio::test]
    async fn test_pool_basic_allocation() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();

        // Initialize git repository for testing
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();

        // Create initial commit
        std::fs::write(temp_dir.path().join("README.md"), "test").unwrap();
        Command::new("git")
            .args(&["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();

        let manager =
            Arc::new(WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap());

        let config = WorktreePoolConfig {
            parallel_worktrees: 2,
            ..Default::default()
        };

        let pool = WorktreePool::new(config, manager);

        // Should be able to acquire two worktrees
        let handle1 = pool.acquire(WorktreeRequest::Anonymous).await.unwrap();
        let handle2 = pool.acquire(WorktreeRequest::Anonymous).await.unwrap();

        assert_ne!(handle1.worktree.id, handle2.worktree.id);

        // Release handles
        handle1.release().await;
        handle2.release().await;
    }

    #[tokio::test]
    async fn test_named_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();

        // Initialize git repository for testing
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();

        // Create initial commit
        std::fs::write(temp_dir.path().join("README.md"), "test").unwrap();
        Command::new("git")
            .args(&["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();

        let manager =
            Arc::new(WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap());

        let config = WorktreePoolConfig::default();
        let pool = WorktreePool::new(config, manager);

        // Create named worktree
        let handle = pool
            .acquire(WorktreeRequest::Named("test".to_string()))
            .await
            .unwrap();
        assert!(handle.worktree.id.contains("test"));

        handle.release().await;
    }

    #[tokio::test]
    async fn test_metrics() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();

        // Initialize git repository for testing
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();

        // Create initial commit
        std::fs::write(temp_dir.path().join("README.md"), "test").unwrap();
        Command::new("git")
            .args(&["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();

        let manager =
            Arc::new(WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap());

        let config = WorktreePoolConfig::default();
        let pool = WorktreePool::new(config, manager);

        let handle = pool.acquire(WorktreeRequest::Anonymous).await.unwrap();

        let metrics = pool.get_metrics().await;
        assert_eq!(metrics.in_use, 1);
        assert_eq!(metrics.total_created, 1);

        handle.release().await;
    }
}
