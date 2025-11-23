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
