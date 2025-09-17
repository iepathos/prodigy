//! Session replay functionality for Claude sessions

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

use super::models::{ReplayEvent, Session, SessionEvent};

/// Session replay for stepping through Claude session history
pub struct SessionReplay {
    session: Session,
    current_position: usize,
    bookmarks: HashMap<String, usize>,
    playback_speed: PlaybackSpeed,
}

impl SessionReplay {
    /// Create a new session replay from a session
    pub fn new(session: Session) -> Self {
        Self {
            session,
            current_position: 0,
            bookmarks: HashMap::new(),
            playback_speed: PlaybackSpeed::Normal,
        }
    }

    /// Get current position in the replay
    pub fn position(&self) -> usize {
        self.current_position
    }

    /// Get total number of events
    pub fn total_events(&self) -> usize {
        self.session.events.len()
    }

    /// Get current progress as percentage
    pub fn progress_percent(&self) -> f64 {
        if self.session.events.is_empty() {
            return 0.0;
        }
        (self.current_position as f64 / self.session.events.len() as f64) * 100.0
    }

    /// Step forward one event
    pub fn step_forward(&mut self) -> Option<ReplayEvent> {
        if self.current_position < self.session.events.len() {
            let event = &self.session.events[self.current_position];
            self.current_position += 1;
            Some(self.format_replay_event(event))
        } else {
            None
        }
    }

    /// Step backward one event
    pub fn step_backward(&mut self) -> Option<ReplayEvent> {
        if self.current_position > 0 {
            self.current_position -= 1;
            let event = &self.session.events[self.current_position];
            Some(self.format_replay_event(event))
        } else {
            None
        }
    }

    /// Jump to a specific timestamp
    pub fn jump_to_timestamp(&mut self, timestamp: DateTime<Utc>) -> Option<ReplayEvent> {
        self.current_position = self
            .session
            .events
            .iter()
            .position(|e| e.timestamp() >= timestamp)
            .unwrap_or(self.session.events.len());

        if self.current_position < self.session.events.len() {
            Some(self.format_replay_event(&self.session.events[self.current_position]))
        } else {
            None
        }
    }

    /// Jump to a specific position
    pub fn jump_to_position(&mut self, position: usize) -> Option<ReplayEvent> {
        if position < self.session.events.len() {
            self.current_position = position;
            Some(self.format_replay_event(&self.session.events[self.current_position]))
        } else {
            None
        }
    }

    /// Reset replay to beginning
    pub fn reset(&mut self) {
        self.current_position = 0;
        debug!("Replay reset to beginning");
    }

    /// Jump to end
    pub fn jump_to_end(&mut self) {
        self.current_position = self.session.events.len();
        debug!("Replay jumped to end");
    }

    /// Set a bookmark at current position
    pub fn set_bookmark(&mut self, name: &str) {
        self.bookmarks
            .insert(name.to_string(), self.current_position);
        info!(
            "Bookmark '{}' set at position {}",
            name, self.current_position
        );
    }

    /// Jump to a bookmark
    pub fn jump_to_bookmark(&mut self, name: &str) -> Option<ReplayEvent> {
        if let Some(&position) = self.bookmarks.get(name) {
            self.jump_to_position(position)
        } else {
            None
        }
    }

    /// List all bookmarks
    pub fn list_bookmarks(&self) -> Vec<(String, usize)> {
        self.bookmarks
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Set playback speed
    pub fn set_playback_speed(&mut self, speed: PlaybackSpeed) {
        self.playback_speed = speed;
    }

    /// Play events automatically with configured speed
    pub async fn play(&mut self) -> Result<Vec<ReplayEvent>> {
        let mut events = Vec::new();
        let delay_ms = self.playback_speed.delay_ms();

        while let Some(event) = self.step_forward() {
            events.push(event.clone());

            if delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            }
        }

        Ok(events)
    }

