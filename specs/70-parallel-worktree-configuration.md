---
number: 70
title: Parallel Worktree Configuration and Management
category: parallel
priority: high
status: draft
dependencies: []
created: 2025-01-14
---

# Specification 70: Parallel Worktree Configuration and Management

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The whitepaper shows flexible worktree configuration:
```yaml
parallel_worktrees: 5  # Max concurrent operations

tasks:
  - name: "Approach 1: Caching"
    worktree: "approach-caching"
    do:
      - claude: "/optimize-with-caching"
```

Currently, worktree management is implicit and lacks fine-grained control over allocation, naming, and parallel execution strategies.

## Objective

Implement comprehensive worktree configuration and management to enable precise control over parallel execution, worktree allocation strategies, and resource management for optimal performance.

## Requirements

### Functional Requirements
- Global `parallel_worktrees: N` configuration
- Per-task `worktree: "name"` specification
- Worktree pooling and reuse strategies
- Named worktrees for specific experiments
- Automatic worktree cleanup policies
- Worktree health monitoring
- Resource limits per worktree
- Worktree warmup and caching
- Cross-worktree coordination

### Non-Functional Requirements
- Efficient worktree creation and deletion
- Minimal disk space overhead
- Thread-safe worktree allocation
- Clear worktree status visibility

## Acceptance Criteria

- [ ] `parallel_worktrees: 10` limits concurrent worktrees
- [ ] `worktree: "experiment-1"` creates named worktree
- [ ] Worktree pool automatically manages allocation
- [ ] Idle worktrees cleaned up after timeout
- [ ] Worktree status visible in progress output
- [ ] Resource limits enforced per worktree
- [ ] Worktree reuse for similar tasks
- [ ] Clean shutdown releases all worktrees
- [ ] Worktree conflicts detected and prevented
- [ ] Metrics show worktree utilization

## Technical Details

### Implementation Approach

