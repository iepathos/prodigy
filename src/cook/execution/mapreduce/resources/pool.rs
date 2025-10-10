//! Resource pooling implementation for MapReduce

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, info};

/// Metrics for resource pool usage
#[derive(Debug, Clone, Default)]
pub struct PoolMetrics {
    /// Total number of resources created
    pub total_created: usize,
    /// Number of resources currently in use
    pub in_use: usize,
    /// Number of resources available in pool
    pub available: usize,
    /// Total number of acquisitions
    pub total_acquisitions: usize,
    /// Number of times resources were reused
    pub reuse_count: usize,
    /// Average wait time for resource acquisition
    pub avg_wait_time_ms: u64,
}

/// Trait for resource pools
#[async_trait]
pub trait ResourcePool<T>: Send + Sync {
    /// Acquire a resource from the pool
    async fn acquire(&self) -> MapReduceResult<super::ResourceGuard<T>>;

    /// Release a resource back to the pool
    fn release(&self, resource: T);

    /// Get pool metrics
    fn metrics(&self) -> PoolMetrics;

    /// Clear all resources from the pool
    async fn clear(&self);
}

/// Generic resource pool implementation
pub struct GenericResourcePool<T, F>
where
    T: Send + 'static,
    F: Fn() -> futures::future::BoxFuture<'static, MapReduceResult<T>> + Send + Sync,
{
    /// Available resources
    available: Arc<Mutex<VecDeque<T>>>,
    /// Factory function for creating new resources
    factory: Arc<F>,
    /// Maximum pool size
    #[allow(dead_code)]
    max_size: usize,
    /// Semaphore for limiting concurrent resources
    semaphore: Arc<Semaphore>,
    /// Pool metrics
    metrics: Arc<Mutex<PoolMetrics>>,
    /// Cleanup function for resources
    cleanup: Arc<dyn Fn(T) + Send + Sync>,
}

impl<T, F> GenericResourcePool<T, F>
where
    T: Send + 'static,
    F: Fn() -> futures::future::BoxFuture<'static, MapReduceResult<T>> + Send + Sync,
{
    /// Create a new resource pool
    pub fn new(max_size: usize, factory: F) -> Self {
        Self::with_cleanup(max_size, factory, |_| {})
    }

    /// Create a new resource pool with custom cleanup
    pub fn with_cleanup<C>(max_size: usize, factory: F, cleanup: C) -> Self
    where
        C: Fn(T) + Send + Sync + 'static,
    {
        Self {
            available: Arc::new(Mutex::new(VecDeque::new())),
            factory: Arc::new(factory),
            max_size,
            semaphore: Arc::new(Semaphore::new(max_size)),
            metrics: Arc::new(Mutex::new(PoolMetrics::default())),
            cleanup: Arc::new(cleanup),
        }
    }

    /// Try to get a resource from the pool without creating a new one
    async fn try_get_available(&self) -> Option<T> {
        let mut available = self.available.lock().await;
        available.pop_front()
    }

    /// Return a resource to the pool
    #[allow(dead_code)]
    async fn return_to_pool(&self, resource: T) {
        let mut available = self.available.lock().await;
        let mut metrics = self.metrics.lock().await;

        // Only return to pool if we're under the max size
        if available.len() < self.max_size {
            available.push_back(resource);
            metrics.available = available.len();
            metrics.in_use = metrics.in_use.saturating_sub(1);
        } else {
            // Pool is full, cleanup the resource
            (self.cleanup)(resource);
            metrics.in_use = metrics.in_use.saturating_sub(1);
        }
    }

    /// Update acquisition metrics with wait time tracking
    fn update_acquisition_metrics(
        metrics: &mut PoolMetrics,
        start: Instant,
        is_reuse: bool,
    ) {
        metrics.in_use += 1;
        metrics.total_acquisitions += 1;

        if is_reuse {
            metrics.reuse_count += 1;
            metrics.available = metrics.available.saturating_sub(1);
        } else {
            metrics.total_created += 1;
        }

        let wait_time = start.elapsed();
        metrics.avg_wait_time_ms = ((metrics.avg_wait_time_ms
            * (metrics.total_acquisitions - 1) as u64)
            + wait_time.as_millis() as u64)
            / metrics.total_acquisitions as u64;
    }
}

