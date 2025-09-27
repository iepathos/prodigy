//! Worktree cleanup management for MapReduce jobs
//!
//! This module provides comprehensive cleanup mechanisms for agent worktrees,
//! ensuring proper resource management and preventing disk space bloat.

pub mod config;
pub mod coordinator;
pub mod error;
pub mod monitor;

#[cfg(test)]
mod tests;

pub use config::WorktreeCleanupConfig;
pub use coordinator::{CleanupTask, WorktreeCleanupCoordinator};
pub use error::{CleanupError, CleanupResult};
pub use monitor::{CleanupRecommendation, WorktreeMetrics, WorktreeResourceMonitor};