1. **Worktree Configuration**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorktreeConfig {
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

       /// Worktree base directory
       #[serde(default = "default_worktree_dir")]
       pub base_dir: PathBuf,

       /// Enable worktree caching
       #[serde(default)]
       pub enable_cache: bool,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum AllocationStrategy {
       /// Create new worktree for each task
       OnDemand,
       /// Pre-create pool of worktrees
       Pooled { size: usize },
       /// Reuse worktrees when possible
       Reuse,
       /// Dedicated worktrees for named tasks
       Dedicated,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct CleanupPolicy {
       /// Cleanup idle worktrees after timeout
       pub idle_timeout: Duration,
       /// Maximum worktree age
       pub max_age: Duration,
       /// Cleanup on workflow completion
       pub cleanup_on_complete: bool,
       /// Keep failed worktrees for debugging
       pub keep_failed: bool,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ResourceLimits {
       /// Maximum disk space per worktree (MB)
       pub max_disk_mb: Option<usize>,
       /// Maximum memory per worktree (MB)
       pub max_memory_mb: Option<usize>,
       /// Maximum CPU percentage
       pub max_cpu_percent: Option<f32>,
   }
   ```

2. **Worktree Pool Manager**:
   ```rust
   pub struct WorktreePool {
       config: WorktreeConfig,
       available: Arc<RwLock<VecDeque<PooledWorktree>>>,
       in_use: Arc<RwLock<HashMap<String, PooledWorktree>>>,
       named: Arc<RwLock<HashMap<String, PooledWorktree>>>,
       metrics: Arc<WorktreeMetrics>,
       semaphore: Arc<Semaphore>,
   }

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
   }

   #[derive(Debug, Clone)]
   pub enum WorktreeStatus {
       Available,
       InUse { task: String },
       Named { name: String },
       Cleaning,
       Failed { error: String },
   }

   impl WorktreePool {
       pub async fn acquire(
           &self,
           request: WorktreeRequest,
       ) -> Result<WorktreeHandle> {
           // Wait for available slot
           let _permit = self.semaphore.acquire().await?;

           match request {
               WorktreeRequest::Named(name) => {
                   self.acquire_named_worktree(name).await
               }
               WorktreeRequest::Anonymous => {
                   self.acquire_anonymous_worktree().await
               }
               WorktreeRequest::Reusable(criteria) => {
                   self.acquire_reusable_worktree(criteria).await
               }
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
               self.in_use.write().await.insert(
                   worktree.id.clone(),
                   worktree.clone(),
               );

               return Ok(WorktreeHandle::new(worktree, self.clone()));
           }

           // Create new worktree if under limit
           if self.in_use.read().await.len() < self.config.parallel_worktrees {
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
                   worktree.status = WorktreeStatus::Named {
                       name: name.clone(),
                   };
                   worktree.last_used = Instant::now();

                   return Ok(WorktreeHandle::new(worktree, self.clone()));
               } else {
                   return Err(anyhow!("Named worktree '{}' is in use", name));
               }
           }

           // Create new named worktree
           let worktree = self.create_named_worktree(name.clone()).await?;
           named.insert(name, worktree.clone());
           Ok(WorktreeHandle::new(worktree, self.clone()))
       }

       pub async fn cleanup_idle(&self) {
           let mut available = self.available.write().await;
           let now = Instant::now();

           available.retain(|w| {
               let idle_duration = now - w.last_used;
               if idle_duration > self.config.cleanup_policy.idle_timeout {
                   info!("Cleaning up idle worktree: {}", w.id);
                   let _ = self.delete_worktree(&w.path);
                   false
               } else {
                   true
               }
           });
       }

       pub fn get_metrics(&self) -> WorktreeMetrics {
           WorktreeMetrics {
               total: self.config.parallel_worktrees,
               in_use: self.in_use.blocking_read().len(),
               available: self.available.blocking_read().len(),
               named: self.named.blocking_read().len(),
               total_created: self.metrics.total_created.load(Ordering::Relaxed),
               total_reused: self.metrics.total_reused.load(Ordering::Relaxed),
           }
       }
   }
   ```

3. **Worktree Handle with Auto-Release**:
   ```rust
   pub struct WorktreeHandle {
       worktree: PooledWorktree,
       pool: Arc<WorktreePool>,
       released: Arc<AtomicBool>,
   }

   impl WorktreeHandle {
       pub fn path(&self) -> &Path {
           &self.worktree.path
       }

       pub fn branch(&self) -> &str {
           &self.worktree.branch
       }

       pub async fn release(self) {
           if !self.released.swap(true, Ordering::SeqCst) {
               self.pool.release_worktree(self.worktree).await;
           }
       }
   }

   impl Drop for WorktreeHandle {
       fn drop(&mut self) {
           if !self.released.load(Ordering::SeqCst) {
               let pool = self.pool.clone();
               let worktree = self.worktree.clone();
               tokio::spawn(async move {
                   pool.release_worktree(worktree).await;
               });
           }
       }
   }
   ```

### Architecture Changes
- Add `WorktreePool` as central manager
- Replace direct worktree creation with pool acquisition
- Add worktree metrics and monitoring
- Implement cleanup background task
- Integrate with resource monitoring

### Data Structures
```yaml
# Global worktree configuration
worktree_config:
  parallel_worktrees: 20
  allocation_strategy: pooled
  cleanup_policy:
    idle_timeout: 300s
    max_age: 3600s
    cleanup_on_complete: true
    keep_failed: false
  resource_limits:
    max_disk_mb: 1000
    max_memory_mb: 512

# Workflow with worktree configuration
tasks:
  - name: "Experiment A"
    worktree: "experiment-a"  # Named worktree
    commands:
      - claude: "/approach-a"

  - name: "Experiment B"
    worktree: "experiment-b"  # Different named worktree
    commands:
      - claude: "/approach-b"

  - name: "Parallel processing"
    foreach: "find . -name '*.py'"
    parallel: 10  # Will use up to 10 worktrees from pool
    do:
      - claude: "/process ${item}"
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/worktree/manager.rs` - Core worktree management
  - `src/cook/execution/mapreduce.rs` - MapReduce integration
  - `src/config/workflow.rs` - Worktree configuration
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Pool allocation strategies
  - Cleanup policies
  - Resource limit enforcement
  - Metrics collection
- **Integration Tests**:
  - Concurrent worktree allocation
  - Named worktree management
  - Pool exhaustion handling
  - Cleanup during execution
- **Performance Tests**:
  - Worktree creation overhead
  - Pool efficiency
  - Resource usage monitoring
  - Scalability limits

## Documentation Requirements

- **Code Documentation**: Document pool management algorithms
- **User Documentation**:
  - Worktree configuration guide
  - Performance tuning
  - Resource management
  - Troubleshooting guide
- **Architecture Updates**: Add worktree pool to architecture

## Implementation Notes

- Use filesystem locks for worktree safety
- Monitor disk space and prevent exhaustion
- Support worktree templates for faster creation
- Enable worktree snapshot/restore
- Future: Distributed worktree pools

## Migration and Compatibility

- Default configuration maintains current behavior
- Existing workflows work without changes
- Gradual adoption of advanced features
- Clear migration path for worktree management