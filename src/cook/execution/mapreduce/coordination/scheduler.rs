//! Work scheduling for MapReduce operations
//!
//! This module provides scheduling strategies for distributing
//! work items across parallel agents.

use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::debug;

/// Strategy for scheduling work items
#[derive(Debug, Clone)]
pub enum SchedulingStrategy {
    /// Round-robin distribution
    RoundRobin,
    /// First-come, first-served queue
    FIFO,
    /// Last-in, first-out stack
    LIFO,
    /// Priority-based scheduling
    Priority { field: String },
    /// Batch scheduling with fixed size
    Batched { batch_size: usize },
}

/// Work item with metadata
#[derive(Debug, Clone)]
pub struct WorkItem {
    /// Index of the item
    pub index: usize,
    /// The actual data
    pub data: Value,
    /// Priority for scheduling
    pub priority: i32,
}

/// Work scheduler for distributing items to agents
pub struct WorkScheduler {
    strategy: SchedulingStrategy,
    work_queue: Arc<RwLock<VecDeque<WorkItem>>>,
    total_items: usize,
    processed_count: Arc<RwLock<usize>>,
}

impl WorkScheduler {
    /// Create a new scheduler with the specified strategy
    pub fn new(strategy: SchedulingStrategy, items: Vec<Value>) -> Self {
        let total_items = items.len();
        let work_items: VecDeque<WorkItem> = items
            .into_iter()
            .enumerate()
            .map(|(index, data)| WorkItem {
                index,
                data,
                priority: 0,
            })
            .collect();

        Self {
            strategy,
            work_queue: Arc::new(RwLock::new(work_items)),
            total_items,
            processed_count: Arc::new(RwLock::new(0)),
        }
    }

    /// Get the next work item according to the scheduling strategy
    pub async fn next_item(&self) -> Option<WorkItem> {
        let mut queue = self.work_queue.write().await;

        let item = match &self.strategy {
            SchedulingStrategy::FIFO | SchedulingStrategy::RoundRobin => queue.pop_front(),
            SchedulingStrategy::LIFO => queue.pop_back(),
            SchedulingStrategy::Priority { .. } => {
                // For priority scheduling, find highest priority item
                if queue.is_empty() {
                    None
                } else {
                    let max_idx = queue
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, item)| item.priority)
                        .map(|(idx, _)| idx)?;

                    queue.remove(max_idx)
                }
            }
            SchedulingStrategy::Batched { .. } => queue.pop_front(),
        };

        if item.is_some() {
            let mut count = self.processed_count.write().await;
            *count += 1;
            debug!("Scheduled item {}/{}", *count, self.total_items);
        }

        item
    }

    /// Get a batch of work items
    pub async fn next_batch(&self, batch_size: usize) -> Vec<WorkItem> {
        let mut queue = self.work_queue.write().await;
        let mut batch = Vec::with_capacity(batch_size);

        for _ in 0..batch_size {
            if let Some(item) = queue.pop_front() {
                batch.push(item);
            } else {
                break;
            }
        }

        if !batch.is_empty() {
            let mut count = self.processed_count.write().await;
            *count += batch.len();
            debug!(
                "Scheduled batch of {} items ({}/{})",
                batch.len(),
                *count,
                self.total_items
            );
        }

        batch
    }

    /// Get remaining item count
    pub async fn remaining_count(&self) -> usize {
        let queue = self.work_queue.read().await;
        queue.len()
    }

    /// Get processed item count
    pub async fn processed_count(&self) -> usize {
        let count = self.processed_count.read().await;
        *count
    }

    /// Check if all items have been scheduled
    pub async fn is_complete(&self) -> bool {
        let queue = self.work_queue.read().await;
        queue.is_empty()
    }

    /// Create a channel-based scheduler for async distribution
    pub async fn create_channel_scheduler(
        self,
        buffer_size: usize,
    ) -> (mpsc::Sender<WorkItem>, mpsc::Receiver<WorkItem>) {
        let (tx, rx) = mpsc::channel(buffer_size);
        let scheduler = Arc::new(self);

        // Spawn task to feed items into channel
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            while let Some(item) = scheduler.next_item().await {
                if tx_clone.send(item).await.is_err() {
                    break;
                }
            }
        });

        (tx, rx)
    }

    /// Reset the scheduler with new items
    pub async fn reset(&self, items: Vec<Value>) {
        let work_items: VecDeque<WorkItem> = items
            .into_iter()
            .enumerate()
            .map(|(index, data)| WorkItem {
                index,
                data,
                priority: 0,
            })
            .collect();

        let mut queue = self.work_queue.write().await;
        *queue = work_items;

        let mut count = self.processed_count.write().await;
        *count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_fifo_scheduling() {
        let items = vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})];

        let scheduler = WorkScheduler::new(SchedulingStrategy::FIFO, items);

        let item1 = scheduler.next_item().await.unwrap();
        assert_eq!(item1.index, 0);

        let item2 = scheduler.next_item().await.unwrap();
        assert_eq!(item2.index, 1);

        let item3 = scheduler.next_item().await.unwrap();
        assert_eq!(item3.index, 2);

        assert!(scheduler.next_item().await.is_none());
    }

    #[tokio::test]
    async fn test_lifo_scheduling() {
        let items = vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})];

        let scheduler = WorkScheduler::new(SchedulingStrategy::LIFO, items);

        let item1 = scheduler.next_item().await.unwrap();
        assert_eq!(item1.index, 2);

        let item2 = scheduler.next_item().await.unwrap();
        assert_eq!(item2.index, 1);

        let item3 = scheduler.next_item().await.unwrap();
        assert_eq!(item3.index, 0);
    }

    #[tokio::test]
    async fn test_batch_scheduling() {
        let items = vec![
            json!({"id": 1}),
            json!({"id": 2}),
            json!({"id": 3}),
            json!({"id": 4}),
        ];

        let scheduler = WorkScheduler::new(SchedulingStrategy::Batched { batch_size: 2 }, items);

        let batch1 = scheduler.next_batch(2).await;
        assert_eq!(batch1.len(), 2);
        assert_eq!(batch1[0].index, 0);
        assert_eq!(batch1[1].index, 1);

        let batch2 = scheduler.next_batch(2).await;
        assert_eq!(batch2.len(), 2);

        let batch3 = scheduler.next_batch(2).await;
        assert_eq!(batch3.len(), 0);
    }
}
