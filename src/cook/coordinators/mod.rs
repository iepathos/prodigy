//! Specialized coordinators for cook module
//!
//! This module provides specialized coordinators that reduce the complexity
//! of the cook orchestrator by grouping related functionality.

mod environment;
mod execution;
mod session;
mod workflow;

#[cfg(test)]
mod environment_tests;
#[cfg(test)]
mod execution_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod workflow_tests;

pub use environment::{DefaultEnvironmentCoordinator, EnvironmentCoordinator, EnvironmentSetup};
pub use execution::{DefaultExecutionCoordinator, ExecutionCoordinator};
pub use session::{DefaultSessionCoordinator, SessionCoordinator};
pub use workflow::{DefaultWorkflowCoordinator, WorkflowContext, WorkflowCoordinator};
