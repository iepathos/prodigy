//! Session storage backend implementations

use super::{PersistedSession, SessionId};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;

/// Trait for session storage backends
#[async_trait]
pub trait SessionStorage: Send + Sync {
    /// Save a session
    async fn save(&self, session: &PersistedSession) -> Result<()>;
    
    /// Load a session
    async fn load(&self, id: &SessionId) -> Result<Option<PersistedSession>>;
    
    /// List all session IDs
    async fn list(&self) -> Result<Vec<SessionId>>;
    
    /// Delete a session
    async fn delete(&self, id: &SessionId) -> Result<()>;
}

/// File-based session storage
pub struct FileSessionStorage {
    base_path: PathBuf,
}

impl FileSessionStorage {
    /// Create new file storage
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Get path for a session
    fn session_path(&self, id: &SessionId) -> PathBuf {
        self.base_path.join(format!("{}.json", id))
    }
}

#[async_trait]
impl SessionStorage for FileSessionStorage {
    async fn save(&self, session: &PersistedSession) -> Result<()> {
        // Ensure directory exists
        fs::create_dir_all(&self.base_path).await?;
        
        // Serialize and save
        let json = serde_json::to_string_pretty(session)?;
        let path = self.session_path(&session.id);
        fs::write(&path, json).await?;
        
        Ok(())
    }
    
    async fn load(&self, id: &SessionId) -> Result<Option<PersistedSession>> {
        let path = self.session_path(id);
        
        if !path.exists() {
            return Ok(None);
        }
        
        let json = fs::read_to_string(&path).await?;
        let session = serde_json::from_str(&json)?;
        
        Ok(Some(session))
    }
    
    async fn list(&self) -> Result<Vec<SessionId>> {
        let mut sessions = Vec::new();
        
        if !self.base_path.exists() {
            return Ok(sessions);
        }
        
        let mut entries = fs::read_dir(&self.base_path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    sessions.push(SessionId::from_string(stem.to_string()));
                }
            }
        }
        
        Ok(sessions)
    }
    
    async fn delete(&self, id: &SessionId) -> Result<()> {
        let path = self.session_path(id);
        
        if path.exists() {
            fs::remove_file(&path).await?;
        }
        
        Ok(())
    }
}

/// In-memory storage for testing
#[cfg(test)]
pub struct InMemoryStorage {
    sessions: std::sync::Mutex<std::collections::HashMap<SessionId, PersistedSession>>,
}

#[cfg(test)]
impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl SessionStorage for InMemoryStorage {
    async fn save(&self, session: &PersistedSession) -> Result<()> {
        self.sessions.lock().unwrap().insert(session.id.clone(), session.clone());
        Ok(())
    }
    
    async fn load(&self, id: &SessionId) -> Result<Option<PersistedSession>> {
        Ok(self.sessions.lock().unwrap().get(id).cloned())
    }
    
    async fn list(&self) -> Result<Vec<SessionId>> {
        Ok(self.sessions.lock().unwrap().keys().cloned().collect())
    }
    
    async fn delete(&self, id: &SessionId) -> Result<()> {
        self.sessions.lock().unwrap().remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_storage() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSessionStorage::new(temp_dir.path().to_path_buf());
        
        // Create test session
        let session = PersistedSession {
            id: SessionId::from_string("test-session".to_string()),
            config: Default::default(),
            state: crate::session::SessionState::Created,
            events: vec![],
            checkpoints: vec![],
        };
        
        // Save
        storage.save(&session).await.unwrap();
        
        // Load
        let loaded = storage.load(&session.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, session.id);
        
        // List
        let sessions = storage.list().await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0], session.id);
        
        // Delete
        storage.delete(&session.id).await.unwrap();
        let loaded = storage.load(&session.id).await.unwrap();
        assert!(loaded.is_none());
    }
}