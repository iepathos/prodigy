//! Result collection strategies for MapReduce operations
//!
//! This module provides different strategies for collecting and managing
//! results from parallel agent execution.

use crate::cook::execution::mapreduce::{AgentResult, AgentStatus};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::VecDeque;

/// Strategy for collecting results
#[derive(Debug, Clone)]
pub enum CollectionStrategy {
    /// Collect all results in memory
    InMemory,
    /// Stream results to disk for large datasets
    Streaming { buffer_size: usize },
    /// Keep only last N results
    Windowed { window_size: usize },
}

/// Result collector for managing agent results
pub struct ResultCollector {
    strategy: CollectionStrategy,
    results: Arc<RwLock<Vec<AgentResult>>>,
    buffer: Arc<RwLock<VecDeque<AgentResult>>>,
}

impl ResultCollector {
    /// Create a new collector with the specified strategy
    pub fn new(strategy: CollectionStrategy) -> Self {
        Self {
            strategy,
            results: Arc::new(RwLock::new(Vec::new())),
            buffer: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Add a result to the collector
    pub async fn add_result(&self, result: AgentResult) {
        match &self.strategy {
            CollectionStrategy::InMemory => {
                let mut results = self.results.write().await;
                results.push(result);
            }
            CollectionStrategy::Streaming { buffer_size } => {
                let mut buffer = self.buffer.write().await;
                buffer.push_back(result);

                if buffer.len() >= *buffer_size {
                    // In a real implementation, we would flush to disk here
                    let mut results = self.results.write().await;
                    while let Some(buffered) = buffer.pop_front() {
                        results.push(buffered);
                    }
                }
            }
            CollectionStrategy::Windowed { window_size } => {
                let mut buffer = self.buffer.write().await;
                buffer.push_back(result);

                while buffer.len() > *window_size {
                    buffer.pop_front();
                }
            }
        }
    }

    /// Get all collected results
    pub async fn get_results(&self) -> Vec<AgentResult> {
        match &self.strategy {
            CollectionStrategy::InMemory | CollectionStrategy::Streaming { .. } => {
                let results = self.results.read().await;
                results.clone()
            }
            CollectionStrategy::Windowed { .. } => {
                let buffer = self.buffer.read().await;
                buffer.iter().cloned().collect()
            }
        }
    }

    /// Get only successful results
    pub async fn get_successful_results(&self) -> Vec<AgentResult> {
        let all_results = self.get_results().await;
        all_results
            .into_iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .collect()
    }

    /// Get only failed results
    pub async fn get_failed_results(&self) -> Vec<AgentResult> {
        let all_results = self.get_results().await;
        all_results
            .into_iter()
            .filter(|r| matches!(r.status, AgentStatus::Failed(_) | AgentStatus::Timeout))
            .collect()
    }

    /// Get count of collected results
    pub async fn count(&self) -> usize {
        match &self.strategy {
            CollectionStrategy::InMemory | CollectionStrategy::Streaming { .. } => {
                let results = self.results.read().await;
                results.len()
            }
            CollectionStrategy::Windowed { .. } => {
                let buffer = self.buffer.read().await;
                buffer.len()
            }
        }
    }

    /// Clear all collected results
    pub async fn clear(&self) {
        let mut results = self.results.write().await;
        results.clear();

        let mut buffer = self.buffer.write().await;
        buffer.clear();
    }

    /// Flush any buffered results
    pub async fn flush(&self) {
        if let CollectionStrategy::Streaming { .. } = &self.strategy {
            let mut buffer = self.buffer.write().await;
            let mut results = self.results.write().await;

            while let Some(buffered) = buffer.pop_front() {
                results.push(buffered);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_result(id: &str, status: AgentStatus) -> AgentResult {
        AgentResult {
            item_id: id.to_string(),
            status,
            output: None,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        }
    }

    #[tokio::test]
    async fn test_in_memory_collection() {
        let collector = ResultCollector::new(CollectionStrategy::InMemory);

        collector.add_result(create_test_result("1", AgentStatus::Success)).await;
        collector.add_result(create_test_result("2", AgentStatus::Failed("error".to_string()))).await;

        let results = collector.get_results().await;
        assert_eq!(results.len(), 2);

        let successful = collector.get_successful_results().await;
        assert_eq!(successful.len(), 1);

        let failed = collector.get_failed_results().await;
        assert_eq!(failed.len(), 1);
    }

    #[tokio::test]
    async fn test_windowed_collection() {
        let collector = ResultCollector::new(CollectionStrategy::Windowed { window_size: 2 });

        collector.add_result(create_test_result("1", AgentStatus::Success)).await;
        collector.add_result(create_test_result("2", AgentStatus::Success)).await;
        collector.add_result(create_test_result("3", AgentStatus::Success)).await;

        let results = collector.get_results().await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].item_id, "2");
        assert_eq!(results[1].item_id, "3");
    }
}