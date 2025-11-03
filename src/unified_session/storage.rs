//! Storage operations for unified sessions

use super::state::{SessionId, UnifiedSession};
use crate::storage::GlobalStorage;
use anyhow::{Context, Result};
use tokio::fs;

/// Handles filesystem persistence for unified sessions
pub struct SessionStorage {
    storage: GlobalStorage,
}

impl SessionStorage {
    /// Create a new session storage handler
    pub fn new(storage: GlobalStorage) -> Self {
        Self { storage }
    }

    /// Save a session to storage
    pub async fn save(&self, session: &UnifiedSession) -> Result<()> {
        let sessions_dir = self.storage.base_dir().join("sessions");
        fs::create_dir_all(&sessions_dir)
            .await
            .context("Failed to create sessions directory")?;

        let session_file = sessions_dir.join(format!("{}.json", session.id.as_str()));
        let json = serde_json::to_string_pretty(session)?;
        fs::write(&session_file, json)
            .await
            .context("Failed to write session file")?;

        Ok(())
    }

    /// Load a session from storage
    pub async fn load(&self, id: &SessionId) -> Result<UnifiedSession> {
        let session_file = self
            .storage
            .base_dir()
            .join("sessions")
            .join(format!("{}.json", id.as_str()));

        if !session_file.exists() {
            return Err(anyhow::anyhow!("Session not found: {}", id.as_str()));
        }

        let json = fs::read_to_string(&session_file)
            .await
            .context("Failed to read session file")?;
        let session: UnifiedSession = serde_json::from_str(&json)?;

        Ok(session)
    }

    /// Delete a session from storage
    pub async fn delete(&self, id: &SessionId) -> Result<()> {
        let session_file = self
            .storage
            .base_dir()
            .join("sessions")
            .join(format!("{}.json", id.as_str()));

        if session_file.exists() {
            fs::remove_file(&session_file)
                .await
                .context("Failed to delete session file")?;
        }

        Ok(())
    }

    /// Load all sessions from storage
    pub async fn load_all(&self) -> Result<Vec<UnifiedSession>> {
        let sessions_dir = self.storage.base_dir().join("sessions");

        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        let mut entries = fs::read_dir(&sessions_dir)
            .await
            .context("Failed to read sessions directory")?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .context("Failed to read directory entry")?
        {
            if let Some(ext) = entry.path().extension() {
                if ext == "json" {
                    let json = fs::read_to_string(entry.path())
                        .await
                        .context("Failed to read session file")?;
                    if let Ok(session) = serde_json::from_str::<UnifiedSession>(&json) {
                        sessions.push(session);
                    }
                }
            }
        }

        Ok(sessions)
    }
}
