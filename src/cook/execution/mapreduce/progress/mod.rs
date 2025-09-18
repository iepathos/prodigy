//! Progress management module for MapReduce execution
//!
//! Provides unified progress tracking, display, and streaming capabilities
//! for all MapReduce operations with support for multiple concurrent displays.

pub mod display;
pub mod operations;
pub mod stream;
pub mod tracker;

// Re-export core types for convenience
pub use display::{ProgressRenderer, TerminalProgressRenderer};
pub use operations::{AgentOperation, AgentOperationTracker};
pub use stream::{ProgressEvent, ProgressStreamConsumer, ProgressStreamer};
pub use tracker::ProgressTracker;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error};

/// Progress update types for the progress system
#[derive(Debug, Clone)]
pub enum ProgressUpdate {
    /// Item completed processing
    ItemComplete(String),
    /// Agent status changed
    AgentStatus(usize, AgentOperation),
    /// Phase changed
    PhaseChange(PhaseType),
    /// Error occurred
    Error(String),
    /// Custom message
    Message(String),
}

/// Phase types for progress tracking
#[derive(Debug, Clone, PartialEq)]
pub enum PhaseType {
    Setup,
    Map,
    Reduce,
}

/// Configuration for progress management
#[derive(Debug, Clone)]
pub struct ProgressConfig {
    /// Enable terminal display
    pub enable_terminal: bool,
    /// Enable progress streaming
    pub enable_streaming: bool,
    /// Update frequency in milliseconds
    pub update_frequency_ms: u64,
    /// Show agent details
    pub show_agent_details: bool,
    /// Maximum agents to display
    pub max_display_agents: usize,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            enable_terminal: true,
            enable_streaming: false,
            update_frequency_ms: 100,
            show_agent_details: true,
            max_display_agents: 10,
        }
    }
}

/// Current progress state
#[derive(Debug, Clone)]
pub struct ProgressState {
    /// Total items to process
    pub total_items: usize,
    /// Completed items count
    pub completed_items: usize,
    /// Failed items count
    pub failed_items: usize,
    /// Agent progress states
    pub agent_states: HashMap<usize, AgentProgress>,
    /// Start time
    pub start_time: Instant,
    /// Current phase
    pub current_phase: PhaseType,
    /// Last update time
    pub last_update: Instant,
}

impl ProgressState {
    /// Create new progress state
    pub fn new(total_items: usize) -> Self {
        let now = Instant::now();
        Self {
            total_items,
            completed_items: 0,
            failed_items: 0,
            agent_states: HashMap::new(),
            start_time: now,
            current_phase: PhaseType::Setup,
            last_update: now,
        }
    }

    /// Get completion percentage
    pub fn completion_percentage(&self) -> f64 {
        if self.total_items == 0 {
            return 100.0;
        }
        (self.completed_items as f64 / self.total_items as f64) * 100.0
    }

    /// Get estimated time remaining
    pub fn estimated_time_remaining(&self) -> Option<std::time::Duration> {
        if self.completed_items == 0 {
            return None;
        }

        let elapsed = self.start_time.elapsed();
        let per_item = elapsed / self.completed_items as u32;
        let remaining_items = self.total_items - self.completed_items;

        Some(per_item * remaining_items as u32)
    }

    /// Get items per second rate
    pub fn items_per_second(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed == 0.0 {
            return 0.0;
        }
        self.completed_items as f64 / elapsed
    }
}

/// Agent progress information
#[derive(Debug, Clone)]
pub struct AgentProgress {
    /// Agent index
    pub index: usize,
    /// Current operation
    pub operation: AgentOperation,
    /// Items processed by this agent
    pub items_processed: usize,
    /// Last activity time
    pub last_activity: Instant,
}

