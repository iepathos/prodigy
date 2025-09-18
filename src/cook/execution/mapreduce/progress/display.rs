//! Terminal and UI rendering for progress tracking

use super::{PhaseType, ProgressState};
use async_trait::async_trait;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

/// Error type for rendering operations
#[derive(Debug)]
pub struct RenderError {
    message: String,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Render error: {}", self.message)
    }
}

impl std::error::Error for RenderError {}

impl RenderError {
    /// Create a new render error
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Trait for progress renderers
#[async_trait]
pub trait ProgressRenderer: Send + Sync {
    /// Initialize the renderer with initial state
    async fn initialize(&mut self, state: &ProgressState) -> Result<(), RenderError>;

    /// Render the current progress state
    async fn render(&mut self, state: &ProgressState) -> Result<(), RenderError>;

    /// Finalize the renderer with a message
    async fn finalize(&mut self, state: &ProgressState, message: &str) -> Result<(), RenderError>;

    /// Check if this renderer supports terminal output
    fn supports_terminal(&self) -> bool;

    /// Get renderer name
    fn name(&self) -> &str;
}

/// Terminal-based progress renderer using indicatif
pub struct TerminalProgressRenderer {
    multi_progress: Option<MultiProgress>,
    overall_bar: Option<ProgressBar>,
    phase_bar: Option<ProgressBar>,
    agent_bars: HashMap<usize, ProgressBar>,
    max_agents: usize,
    is_initialized: bool,
}

impl TerminalProgressRenderer {
    /// Create a new terminal progress renderer
    pub fn new(max_agents: usize) -> Self {
        Self {
            multi_progress: None,
            overall_bar: None,
            phase_bar: None,
            agent_bars: HashMap::new(),
            max_agents,
            is_initialized: false,
        }
    }

    /// Create the overall progress bar
    fn create_overall_bar(&self, multi: &MultiProgress, total: usize) -> ProgressBar {
        let bar = multi.add(ProgressBar::new(total as u64));
        bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} ({percent}%) {msg}",
                )
                .unwrap()
                .progress_chars("##-"),
        );
        bar.enable_steady_tick(Duration::from_millis(100));
        bar
    }

    /// Create the phase indicator bar
    fn create_phase_bar(&self, multi: &MultiProgress) -> ProgressBar {
        let bar = multi.add(ProgressBar::new_spinner());
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.yellow} Phase: {msg}")
                .unwrap(),
        );
        bar.enable_steady_tick(Duration::from_millis(100));
        bar
    }

    /// Create an agent progress bar
    fn create_agent_bar(&self, multi: &MultiProgress, index: usize) -> ProgressBar {
        let bar = multi.add(ProgressBar::new_spinner());
        bar.set_style(
            ProgressStyle::default_spinner()
                .template(&format!(
                    "  {{spinner:.cyan}} Agent {:2}: {{msg}}",
                    index + 1
                ))
                .unwrap(),
        );
        bar
    }

    /// Format phase for display
    fn format_phase(phase: &PhaseType) -> &'static str {
        match phase {
            PhaseType::Setup => "Setup",
            PhaseType::Map => "Map",
            PhaseType::Reduce => "Reduce",
        }
    }

    /// Format time duration for display
    fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }
}

#[async_trait]
impl ProgressRenderer for TerminalProgressRenderer {
    async fn initialize(&mut self, state: &ProgressState) -> Result<(), RenderError> {
        if self.is_initialized {
            return Ok(());
        }

        let multi = MultiProgress::new();

        // Create overall progress bar
        let overall = self.create_overall_bar(&multi, state.total_items);
        overall.set_message("Starting...");

        // Create phase bar
        let phase = self.create_phase_bar(&multi);
        phase.set_message(Self::format_phase(&state.current_phase));

        // Store references
        self.multi_progress = Some(multi.clone());
        self.overall_bar = Some(overall);
        self.phase_bar = Some(phase);
        self.is_initialized = true;

        Ok(())
    }

    async fn render(&mut self, state: &ProgressState) -> Result<(), RenderError> {
        if !self.is_initialized {
            return Err(RenderError::new("Renderer not initialized"));
        }

        let multi = self
            .multi_progress
            .as_ref()
            .ok_or_else(|| RenderError::new("MultiProgress not initialized"))?;

        // Update overall progress
        if let Some(overall) = &self.overall_bar {
            overall.set_position(state.completed_items as u64);

            // Build status message
            let rate = state.items_per_second();
            let mut msg = format!(
                "{}/{} items | {:.1} items/s",
                state.completed_items, state.total_items, rate
            );

            if state.failed_items > 0 {
                msg.push_str(&format!(" | {} failed", state.failed_items));
            }

            if let Some(eta) = state.estimated_time_remaining() {
                msg.push_str(&format!(" | ETA: {}", Self::format_duration(eta)));
            }

            overall.set_message(msg);
        }

        // Update phase
        if let Some(phase) = &self.phase_bar {
            phase.set_message(format!(
                "{} | Elapsed: {}",
                Self::format_phase(&state.current_phase),
                Self::format_duration(state.start_time.elapsed())
            ));
        }

        // Update agent bars
        for (index, agent_progress) in &state.agent_states {
            // Only show up to max_agents
            if *index >= self.max_agents {
                continue;
            }

            // Check if we need to create a new bar
            if !self.agent_bars.contains_key(index) {
                let new_bar = self.create_agent_bar(multi, *index);
                self.agent_bars.insert(*index, new_bar);
            }

            // Update the bar
            if let Some(bar) = self.agent_bars.get(index) {
                let status = agent_progress.operation.description();
                let msg = if agent_progress.items_processed > 0 {
                    format!("{} | {} items done", status, agent_progress.items_processed)
                } else {
                    status
                };

                bar.set_message(msg);
            }
        }

        // Remove bars for agents that are no longer active
        let active_indices: Vec<usize> = state
            .agent_states
            .keys()
            .copied()
            .filter(|idx| *idx < self.max_agents)
            .collect();

        let to_remove: Vec<usize> = self
            .agent_bars
            .keys()
            .copied()
            .filter(|idx| !active_indices.contains(idx))
            .collect();

        for idx in to_remove {
            if let Some(bar) = self.agent_bars.remove(&idx) {
                bar.finish_and_clear();
            }
        }

        Ok(())
    }

