//! Core progress tracking logic for MapReduce execution

use super::operations::AgentOperation;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

/// Progress tracking for parallel execution
pub struct ProgressTracker {
    #[allow(dead_code)]
    multi_progress: MultiProgress,
    overall_bar: ProgressBar,
    agent_bars: Vec<ProgressBar>,
    tick_handle: Option<JoinHandle<()>>,
    is_finished: Arc<AtomicBool>,
    #[allow(dead_code)]
    start_time: Instant,
    agent_operations: Arc<RwLock<Vec<AgentOperation>>>,
}

impl ProgressTracker {
    /// Create a new progress tracker
    pub fn new(total_items: usize, max_parallel: usize) -> Self {
        let multi_progress = MultiProgress::new();

        // Overall progress bar
        let overall_bar = multi_progress.add(ProgressBar::new(total_items as u64));
        overall_bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}",
                )
                .unwrap()
                .progress_chars("##-"),
        );
        overall_bar.set_message("Processing items...");

        // Enable steady tick for timer updates
        overall_bar.enable_steady_tick(Duration::from_millis(100));

        // Individual agent progress bars
        let mut agent_bars = Vec::new();
        let mut agent_operations = Vec::new();
        for i in 0..max_parallel.min(total_items) {
            let bar = multi_progress.add(ProgressBar::new(100));
            bar.set_style(
                ProgressStyle::default_bar()
                    .template(&format!("  Agent {:2}: {{msg}}", i + 1))
                    .unwrap(),
            );
            bar.set_message("Idle");
            agent_bars.push(bar);
            agent_operations.push(AgentOperation::Idle);
        }

        Self {
            multi_progress,
            overall_bar,
            agent_bars,
            tick_handle: None,
            is_finished: Arc::new(AtomicBool::new(false)),
            start_time: Instant::now(),
            agent_operations: Arc::new(RwLock::new(agent_operations)),
        }
    }

    /// Update an agent's display message
    pub fn update_agent(&self, agent_index: usize, message: &str) {
        if agent_index < self.agent_bars.len() {
            self.agent_bars[agent_index].set_message(message.to_string());
        }
    }

    /// Update an agent's operation status
    pub async fn update_agent_operation(&self, agent_index: usize, operation: AgentOperation) {
        let mut ops = self.agent_operations.write().await;
        if agent_index < ops.len() {
            ops[agent_index] = operation.clone();

            // Format the operation for display
            let message = format_operation(&operation);
            self.update_agent(agent_index, &message);
        }
    }

    /// Mark an item as complete
    pub fn complete_item(&self) {
        self.overall_bar.inc(1);
    }

    /// Finish progress tracking with a message
    pub fn finish(&self, message: &str) {
        self.is_finished.store(true, Ordering::Relaxed);
        self.overall_bar.finish_with_message(message.to_string());
        for bar in &self.agent_bars {
            bar.finish_and_clear();
        }
    }

    /// Start the timer update task
    pub fn start_timer(&mut self) {
        let is_finished = self.is_finished.clone();
        let overall_bar = self.overall_bar.clone();

        // Spawn timer update task
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                if is_finished.load(Ordering::Relaxed) {
                    break;
                }
                overall_bar.tick();
            }
        });

        self.tick_handle = Some(handle);
    }

    /// Get current agent operations
    pub async fn get_agent_operations(&self) -> Vec<AgentOperation> {
        self.agent_operations.read().await.clone()
    }

    /// Get the number of agent bars
    pub fn agent_count(&self) -> usize {
        self.agent_bars.len()
    }

    /// Check if tracking is finished
    pub fn is_finished(&self) -> bool {
        self.is_finished.load(Ordering::Relaxed)
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Set overall progress message
    pub fn set_message(&self, message: &str) {
        self.overall_bar.set_message(message.to_string());
    }

    /// Get current progress position
    pub fn position(&self) -> u64 {
        self.overall_bar.position()
    }

    /// Get total items
    pub fn length(&self) -> u64 {
        self.overall_bar.length().unwrap_or(0)
    }

    /// Increment overall progress by a specific amount
    pub fn inc(&self, delta: u64) {
        self.overall_bar.inc(delta);
    }

    /// Set specific position
    pub fn set_position(&self, pos: u64) {
        self.overall_bar.set_position(pos);
    }

    /// Clear all progress bars
    pub fn clear(&self) {
        self.overall_bar.finish_and_clear();
        for bar in &self.agent_bars {
            bar.finish_and_clear();
        }
    }
}

/// Format an agent operation for display
fn format_operation(operation: &AgentOperation) -> String {
    match operation {
        AgentOperation::Idle => "Idle".to_string(),
        AgentOperation::Setup(cmd) => {
            format!("[setup] {}", truncate_command(cmd, 40))
        }
        AgentOperation::Claude(cmd) => {
            format!("[claude] {}", truncate_command(cmd, 40))
        }
        AgentOperation::Shell(cmd) => {
            format!("[shell] {}", truncate_command(cmd, 40))
        }
        AgentOperation::Test(cmd) => {
            format!("[test] {}", truncate_command(cmd, 40))
        }
        AgentOperation::Handler(name) => format!("[handler] {}", name),
        AgentOperation::Retrying(item, attempt) => {
            format!("Retrying {} (attempt {})", item, attempt)
        }
        AgentOperation::Complete => "Complete".to_string(),
    }
}

/// Truncate a command string to a maximum length
fn truncate_command(cmd: &str, max_len: usize) -> String {
    if cmd.len() <= max_len {
        cmd.to_string()
    } else {
        format!("{}...", &cmd[..max_len - 3])
    }
}

impl Drop for ProgressTracker {
    fn drop(&mut self) {
        // Ensure the timer task is cancelled
        if let Some(handle) = self.tick_handle.take() {
            handle.abort();
        }

        // Clear progress bars
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_tracker_creation() {
        let tracker = ProgressTracker::new(100, 10);
        assert_eq!(tracker.agent_count(), 10);
        assert!(!tracker.is_finished());
        assert_eq!(tracker.length(), 100);
    }

    #[tokio::test]
    async fn test_progress_operations() {
        let mut tracker = ProgressTracker::new(10, 5);
        tracker.start_timer();

        // Update operations
        tracker
            .update_agent_operation(0, AgentOperation::Setup("test setup".to_string()))
            .await;
        tracker
            .update_agent_operation(1, AgentOperation::Complete)
            .await;

        // Check operations
        let ops = tracker.get_agent_operations().await;
        assert!(matches!(ops[0], AgentOperation::Setup(_)));
        assert!(matches!(ops[1], AgentOperation::Complete));

        // Complete items
        tracker.complete_item();
        assert_eq!(tracker.position(), 1);

        // Finish
        tracker.finish("Done");
        assert!(tracker.is_finished());
    }

    #[test]
    fn test_truncate_command() {
        assert_eq!(truncate_command("short", 10), "short");
        assert_eq!(
            truncate_command("this is a very long command", 10),
            "this is..."
        );
    }

    #[test]
    fn test_format_operation() {
        let op = AgentOperation::Claude("claude command".to_string());
        assert!(format_operation(&op).contains("[claude]"));

        let op = AgentOperation::Retrying("item1".to_string(), 2);
        let formatted = format_operation(&op);
        assert!(formatted.contains("Retrying item1"));
        assert!(formatted.contains("attempt 2"));
    }
}