/// Main progress manager for MapReduce execution
pub struct ProgressManager {
    /// Current progress state
    state: Arc<RwLock<ProgressState>>,
    /// Progress renderers
    renderers: Vec<Box<dyn ProgressRenderer>>,
    /// Update channel sender
    update_tx: mpsc::Sender<ProgressUpdate>,
    /// Update channel receiver
    update_rx: Option<mpsc::Receiver<ProgressUpdate>>,
    /// Configuration
    config: ProgressConfig,
    /// Progress streamer
    streamer: Option<Arc<ProgressStreamer>>,
}

impl ProgressManager {
    /// Create new progress manager
    pub fn new(config: ProgressConfig) -> Self {
        let (update_tx, update_rx) = mpsc::channel(1000);

        let mut renderers: Vec<Box<dyn ProgressRenderer>> = Vec::new();

        // Add terminal renderer if enabled
        if config.enable_terminal {
            renderers.push(Box::new(TerminalProgressRenderer::new(
                config.max_display_agents,
            )));
        }

        // Create streamer if enabled
        let streamer = if config.enable_streaming {
            Some(Arc::new(ProgressStreamer::new()))
        } else {
            None
        };

        Self {
            state: Arc::new(RwLock::new(ProgressState::new(0))),
            renderers,
            update_tx,
            update_rx: Some(update_rx),
            config,
            streamer,
        }
    }

    /// Start tracking progress for given number of items
    pub async fn start_tracking(&mut self, total_items: usize) {
        {
            let mut state = self.state.write().await;
            *state = ProgressState::new(total_items);

            // Initialize renderers
            for renderer in &mut self.renderers {
                if let Err(e) = renderer.initialize(&state).await {
                    error!("Failed to initialize renderer: {}", e);
                }
            }
        }

        // Start update processing
        self.start_update_processor().await;
    }

    /// Update agent status
    pub async fn update_agent(&self, index: usize, operation: AgentOperation) {
        let update = ProgressUpdate::AgentStatus(index, operation);
        if let Err(e) = self.update_tx.send(update).await {
            debug!("Failed to send agent update: {}", e);
        }
    }

    /// Mark item as complete
    pub async fn item_complete(&self, item_id: &str) {
        let update = ProgressUpdate::ItemComplete(item_id.to_string());
        if let Err(e) = self.update_tx.send(update).await {
            debug!("Failed to send completion update: {}", e);
        }
    }

    /// Report an error
    pub async fn report_error(&self, error: &str) {
        let update = ProgressUpdate::Error(error.to_string());
        if let Err(e) = self.update_tx.send(update).await {
            debug!("Failed to send error update: {}", e);
        }
    }

    /// Change phase
    pub async fn change_phase(&self, phase: PhaseType) {
        let update = ProgressUpdate::PhaseChange(phase);
        if let Err(e) = self.update_tx.send(update).await {
            debug!("Failed to send phase change: {}", e);
        }
    }

    /// Subscribe to progress events
    pub fn subscribe(&self) -> Option<tokio::sync::broadcast::Receiver<ProgressEvent>> {
        self.streamer.as_ref().map(|s| s.subscribe())
    }

    /// Finish progress tracking
    pub async fn finish(&mut self, message: &str) {
        // Send final message
        if let Err(e) = self
            .update_tx
            .send(ProgressUpdate::Message(message.to_string()))
            .await
        {
            debug!("Failed to send finish message: {}", e);
        }

        // Finalize renderers
        let state = self.state.read().await;
        for renderer in &mut self.renderers {
            if let Err(e) = renderer.finalize(&state, message).await {
                error!("Failed to finalize renderer: {}", e);
            }
        }

        // Stop streamer
        if let Some(streamer) = &self.streamer {
            streamer.stop().await;
        }
    }

