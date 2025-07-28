use std::path::PathBuf;
use std::time::{Duration, Instant};

/// State tracking for batch specification implementation
#[derive(Debug, Clone)]
pub struct BatchImplementState {
    /// List of specification files to implement
    pub specs: Vec<PathBuf>,

    /// Completed specifications with their results (spec_id, success, duration)
    pub completed: Vec<(String, bool, Duration)>,

    /// Currently implementing specification
    pub current_spec: Option<String>,

    /// Start time of the batch operation
    pub start_time: Instant,

    /// Whether we're in dry-run mode
    pub dry_run: bool,
}

impl BatchImplementState {
    /// Create a new batch implementation state
    pub fn new(specs: Vec<PathBuf>, dry_run: bool) -> Self {
        Self {
            specs,
            completed: Vec::new(),
            current_spec: None,
            start_time: Instant::now(),
            dry_run,
        }
    }

    /// Get the total number of specifications
    pub fn total_specs(&self) -> usize {
        self.specs.len()
    }

    /// Get the number of completed specifications
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    /// Get the number of successful implementations
    pub fn success_count(&self) -> usize {
        self.completed
            .iter()
            .filter(|(_, success, _)| *success)
            .count()
    }

    /// Get the number of failed implementations
    pub fn failure_count(&self) -> usize {
        self.completed
            .iter()
            .filter(|(_, success, _)| !*success)
            .count()
    }

    /// Get remaining specifications count
    pub fn remaining_count(&self) -> usize {
        self.total_specs() - self.completed_count()
    }

    /// Get progress percentage
    pub fn progress_percentage(&self) -> f32 {
        if self.total_specs() == 0 {
            100.0
        } else {
            (self.completed_count() as f32 / self.total_specs() as f32) * 100.0
        }
    }

    /// Mark current spec as completed
    pub fn complete_current(&mut self, success: bool, duration: Duration) {
        if let Some(spec_id) = self.current_spec.take() {
            self.completed.push((spec_id, success, duration));
        }
    }

    /// Generate a summary report
    pub fn generate_summary(&self) -> String {
        let total_duration = self.start_time.elapsed();
        let mut summary = format!("\n📊 Batch Implementation Summary\n{}\n", "=".repeat(40));

        summary.push_str(&format!("Total specifications: {}\n", self.total_specs()));
        summary.push_str(&format!("✅ Succeeded: {}\n", self.success_count()));
        summary.push_str(&format!("❌ Failed: {}\n", self.failure_count()));
        summary.push_str(&format!("⏸ Remaining: {}\n", self.remaining_count()));
        summary.push_str(&format!("⏱ Total time: {:?}\n", total_duration));

        if !self.completed.is_empty() {
            summary.push_str(&format!("\n{}\n", "Details:"));
            for (spec_id, success, duration) in &self.completed {
                let status = if *success { "✅" } else { "❌" };
                summary.push_str(&format!("{} {} ({:?})\n", status, spec_id, duration));
            }
        }

        summary
    }
}
