//! Specialized coordinators for cook module
//!
//! This module provides specialized coordinators that reduce the complexity
//! of the cook orchestrator by grouping related functionality.

mod environment;
mod execution;
mod session;
mod workflow;

pub use environment::{DefaultEnvironmentCoordinator, EnvironmentCoordinator, EnvironmentSetup};
pub use execution::{DefaultExecutionCoordinator, ExecutionCoordinator};
pub use session::{DefaultSessionCoordinator, SessionCoordinator};
pub use workflow::{DefaultWorkflowCoordinator, WorkflowContext, WorkflowCoordinator};
