//! Progress tracking for sequential workflow execution and resume
//!
//! Provides real-time progress monitoring for sequential workflows,
//! including support for resumed workflows with accurate step tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

/// Progress tracker for sequential workflow execution
#[derive(Clone)]
pub struct SequentialProgressTracker {
    /// Total number of steps in the workflow
    pub total_steps: usize,
    /// Current step being executed (0-indexed)
    pub current_step: usize,
    /// Number of steps completed
    pub completed_steps: usize,
    /// Number of steps skipped (from resume)
    pub skipped_steps: usize,
    /// Start time of execution
    pub start_time: Instant,
    /// Current iteration number
    pub current_iteration: usize,
    /// Maximum iterations
    pub max_iterations: usize,
    /// Progress state
    pub state: Arc<RwLock<ProgressState>>,
    /// Progress update channel
    pub update_sender: mpsc::UnboundedSender<ProgressUpdate>,
    /// Progress callback
    pub progress_callback: Option<Arc<dyn Fn(ProgressUpdate) + Send + Sync>>,
}

/// Current progress state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressState {
    /// Workflow ID
    pub workflow_id: String,
    /// Workflow name
    pub workflow_name: String,
    /// Current phase
    pub phase: ExecutionPhase,
    /// Overall progress percentage (0-100)
    pub overall_progress: f32,
    /// Step progress percentage (0-100)
    pub step_progress: f32,
    /// Current step name
    pub current_step_name: String,
    /// Steps completed
    pub steps_completed: usize,
    /// Total steps
    pub total_steps: usize,
    /// Steps skipped (from resume)
    pub steps_skipped: usize,
    /// Current iteration
    pub current_iteration: usize,
    /// Maximum iterations
    pub max_iterations: usize,
    /// Estimated time remaining
    pub estimated_time_remaining: Option<Duration>,
    /// Average step duration
    pub average_step_duration: Option<Duration>,
    /// Errors encountered
    pub error_count: usize,
    /// Last update timestamp
    pub last_update: DateTime<Utc>,
}

/// Execution phase
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionPhase {
    /// Initializing workflow
    Initializing,
    /// Loading checkpoint for resume
    LoadingCheckpoint,
    /// Restoring workflow state
    RestoringState,
    /// Executing steps
    ExecutingSteps,
    /// Executing error handler
    ExecutingErrorHandler,
    /// Saving checkpoint
    SavingCheckpoint,
    /// Completing iteration
    CompletingIteration,
    /// Workflow completed
    Completed,
    /// Workflow failed
    Failed { error: String },
}

/// Progress update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    /// Timestamp of update
    pub timestamp: DateTime<Utc>,
    /// Update type
    pub update_type: UpdateType,
    /// Current state snapshot
    pub state: ProgressState,
    /// Optional message
    pub message: Option<String>,
}

/// Type of progress update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateType {
    /// Workflow started
    WorkflowStarted,
    /// Iteration started
    IterationStarted { iteration: usize },
    /// Step started
    StepStarted {
        step_index: usize,
        step_name: String,
    },
    /// Step progress
    StepProgress { percentage: f32 },
    /// Step completed
    StepCompleted {
        step_index: usize,
        duration: Duration,
    },
    /// Step skipped (from resume)
    StepSkipped { step_index: usize, reason: String },
    /// Step failed
    StepFailed { step_index: usize, error: String },
    /// Error handler started
    ErrorHandlerStarted,
    /// Error handler completed
    ErrorHandlerCompleted { success: bool },
    /// Checkpoint saved
    CheckpointSaved,
    /// Iteration completed
    IterationCompleted { iteration: usize },
    /// Workflow completed
    WorkflowCompleted { total_duration: Duration },
    /// Workflow failed
    WorkflowFailed { error: String },
}

