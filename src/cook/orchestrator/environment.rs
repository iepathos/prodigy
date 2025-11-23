//! Environment-based dependency injection for orchestrator
//!
//! Provides OrchestratorEnv struct for explicit dependency management,
//! enabling mockless testing and clear separation of I/O from business logic.

use crate::abstractions::git::GitOperations;
use crate::cook::execution::{ClaudeExecutor, CommandExecutor};
use crate::cook::interaction::UserInteraction;
use crate::cook::session::SessionManager;
use crate::subprocess::SubprocessManager;
use std::sync::Arc;

/// Environment providing all orchestrator dependencies
///
/// This struct captures all external dependencies needed by the orchestrator,
/// enabling:
/// - Explicit dependency passing (no hidden state)
/// - Mock environments for testing
/// - Clear I/O boundaries
#[derive(Clone)]
pub struct OrchestratorEnv {
    /// Session management
    pub session_manager: Arc<dyn SessionManager>,
    /// Command execution
    pub command_executor: Arc<dyn CommandExecutor>,
    /// Claude command execution
    pub claude_executor: Arc<dyn ClaudeExecutor>,
    /// User interaction (prompts, confirmations)
    pub user_interaction: Arc<dyn UserInteraction>,
    /// Git operations
    pub git_operations: Arc<dyn GitOperations>,
    /// Subprocess management
    pub subprocess_manager: SubprocessManager,
}

impl OrchestratorEnv {
    /// Create a new environment with provided dependencies
    pub fn new(
        session_manager: Arc<dyn SessionManager>,
        command_executor: Arc<dyn CommandExecutor>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        subprocess_manager: SubprocessManager,
    ) -> Self {
        Self {
            session_manager,
            command_executor,
            claude_executor,
            user_interaction,
            git_operations,
            subprocess_manager,
        }
    }
}

impl std::fmt::Debug for OrchestratorEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrchestratorEnv")
            .field("session_manager", &"<dyn SessionManager>")
            .field("command_executor", &"<dyn CommandExecutor>")
            .field("claude_executor", &"<dyn ClaudeExecutor>")
            .field("user_interaction", &"<dyn UserInteraction>")
            .field("git_operations", &"<dyn GitOperations>")
            .field("subprocess_manager", &self.subprocess_manager)
            .finish()
    }
}

#[cfg(test)]
pub mod mocks {
    //! Mock implementations for testing
    use crate::cook::session::{SessionManager, SessionState, SessionSummary, SessionUpdate};
    use async_trait::async_trait;
    use std::path::Path;

    /// Mock session manager for testing
    #[derive(Debug, Clone)]
    pub struct MockSessionManager;

    #[async_trait]
    impl SessionManager for MockSessionManager {
        async fn start_session(&self, _session_id: &str) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn update_session(&self, _update: SessionUpdate) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn complete_session(&self) -> Result<SessionSummary, anyhow::Error> {
            Ok(SessionSummary {
                iterations: 0,
                files_changed: 0,
            })
        }

        fn get_state(&self) -> Result<SessionState, anyhow::Error> {
            use std::path::PathBuf;
            Ok(SessionState::new(
                "test-session".to_string(),
                PathBuf::from("/tmp"),
            ))
        }

        async fn save_state(&self, _path: &Path) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn load_state(&self, _path: &Path) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn load_session(&self, _session_id: &str) -> Result<SessionState, anyhow::Error> {
            use std::path::PathBuf;
            Ok(SessionState::new(
                _session_id.to_string(),
                PathBuf::from("/tmp"),
            ))
        }

        async fn save_checkpoint(&self, _state: &SessionState) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn list_resumable(
            &self,
        ) -> Result<Vec<crate::cook::session::SessionInfo>, anyhow::Error> {
            Ok(vec![])
        }

        async fn get_last_interrupted(&self) -> Result<Option<String>, anyhow::Error> {
            Ok(None)
        }
    }
}

#[cfg(test)]
impl OrchestratorEnv {
    /// Create a test environment with mock implementations
    ///
    /// NOTE: This is a partial implementation. Full mock environment
    /// requires implementing all trait methods which is deferred for now.
    /// Tests should use Effect::pure() for testing pure logic without I/O.
    #[allow(dead_code)]
    pub fn test() -> Self {
        todo!("Full mock environment implementation deferred - use Effect::pure() for testing")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_session_manager() {
        use mocks::MockSessionManager;
        let manager = MockSessionManager;
        // Test start session
        assert!(manager.start_session("test-123").await.is_ok());
        // Test load session
        assert!(manager.load_session("test-123").await.is_ok());
    }
}