    /// Play a specific range of events
    pub async fn play_range(&mut self, start: usize, end: usize) -> Result<Vec<ReplayEvent>> {
        let mut events = Vec::new();
        let delay_ms = self.playback_speed.delay_ms();

        self.current_position = start;
        while self.current_position < end.min(self.session.events.len()) {
            if let Some(event) = self.step_forward() {
                events.push(event);

                if delay_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
            } else {
                break;
            }
        }

        Ok(events)
    }

    /// Format a session event for replay
    fn format_replay_event(&self, event: &SessionEvent) -> ReplayEvent {
        let (event_type, content) = match event {
            SessionEvent::System { message, .. } => (
                "system".to_string(),
                serde_json::json!({ "message": message }),
            ),
            SessionEvent::Assistant { content, model, .. } => (
                "assistant".to_string(),
                serde_json::json!({
                    "content": content,
                    "model": model
                }),
            ),
            SessionEvent::ToolUse {
                tool_name,
                parameters,
                ..
            } => (
                "tool_use".to_string(),
                serde_json::json!({
                    "tool": tool_name,
                    "parameters": parameters
                }),
            ),
            SessionEvent::ToolResult {
                tool_name,
                result,
                duration_ms,
                ..
            } => (
                "tool_result".to_string(),
                serde_json::json!({
                    "tool": tool_name,
                    "result": result,
                    "duration_ms": duration_ms
                }),
            ),
            SessionEvent::Error {
                error_type,
                message,
                ..
            } => (
                "error".to_string(),
                serde_json::json!({
                    "error_type": error_type,
                    "message": message
                }),
            ),
        };

        let mut metadata = HashMap::new();
        metadata.insert("session_id".to_string(), self.session.session_id.clone());
        metadata.insert("position".to_string(), self.current_position.to_string());
        metadata.insert(
            "total_events".to_string(),
            self.session.events.len().to_string(),
        );

        ReplayEvent {
            timestamp: event.timestamp(),
            event_type,
            content,
            metadata,
        }
    }

    /// Get a summary of the current session
    pub fn get_summary(&self) -> SessionSummary {
        let tool_count = self
            .session
            .events
            .iter()
            .filter(|e| matches!(e, SessionEvent::ToolUse { .. }))
            .count();

        let error_count = self
            .session
            .events
            .iter()
            .filter(|e| matches!(e, SessionEvent::Error { .. }))
            .count();

        let duration = self
            .session
            .completed_at
            .unwrap_or_else(Utc::now)
            .signed_duration_since(self.session.started_at)
            .num_seconds();

        SessionSummary {
            session_id: self.session.session_id.clone(),
            total_events: self.session.events.len(),
            tool_invocations: tool_count,
            errors: error_count,
            duration_seconds: duration as u64,
            input_tokens: self.session.total_input_tokens,
            output_tokens: self.session.total_output_tokens,
            cache_tokens: self.session.total_cache_tokens,
        }
    }

    /// Export session transcript
    pub fn export_transcript(&self) -> String {
        let mut transcript = String::new();
        transcript.push_str(&format!("Session: {}\n", self.session.session_id));
        transcript.push_str(&format!("Started: {}\n", self.session.started_at));
        if let Some(completed) = self.session.completed_at {
            transcript.push_str(&format!("Completed: {}\n", completed));
        }
        transcript.push_str("\n--- Transcript ---\n\n");

        for (i, event) in self.session.events.iter().enumerate() {
            transcript.push_str(&format!(
                "[{:04}] {}\n",
                i,
                Self::event_to_transcript(event)
            ));
        }

        transcript
    }

