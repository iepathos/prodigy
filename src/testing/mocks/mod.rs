//! Mock implementations for testing
//!
//! This module provides comprehensive mock implementations for all external dependencies.

pub mod claude;
pub mod config;
pub mod fs;
pub mod git;
pub mod session;
pub mod subprocess;
pub mod unified_session;
pub mod workflow;
pub mod worktree;

pub use claude::*;
pub use config::*;
pub use fs::*;
pub use git::*;
pub use session::*;
pub use subprocess::{CommandExecutorMock, MockSubprocessManager, MockSubprocessManagerBuilder};
pub use unified_session::*;
pub use workflow::*;
pub use worktree::*;