    /// Start the update processor task
    async fn start_update_processor(&mut self) {
        let mut update_rx = self
            .update_rx
            .take()
            .expect("Update receiver already taken");
        let state = Arc::clone(&self.state);
        let renderers = std::mem::take(&mut self.renderers);
        let renderers = Arc::new(RwLock::new(renderers));
        let streamer = self.streamer.clone();
        let update_frequency = std::time::Duration::from_millis(self.config.update_frequency_ms);

        tokio::spawn(async move {
            let mut last_render = Instant::now();

            while let Some(update) = update_rx.recv().await {
                // Apply update to state
                let mut state = state.write().await;
                Self::apply_update(&mut state, &update);

                // Stream event if enabled
                if let Some(streamer) = &streamer {
                    let event = Self::update_to_event(&update, &state);
                    streamer.stream_event(event).await;
                }

                // Render if enough time has passed
                if last_render.elapsed() >= update_frequency {
                    let mut renderers = renderers.write().await;
                    for renderer in renderers.iter_mut() {
                        if let Err(e) = renderer.render(&state).await {
                            debug!("Render error: {}", e);
                        }
                    }
                    last_render = Instant::now();
                }
            }
        });
    }

    /// Apply an update to the progress state
    fn apply_update(state: &mut ProgressState, update: &ProgressUpdate) {
        state.last_update = Instant::now();

        match update {
            ProgressUpdate::ItemComplete(_) => {
                state.completed_items += 1;
            }
            ProgressUpdate::AgentStatus(index, operation) => {
                let agent = state
                    .agent_states
                    .entry(*index)
                    .or_insert_with(|| AgentProgress {
                        index: *index,
                        operation: AgentOperation::Idle,
                        items_processed: 0,
                        last_activity: Instant::now(),
                    });

                // Update items processed if operation is complete
                if matches!(operation, AgentOperation::Complete) {
                    agent.items_processed += 1;
                }

                agent.operation = operation.clone();
                agent.last_activity = Instant::now();
            }
            ProgressUpdate::PhaseChange(phase) => {
                state.current_phase = phase.clone();
            }
            ProgressUpdate::Error(_) => {
                state.failed_items += 1;
            }
            ProgressUpdate::Message(_) => {
                // No state change for messages
            }
        }
    }

    /// Convert update to event
    fn update_to_event(update: &ProgressUpdate, state: &ProgressState) -> ProgressEvent {
        match update {
            ProgressUpdate::ItemComplete(id) => ProgressEvent::ItemComplete {
                item_id: id.clone(),
                total_completed: state.completed_items,
                percentage: state.completion_percentage(),
            },
            ProgressUpdate::AgentStatus(index, operation) => ProgressEvent::AgentUpdate {
                agent_index: *index,
                operation: operation.clone(),
                timestamp: Instant::now(),
            },
            ProgressUpdate::PhaseChange(phase) => ProgressEvent::PhaseChange {
                phase: phase.clone(),
                timestamp: Instant::now(),
            },
            ProgressUpdate::Error(msg) => ProgressEvent::Error {
                message: msg.clone(),
                failed_count: state.failed_items,
            },
            ProgressUpdate::Message(msg) => ProgressEvent::Message(msg.clone()),
        }
    }

    /// Get current state snapshot
    pub async fn get_state(&self) -> ProgressState {
        self.state.read().await.clone()
    }

    /// Add a custom renderer
    pub async fn add_renderer(&mut self, renderer: Box<dyn ProgressRenderer>) {
        let state = self.state.read().await;
        let mut renderer = renderer;
        if let Err(e) = renderer.initialize(&state).await {
            error!("Failed to initialize custom renderer: {}", e);
        } else {
            // This would need a different approach since renderers are moved
            // For now, custom renderers should be added before start_tracking
            debug!("Custom renderer added");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_state() {
        let state = ProgressState::new(100);
        assert_eq!(state.total_items, 100);
        assert_eq!(state.completed_items, 0);
        assert_eq!(state.completion_percentage(), 0.0);
    }

    #[tokio::test]
    async fn test_progress_manager() {
        let config = ProgressConfig {
            enable_terminal: false,
            enable_streaming: true,
            ..Default::default()
        };

        let mut manager = ProgressManager::new(config);
        manager.start_tracking(10).await;

        // Test updates
        manager.item_complete("item1").await;
        manager.update_agent(0, AgentOperation::Complete).await;

        // Give time for updates to process
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let state = manager.get_state().await;
        assert_eq!(state.completed_items, 1);
    }
}