    /// Convert event to transcript line
    fn event_to_transcript(event: &SessionEvent) -> String {
        match event {
            SessionEvent::System { timestamp, message } => {
                format!("{} [SYSTEM] {}", timestamp.format("%H:%M:%S"), message)
            }
            SessionEvent::Assistant {
                timestamp,
                content,
                model,
            } => {
                let model_str = model
                    .as_ref()
                    .map(|m| format!(" ({})", m))
                    .unwrap_or_default();
                format!(
                    "{} [ASSISTANT{}] {}",
                    timestamp.format("%H:%M:%S"),
                    model_str,
                    content
                )
            }
            SessionEvent::ToolUse {
                timestamp,
                tool_name,
                parameters,
            } => {
                format!(
                    "{} [TOOL USE] {} with params: {}",
                    timestamp.format("%H:%M:%S"),
                    tool_name,
                    serde_json::to_string(parameters).unwrap_or_else(|_| "{}".to_string())
                )
            }
            SessionEvent::ToolResult {
                timestamp,
                tool_name,
                duration_ms,
                ..
            } => {
                let duration_str = duration_ms
                    .map(|d| format!(" ({}ms)", d))
                    .unwrap_or_default();
                format!(
                    "{} [TOOL RESULT] {}{}",
                    timestamp.format("%H:%M:%S"),
                    tool_name,
                    duration_str
                )
            }
            SessionEvent::Error {
                timestamp,
                error_type,
                message,
            } => {
                format!(
                    "{} [ERROR] {}: {}",
                    timestamp.format("%H:%M:%S"),
                    error_type,
                    message
                )
            }
        }
    }
}

/// Playback speed for automatic replay
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PlaybackSpeed {
    Slow,     // 2000ms between events
    Normal,   // 1000ms between events
    Fast,     // 500ms between events
    VeryFast, // 100ms between events
    Instant,  // No delay
}

impl PlaybackSpeed {
    fn delay_ms(&self) -> u64 {
        match self {
            Self::Slow => 2000,
            Self::Normal => 1000,
            Self::Fast => 500,
            Self::VeryFast => 100,
            Self::Instant => 0,
        }
    }
}

/// Summary of a session for quick overview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub total_events: usize,
    pub tool_invocations: usize,
    pub errors: usize,
    pub duration_seconds: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::models::Session;

    fn create_test_session() -> Session {
        let mut session = Session {
            session_id: "test-session".to_string(),
            project_path: "/test/project".to_string(),
            jsonl_path: "/test/session.jsonl".to_string(),
            started_at: Utc::now(),
            completed_at: None,
            model: Some("claude-3-opus".to_string()),
            events: Vec::new(),
            total_input_tokens: 1000,
            total_output_tokens: 500,
            total_cache_tokens: 100,
            tool_invocations: Vec::new(),
        };

        // Add some test events
        session.events.push(SessionEvent::System {
            timestamp: Utc::now(),
            message: "Session started".to_string(),
        });

        session.events.push(SessionEvent::ToolUse {
            timestamp: Utc::now(),
            tool_name: "Bash".to_string(),
            parameters: serde_json::json!({"command": "ls"}),
        });

        session
    }

    #[test]
    fn test_session_replay_navigation() {
        let session = create_test_session();
        let mut replay = SessionReplay::new(session);

        assert_eq!(replay.position(), 0);
        assert_eq!(replay.total_events(), 2);

        // Step forward
        let event = replay.step_forward();
        assert!(event.is_some());
        assert_eq!(replay.position(), 1);

        // Step backward
        let event = replay.step_backward();
        assert!(event.is_some());
        assert_eq!(replay.position(), 0);
    }

    #[test]
    fn test_bookmarks() {
        let session = create_test_session();
        let mut replay = SessionReplay::new(session);

        replay.step_forward();
        replay.set_bookmark("checkpoint");

        replay.reset();
        assert_eq!(replay.position(), 0);

        replay.jump_to_bookmark("checkpoint");
        assert_eq!(replay.position(), 1);
    }

    #[test]
    fn test_export_transcript() {
        let session = create_test_session();
        let replay = SessionReplay::new(session);

        let transcript = replay.export_transcript();
        assert!(transcript.contains("Session: test-session"));
        assert!(transcript.contains("[SYSTEM]"));
        assert!(transcript.contains("[TOOL USE]"));
    }
}