    async fn finalize(&mut self, state: &ProgressState, message: &str) -> Result<(), RenderError> {
        if !self.is_initialized {
            return Ok(());
        }

        // Update overall bar
        if let Some(overall) = &self.overall_bar {
            overall.set_position(state.completed_items as u64);
            overall.finish_with_message(format!(
                "{} | Total time: {}",
                message,
                Self::format_duration(state.start_time.elapsed())
            ));
        }

        // Clear phase bar
        if let Some(phase) = &self.phase_bar {
            phase.finish_and_clear();
        }

        // Clear all agent bars
        for bar in self.agent_bars.values() {
            bar.finish_and_clear();
        }

        self.agent_bars.clear();
        self.is_initialized = false;

        Ok(())
    }

    fn supports_terminal(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "Terminal Progress Renderer"
    }
}

/// Simple text-based progress renderer (for non-terminal environments)
pub struct TextProgressRenderer {
    last_update: Option<std::time::Instant>,
    update_interval: Duration,
}

impl TextProgressRenderer {
    /// Create a new text progress renderer
    pub fn new() -> Self {
        Self {
            last_update: None,
            update_interval: Duration::from_secs(5),
        }
    }

    /// Set update interval
    pub fn with_update_interval(mut self, interval: Duration) -> Self {
        self.update_interval = interval;
        self
    }
}

impl Default for TextProgressRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProgressRenderer for TextProgressRenderer {
    async fn initialize(&mut self, state: &ProgressState) -> Result<(), RenderError> {
        println!(
            "Starting MapReduce execution: {} total items",
            state.total_items
        );
        self.last_update = Some(std::time::Instant::now());
        Ok(())
    }

    async fn render(&mut self, state: &ProgressState) -> Result<(), RenderError> {
        // Only update at specified intervals
        if let Some(last) = self.last_update {
            if last.elapsed() < self.update_interval {
                return Ok(());
            }
        }

        println!(
            "Progress: {}/{} ({:.1}%) | Phase: {:?} | Rate: {:.1} items/s",
            state.completed_items,
            state.total_items,
            state.completion_percentage(),
            state.current_phase,
            state.items_per_second()
        );

        if state.failed_items > 0 {
            println!("  Failed items: {}", state.failed_items);
        }

        self.last_update = Some(std::time::Instant::now());
        Ok(())
    }

    async fn finalize(&mut self, state: &ProgressState, message: &str) -> Result<(), RenderError> {
        println!(
            "Completed: {} | {}/{} items processed | {} failed | Duration: {:?}",
            message,
            state.completed_items,
            state.total_items,
            state.failed_items,
            state.start_time.elapsed()
        );
        Ok(())
    }

    fn supports_terminal(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        "Text Progress Renderer"
    }
}

/// No-op progress renderer (for silent operation)
pub struct NullProgressRenderer;

#[async_trait]
impl ProgressRenderer for NullProgressRenderer {
    async fn initialize(&mut self, _state: &ProgressState) -> Result<(), RenderError> {
        Ok(())
    }

    async fn render(&mut self, _state: &ProgressState) -> Result<(), RenderError> {
        Ok(())
    }

    async fn finalize(
        &mut self,
        _state: &ProgressState,
        _message: &str,
    ) -> Result<(), RenderError> {
        Ok(())
    }

    fn supports_terminal(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        "Null Progress Renderer"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::progress::operations::AgentOperation;

    #[tokio::test]
    async fn test_terminal_renderer() {
        let mut renderer = TerminalProgressRenderer::new(5);
        let state = ProgressState::new(100);

        // Initialize
        assert!(renderer.initialize(&state).await.is_ok());
        assert!(renderer.is_initialized);

        // Render
        assert!(renderer.render(&state).await.is_ok());

        // Finalize
        assert!(renderer.finalize(&state, "Done").await.is_ok());
        assert!(!renderer.is_initialized);
    }

    #[tokio::test]
    async fn test_text_renderer() {
        let mut renderer = TextProgressRenderer::new();
        let mut state = ProgressState::new(50);
        state.completed_items = 25;

        assert!(renderer.initialize(&state).await.is_ok());
        assert!(renderer.render(&state).await.is_ok());
        assert!(renderer.finalize(&state, "Complete").await.is_ok());
    }

    #[test]
    fn test_format_duration() {
        let dur = Duration::from_secs(3661); // 1h 1m 1s
        assert_eq!(TerminalProgressRenderer::format_duration(dur), "1h 1m 1s");

        let dur = Duration::from_secs(65); // 1m 5s
        assert_eq!(TerminalProgressRenderer::format_duration(dur), "1m 5s");

        let dur = Duration::from_secs(45); // 45s
        assert_eq!(TerminalProgressRenderer::format_duration(dur), "45s");
    }

    #[test]
    fn test_format_phase() {
        assert_eq!(
            TerminalProgressRenderer::format_phase(&PhaseType::Setup),
            "Setup"
        );
        assert_eq!(
            TerminalProgressRenderer::format_phase(&PhaseType::Map),
            "Map"
        );
        assert_eq!(
            TerminalProgressRenderer::format_phase(&PhaseType::Reduce),
            "Reduce"
        );
    }
}