impl SequentialProgressTracker {
    /// Create a new progress tracker
    pub fn new(
        workflow_id: String,
        workflow_name: String,
        total_steps: usize,
        max_iterations: usize,
    ) -> Self {
        let (update_sender, _) = mpsc::unbounded_channel();

        let initial_state = ProgressState {
            workflow_id,
            workflow_name,
            phase: ExecutionPhase::Initializing,
            overall_progress: 0.0,
            step_progress: 0.0,
            current_step_name: String::new(),
            steps_completed: 0,
            total_steps,
            steps_skipped: 0,
            current_iteration: 0,
            max_iterations,
            estimated_time_remaining: None,
            average_step_duration: None,
            error_count: 0,
            last_update: Utc::now(),
        };

        Self {
            total_steps,
            current_step: 0,
            completed_steps: 0,
            skipped_steps: 0,
            start_time: Instant::now(),
            current_iteration: 0,
            max_iterations,
            state: Arc::new(RwLock::new(initial_state)),
            update_sender,
            progress_callback: None,
        }
    }

    /// Create a progress tracker for resume
    pub fn for_resume(
        workflow_id: String,
        workflow_name: String,
        total_steps: usize,
        max_iterations: usize,
        skipped_steps: usize,
        starting_iteration: usize,
    ) -> Self {
        let mut tracker = Self::new(workflow_id, workflow_name, total_steps, max_iterations);
        tracker.skipped_steps = skipped_steps;
        tracker.completed_steps = skipped_steps;
        tracker.current_step = skipped_steps;
        tracker.current_iteration = starting_iteration;
        tracker
    }

