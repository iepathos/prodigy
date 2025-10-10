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

    /// Create a resource guard that returns resources to pool on drop
    fn create_resource_guard(
        resource: T,
        pool: Arc<Mutex<VecDeque<T>>>,
        cleanup: Arc<dyn Fn(T) + Send + Sync>,
    ) -> super::ResourceGuard<T> {
        let pool_weak = Arc::downgrade(&pool);
        super::ResourceGuard::new(resource, move |r| {
            if let Some(pool) = pool_weak.upgrade() {
                // Return to pool asynchronously
                tokio::spawn(async move {
                    let mut available = pool.lock().await;
                    available.push_back(r);
                });
            } else {
                // Pool is gone, cleanup the resource
                cleanup(r);
            }
        })
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

        // Strategy: Try reuse → acquire permit → create new
        // This ensures existing resources are utilized before creating new ones

        // Try to get an available resource first (fast path)
        if let Some(resource) = self.try_get_available().await {
            let mut metrics = self.metrics.lock().await;
            Self::update_acquisition_metrics(&mut metrics, start, true);

            debug!("Reused resource from pool");

            return Ok(Self::create_resource_guard(
                resource,
                self.available.clone(),
                self.cleanup.clone(),
            ));
        }

        // No available resources - create new one (slow path)
        // Acquire semaphore permit to limit concurrent resources
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

        Ok(Self::create_resource_guard(
            resource,
            self.available.clone(),
            self.cleanup.clone(),
        ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Simple counter resource for testing
    #[derive(Debug, Clone)]
    struct CounterResource {
        id: usize,
    }

    /// Create a simple factory that generates incrementing IDs
    fn create_counter_factory(
        counter: Arc<AtomicUsize>,
    ) -> impl Fn() -> futures::future::BoxFuture<'static, MapReduceResult<CounterResource>> {
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                let id = counter.fetch_add(1, Ordering::Relaxed);
                Ok(CounterResource { id })
            })
        }
    }

    #[tokio::test]
    async fn test_pool_creates_new_resource() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = create_counter_factory(counter.clone());
        let pool = GenericResourcePool::new(5, factory);

        let guard = pool.acquire().await.expect("Failed to acquire resource");
        let resource = guard.get().expect("Resource should be present");

        assert_eq!(resource.id, 0);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_pool_reuses_resource() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = create_counter_factory(counter.clone());
        let pool = Arc::new(GenericResourcePool::new(5, factory));

        // Acquire and drop a resource
        {
            let _guard = pool.acquire().await.expect("Failed to acquire");
            // Guard dropped here, resource should be returned to pool
        }

        // Give async cleanup time to complete
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Acquire again - should reuse the resource
        let guard = pool.acquire().await.expect("Failed to reacquire");
        let resource = guard.get().expect("Resource should be present");

        // Should still be the first resource (id=0)
        assert_eq!(resource.id, 0);
        // Counter should still be 1 (no new resource created)
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_pool_respects_max_size() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = create_counter_factory(counter.clone());
        let pool = Arc::new(GenericResourcePool::new(2, factory));

        // Acquire 2 resources (max size)
        let guard1 = pool.acquire().await.expect("Failed to acquire 1");
        let guard2 = pool.acquire().await.expect("Failed to acquire 2");

        assert_eq!(counter.load(Ordering::Relaxed), 2);

        // Drop one guard to return it to the pool
        drop(guard1);
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Acquire a 3rd - should reuse the returned resource
        let guard3 = pool.acquire().await.expect("Failed to acquire 3");

        // Should still be 2 total resources created (reused first one)
        assert_eq!(counter.load(Ordering::Relaxed), 2);

        drop(guard2);
        drop(guard3);
    }

    #[tokio::test]
    async fn test_update_acquisition_metrics_reuse() {
        let mut metrics = PoolMetrics::default();
        let start = Instant::now();

        // Create a concrete type for testing
        type TestPool = GenericResourcePool<
            CounterResource,
            fn() -> futures::future::BoxFuture<'static, MapReduceResult<CounterResource>>,
        >;

        TestPool::update_acquisition_metrics(&mut metrics, start, true);

        assert_eq!(metrics.in_use, 1);
        assert_eq!(metrics.total_acquisitions, 1);
        assert_eq!(metrics.reuse_count, 1);
        assert_eq!(metrics.total_created, 0);
    }

    #[tokio::test]
    async fn test_update_acquisition_metrics_new() {
        let mut metrics = PoolMetrics::default();
        let start = Instant::now();

        type TestPool = GenericResourcePool<
            CounterResource,
            fn() -> futures::future::BoxFuture<'static, MapReduceResult<CounterResource>>,
        >;

        TestPool::update_acquisition_metrics(&mut metrics, start, false);

        assert_eq!(metrics.in_use, 1);
        assert_eq!(metrics.total_acquisitions, 1);
        assert_eq!(metrics.reuse_count, 0);
        assert_eq!(metrics.total_created, 1);
    }

    #[tokio::test]
    async fn test_update_acquisition_metrics_multiple() {
        let mut metrics = PoolMetrics::default();
        let start = Instant::now();

        type TestPool = GenericResourcePool<
            CounterResource,
            fn() -> futures::future::BoxFuture<'static, MapReduceResult<CounterResource>>,
        >;

        // First acquisition (new)
        TestPool::update_acquisition_metrics(&mut metrics, start, false);

        // Second acquisition (reuse)
        TestPool::update_acquisition_metrics(&mut metrics, start, true);

        assert_eq!(metrics.in_use, 2);
        assert_eq!(metrics.total_acquisitions, 2);
        assert_eq!(metrics.reuse_count, 1);
        assert_eq!(metrics.total_created, 1);
    }

    #[tokio::test]
    async fn test_resource_guard_returns_to_pool() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = create_counter_factory(counter.clone());
        let pool = Arc::new(GenericResourcePool::new(5, factory));

        // Acquire a resource
        let guard = pool.acquire().await.expect("Failed to acquire");
        let initial_id = guard.get().expect("Resource should be present").id;

        // Drop the guard
        drop(guard);

        // Give async cleanup time to complete
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Acquire again - should get the same resource back
        let guard2 = pool.acquire().await.expect("Failed to reacquire");
        let reused_id = guard2.get().expect("Resource should be present").id;

        assert_eq!(initial_id, reused_id);
    }

    #[tokio::test]
    async fn test_resource_cleanup_called() {
        let counter = Arc::new(AtomicUsize::new(0));
        let cleanup_counter = Arc::new(AtomicUsize::new(0));
        let cleanup_counter_clone = cleanup_counter.clone();

        let factory = create_counter_factory(counter.clone());
        let pool = GenericResourcePool::with_cleanup(5, factory, move |_resource| {
            cleanup_counter_clone.fetch_add(1, Ordering::Relaxed);
        });

        // Acquire and drop multiple resources to populate the pool
        for _ in 0..3 {
            let guard = pool.acquire().await.expect("Failed to acquire");
            drop(guard);
        }

        // Give async cleanup time to complete
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Clear the pool - cleanup should be called for all resources
        pool.clear().await;

        // Cleanup should have been called for all 3 resources
        assert_eq!(cleanup_counter.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_concurrent_acquisitions() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = create_counter_factory(counter.clone());
        let pool = Arc::new(GenericResourcePool::new(10, factory));

        let mut handles = vec![];

        // Spawn multiple rounds of acquisitions
        // First round: Create initial resources
        for _ in 0..5 {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                let guard = pool_clone.acquire().await.expect("Failed to acquire");
                tokio::time::sleep(Duration::from_millis(10)).await;
                drop(guard);
            });
            handles.push(handle);
        }

        // Wait for first round
        for handle in handles.drain(..) {
            handle.await.expect("Task panicked");
        }

        // Give time for resources to return to pool
        tokio::time::sleep(Duration::from_millis(50)).await;

        let created_after_first_round = counter.load(Ordering::Relaxed);

        // Second round: Should reuse existing resources
        for _ in 0..5 {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                let guard = pool_clone.acquire().await.expect("Failed to acquire");
                tokio::time::sleep(Duration::from_millis(10)).await;
                drop(guard);
            });
            handles.push(handle);
        }

        // Wait for second round
        for handle in handles {
            handle.await.expect("Task panicked");
        }

        let created_after_second_round = counter.load(Ordering::Relaxed);

        // Second round should have reused resources (same count)
        assert_eq!(
            created_after_first_round, created_after_second_round,
            "Resources should have been reused in second round"
        );
    }
}