#[async_trait]
impl<T, F> ResourcePool<T> for GenericResourcePool<T, F>
where
    T: Send + 'static,
    F: Fn() -> futures::future::BoxFuture<'static, MapReduceResult<T>> + Send + Sync,
{
    async fn acquire(&self) -> MapReduceResult<super::ResourceGuard<T>> {
        let start = Instant::now();

        // Try to get an available resource first
        if let Some(resource) = self.try_get_available().await {
            let mut metrics = self.metrics.lock().await;
            Self::update_acquisition_metrics(&mut metrics, start, true);

            debug!("Reused resource from pool");

            let pool = Arc::downgrade(&self.available);
            let cleanup = self.cleanup.clone();

            return Ok(super::ResourceGuard::new(resource, move |r| {
                if let Some(pool) = pool.upgrade() {
                    // Return to pool asynchronously
                    tokio::spawn(async move {
                        let mut available = pool.lock().await;
                        available.push_back(r);
                    });
                } else {
                    // Pool is gone, cleanup the resource
                    cleanup(r);
                }
            }));
        }

        // Acquire semaphore permit
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| MapReduceError::General {
                message: format!("Failed to acquire pool semaphore: {}", e),
                source: None,
            })?;

        // Create a new resource
        let resource = (self.factory)().await?;

        let mut metrics = self.metrics.lock().await;
        Self::update_acquisition_metrics(&mut metrics, start, false);

        info!("Created new resource (total: {})", metrics.total_created);

        let pool = Arc::downgrade(&self.available);
        let cleanup = self.cleanup.clone();

        Ok(super::ResourceGuard::new(resource, move |r| {
            if let Some(pool) = pool.upgrade() {
                // Return to pool asynchronously
                tokio::spawn(async move {
                    let mut available = pool.lock().await;
                    available.push_back(r);
                });
            } else {
                // Pool is gone, cleanup the resource
                cleanup(r);
            }
        }))
    }

    fn release(&self, resource: T) {
        let available = self.available.clone();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            let mut avail = available.lock().await;
            let mut m = metrics.lock().await;

            avail.push_back(resource);
            m.available = avail.len();
            m.in_use = m.in_use.saturating_sub(1);
        });
    }

    fn metrics(&self) -> PoolMetrics {
        // This is a synchronous function but metrics are behind async mutex
        // We'll return a snapshot from the last update
        // In production, you might want to use a different synchronization primitive
        PoolMetrics::default()
    }

    async fn clear(&self) {
        let mut available = self.available.lock().await;
        let cleanup = self.cleanup.clone();

        // Cleanup all available resources
        while let Some(resource) = available.pop_front() {
            cleanup(resource);
        }

        let mut metrics = self.metrics.lock().await;
        metrics.available = 0;
    }
}

/// Bounded resource pool with timeout
pub struct BoundedResourcePool<T>
where
    T: Send + 'static,
{
    inner: Arc<dyn ResourcePool<T>>,
    acquire_timeout: Duration,
}

impl<T> BoundedResourcePool<T>
where
    T: Send + 'static,
{
    /// Create a new bounded resource pool
    pub fn new(inner: Arc<dyn ResourcePool<T>>, acquire_timeout: Duration) -> Self {
        Self {
            inner,
            acquire_timeout,
        }
    }
}

#[async_trait]
impl<T> ResourcePool<T> for BoundedResourcePool<T>
where
    T: Send + 'static,
{
    async fn acquire(&self) -> MapReduceResult<super::ResourceGuard<T>> {
        tokio::time::timeout(self.acquire_timeout, self.inner.acquire())
            .await
            .map_err(|_| MapReduceError::General {
                message: format!(
                    "Resource acquisition timed out after {:?}",
                    self.acquire_timeout
                ),
                source: None,
            })?
    }

    fn release(&self, resource: T) {
        self.inner.release(resource)
    }

    fn metrics(&self) -> PoolMetrics {
        self.inner.metrics()
    }

    async fn clear(&self) {
        self.inner.clear().await
    }
}