    /// Set progress callback
    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(ProgressUpdate) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Arc::new(callback));
    }

    /// Update phase
    pub async fn update_phase(&self, phase: ExecutionPhase) {
        let mut state = self.state.write().await;
        state.phase = phase.clone();
        state.last_update = Utc::now();

        let update = ProgressUpdate {
            timestamp: Utc::now(),
            update_type: match phase {
                ExecutionPhase::Initializing => UpdateType::WorkflowStarted,
                ExecutionPhase::Completed => UpdateType::WorkflowCompleted {
                    total_duration: self.start_time.elapsed(),
                },
                ExecutionPhase::Failed { ref error } => UpdateType::WorkflowFailed {
                    error: error.clone(),
                },
                _ => return, // Don't send updates for intermediate phases
            },
            state: state.clone(),
            message: None,
        };

        self.send_update(update).await;
    }

    /// Start iteration
    pub async fn start_iteration(&mut self, iteration: usize) {
        self.current_iteration = iteration;

        let mut state = self.state.write().await;
        state.current_iteration = iteration;
        state.phase = ExecutionPhase::ExecutingSteps;
        state.last_update = Utc::now();

        let update = ProgressUpdate {
            timestamp: Utc::now(),
            update_type: UpdateType::IterationStarted { iteration },
            state: state.clone(),
            message: Some(format!(
                "Starting iteration {}/{}",
                iteration, self.max_iterations
            )),
        };

        self.send_update(update).await;
    }

    /// Start step
    pub async fn start_step(&mut self, step_index: usize, step_name: String) {
        self.current_step = step_index;

        let mut state = self.state.write().await;
        state.current_step_name = step_name.clone();
        state.step_progress = 0.0;
        state.phase = ExecutionPhase::ExecutingSteps;
        state.last_update = Utc::now();

        // Calculate overall progress
        self.update_overall_progress(&mut state);

        let update = ProgressUpdate {
            timestamp: Utc::now(),
            update_type: UpdateType::StepStarted {
                step_index,
                step_name,
            },
            state: state.clone(),
            message: Some(format!(
                "Step {}/{}: {}",
                step_index + 1,
                self.total_steps,
                state.current_step_name
            )),
        };

        self.send_update(update).await;
    }

    /// Update step progress
    pub async fn update_step_progress(&self, percentage: f32) {
        let mut state = self.state.write().await;
        state.step_progress = percentage.clamp(0.0, 100.0);
        state.last_update = Utc::now();

        let update = ProgressUpdate {
            timestamp: Utc::now(),
            update_type: UpdateType::StepProgress { percentage },
            state: state.clone(),
            message: None,
        };

        self.send_update(update).await;
    }

    /// Complete step
    pub async fn complete_step(&mut self, step_index: usize, duration: Duration) {
        self.completed_steps += 1;

        let mut state = self.state.write().await;
        state.steps_completed = self.completed_steps;
        state.step_progress = 100.0;
        state.last_update = Utc::now();

        // Update average step duration
        if let Some(avg) = state.average_step_duration {
            let new_avg = (avg + duration) / 2;
            state.average_step_duration = Some(new_avg);
        } else {
            state.average_step_duration = Some(duration);
        }

        // Calculate estimated time remaining
        if let Some(avg_duration) = state.average_step_duration {
            let remaining_steps = self.total_steps - self.completed_steps;
            let remaining_iterations = self.max_iterations - self.current_iteration;
            let estimated_steps = remaining_steps + (remaining_iterations * self.total_steps);
            state.estimated_time_remaining = Some(avg_duration * estimated_steps as u32);
        }

        // Update overall progress
        self.update_overall_progress(&mut state);

        let update = ProgressUpdate {
            timestamp: Utc::now(),
            update_type: UpdateType::StepCompleted {
                step_index,
                duration,
            },
            state: state.clone(),
            message: Some(format!(
                "Completed step {}/{}",
                step_index + 1,
                self.total_steps
            )),
        };

        self.send_update(update).await;
    }

    /// Skip step (for resume)
    pub async fn skip_step(&mut self, step_index: usize, reason: String) {
        self.skipped_steps += 1;
        self.current_step = step_index + 1;

        let mut state = self.state.write().await;
        state.steps_skipped = self.skipped_steps;
        state.last_update = Utc::now();

        // Update overall progress
        self.update_overall_progress(&mut state);

        let update = ProgressUpdate {
            timestamp: Utc::now(),
            update_type: UpdateType::StepSkipped { step_index, reason },
            state: state.clone(),
            message: Some(format!(
                "Skipped step {}/{}",
                step_index + 1,
                self.total_steps
            )),
        };

        self.send_update(update).await;
    }

    /// Mark step as failed
    pub async fn fail_step(&mut self, step_index: usize, error: String) {
        let mut state = self.state.write().await;
        state.error_count += 1;
        state.last_update = Utc::now();

        let update = ProgressUpdate {
            timestamp: Utc::now(),
            update_type: UpdateType::StepFailed {
                step_index,
                error: error.clone(),
            },
            state: state.clone(),
            message: Some(format!("Step {} failed: {}", step_index + 1, error)),
        };

        self.send_update(update).await;
    }

    /// Calculate overall progress
    fn update_overall_progress(&self, state: &mut ProgressState) {
        let total_work = self.total_steps * self.max_iterations;
        let completed_work = (self.completed_steps + self.skipped_steps)
            + (self.current_iteration.saturating_sub(1) * self.total_steps);

        state.overall_progress = if total_work > 0 {
            ((completed_work as f32 / total_work as f32) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
    }

    /// Send progress update
    async fn send_update(&self, update: ProgressUpdate) {
        // Send to channel
        let _ = self.update_sender.send(update.clone());

        // Call callback if set
        if let Some(ref callback) = self.progress_callback {
            callback(update);
        }
    }

    /// Get current progress state
    pub async fn get_state(&self) -> ProgressState {
        self.state.read().await.clone()
    }

    /// Format progress for display
    pub async fn format_progress(&self) -> String {
        let state = self.state.read().await;

        let mut output = vec![
            format!("Workflow: {}", state.workflow_name),
            format!("Progress: {:.1}%", state.overall_progress),
        ];

        if self.max_iterations > 1 {
            output.push(format!(
                "Iteration: {}/{}",
                state.current_iteration, state.max_iterations
            ));
        }

        output.push(format!(
            "Steps: {}/{} completed",
            state.steps_completed, state.total_steps
        ));

        if state.steps_skipped > 0 {
            output.push(format!("Skipped: {} (resumed)", state.steps_skipped));
        }

        if !state.current_step_name.is_empty() {
            output.push(format!("Current: {}", state.current_step_name));
            if state.step_progress > 0.0 && state.step_progress < 100.0 {
                output.push(format!("Step Progress: {:.1}%", state.step_progress));
            }
        }

        if let Some(remaining) = state.estimated_time_remaining {
            let mins = remaining.as_secs() / 60;
            let secs = remaining.as_secs() % 60;
            output.push(format!("Estimated time remaining: {}m {}s", mins, secs));
        }

        if state.error_count > 0 {
            output.push(format!("Errors: {}", state.error_count));
        }

        output.join(" | ")
    }
}

/// Progress display helper for terminal output
pub struct ProgressDisplay {
    last_update: Instant,
    min_update_interval: Duration,
}

impl Default for ProgressDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressDisplay {
    /// Create a new progress display
    pub fn new() -> Self {
        Self {
            last_update: Instant::now(),
            min_update_interval: Duration::from_millis(100), // Update at most 10 times per second
        }
    }

    /// Display progress update if enough time has passed
    pub fn update(&mut self, message: &str) {
        if self.last_update.elapsed() >= self.min_update_interval {
            println!("🔄 {}", message);
            self.last_update = Instant::now();
        }
    }

    /// Force display a message regardless of timing
    pub fn force_update(&mut self, message: &str) {
        println!("🔄 {}", message);
        self.last_update = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_tracker_creation() {
        let tracker = SequentialProgressTracker::new(
            "test-id".to_string(),
            "Test Workflow".to_string(),
            10,
            2,
        );

        let state = tracker.get_state().await;
        assert_eq!(state.workflow_id, "test-id");
        assert_eq!(state.workflow_name, "Test Workflow");
        assert_eq!(state.total_steps, 10);
        assert_eq!(state.max_iterations, 2);
        assert_eq!(state.overall_progress, 0.0);
    }

    #[tokio::test]
    async fn test_progress_tracker_for_resume() {
        let tracker = SequentialProgressTracker::for_resume(
            "resume-id".to_string(),
            "Resume Workflow".to_string(),
            10,
            2,
            3, // 3 steps already completed
            1, // Starting from iteration 1
        );

        assert_eq!(tracker.skipped_steps, 3);
        assert_eq!(tracker.completed_steps, 3);
        assert_eq!(tracker.current_step, 3);
        assert_eq!(tracker.current_iteration, 1);
    }

    #[tokio::test]
    async fn test_progress_calculation() {
        let mut tracker = SequentialProgressTracker::new(
            "calc-id".to_string(),
            "Calc Workflow".to_string(),
            5, // 5 steps
            2, // 2 iterations
        );

        // Start first iteration
        tracker.start_iteration(1).await;

        // Complete 2 steps
        tracker.complete_step(0, Duration::from_secs(1)).await;
        tracker.complete_step(1, Duration::from_secs(1)).await;

        let state = tracker.get_state().await;

        // 2 steps out of 10 total (5 steps * 2 iterations) = 20%
        assert_eq!(state.overall_progress, 20.0);
        assert_eq!(state.steps_completed, 2);
    }

    #[tokio::test]
    async fn test_progress_with_skipped_steps() {
        let mut tracker = SequentialProgressTracker::for_resume(
            "skip-id".to_string(),
            "Skip Workflow".to_string(),
            5,
            2, // 2 iterations total
            2, // 2 steps already completed
            1, // Starting iteration 1 (not completed yet)
        );

        // Skip a step
        tracker.skip_step(2, "Already completed".to_string()).await;

        let state = tracker.get_state().await;
        assert_eq!(state.steps_skipped, 3);

        // With 2 iterations and 5 steps each = 10 total steps
        // We start at iteration 1, with 2 steps already completed
        // After skipping one more, we have 2 completed + 3 skipped = 5 total
        // The calculation is: (2 + 3 + (1-1)*5) / 10 = 5/10 = 50%
        assert_eq!(state.overall_progress, 50.0);
    }
}
