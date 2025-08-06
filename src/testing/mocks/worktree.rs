//! Mock worktree manager for testing

use crate::worktree::{WorktreeSession, WorktreeStatus};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Builder for creating configured mock worktree managers
pub struct MockWorktreeManagerBuilder {
    session: Option<WorktreeSession>,
    fail_on_create: Option<String>,
    fail_on_cleanup: Option<String>,
}

impl MockWorktreeManagerBuilder {
    pub fn new() -> Self {
        Self {
            session: None,
            fail_on_create: None,
            fail_on_cleanup: None,
        }
    }

    pub fn with_session(mut self, session: WorktreeSession) -> Self {
        self.session = Some(session);
        self
    }

    pub fn fail_on_create(mut self, error: &str) -> Self {
        self.fail_on_create = Some(error.to_string());
        self
    }

    pub fn fail_on_cleanup(mut self, error: &str) -> Self {
        self.fail_on_cleanup = Some(error.to_string());
        self
    }

    pub fn build(self) -> MockWorktreeManager {
        MockWorktreeManager {
            session: Arc::new(Mutex::new(self.session)),
            fail_on_create: self.fail_on_create,
            fail_on_cleanup: self.fail_on_cleanup,
            create_count: Arc::new(Mutex::new(0)),
            cleanup_count: Arc::new(Mutex::new(0)),
        }
    }
}

/// Mock implementation of WorktreeManager for testing
pub struct MockWorktreeManager {
    session: Arc<Mutex<Option<WorktreeSession>>>,
    fail_on_create: Option<String>,
    fail_on_cleanup: Option<String>,
    create_count: Arc<Mutex<usize>>,
    cleanup_count: Arc<Mutex<usize>>,
}

impl MockWorktreeManager {
    pub fn new() -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
            fail_on_create: None,
            fail_on_cleanup: None,
            create_count: Arc::new(Mutex::new(0)),
            cleanup_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn builder() -> MockWorktreeManagerBuilder {
        MockWorktreeManagerBuilder::new()
    }

    pub fn get_create_count(&self) -> usize {
        *self.create_count.lock().unwrap()
    }

    pub fn get_cleanup_count(&self) -> usize {
        *self.cleanup_count.lock().unwrap()
    }
}

impl MockWorktreeManager {
    pub async fn create_session(&self) -> Result<WorktreeSession> {
        *self.create_count.lock().unwrap() += 1;
        
        if let Some(error) = &self.fail_on_create {
            return Err(anyhow::anyhow!(error.clone()));
        }
        
        if let Some(session) = &*self.session.lock().unwrap() {
            Ok(session.clone())
        } else {
            // Return a default session
            Ok(WorktreeSession {
                name: "test-worktree".to_string(),
                path: PathBuf::from("/tmp/worktree"),
                branch: "test-branch".to_string(),
                created_at: chrono::Utc::now(),
            })
        }
    }

    pub async fn cleanup_session(&self, _name: &str) -> Result<()> {
        *self.cleanup_count.lock().unwrap() += 1;
        
        if let Some(error) = &self.fail_on_cleanup {
            return Err(anyhow::anyhow!(error.clone()));
        }
        
        Ok(())
    }

    pub async fn list_sessions(&self) -> Result<Vec<WorktreeSession>> {
        if let Some(session) = &*self.session.lock().unwrap() {
            Ok(vec![session.clone()])
        } else {
            Ok(vec![])
        }
    }

    pub async fn get_session(&self, _name: &str) -> Result<Option<WorktreeSession>> {
        Ok(self.session.lock().unwrap().clone())
    }

    pub async fn cleanup_all(&self) -> Result<()> {
        *self.cleanup_count.lock().unwrap() += 1;
        
        if let Some(error) = &self.fail_on_cleanup {
            return Err(anyhow::anyhow!(error.clone()));
        }
        
        Ok(())
    }
}