//! Simple JSON-based state management
//!
//! This module provides a dead simple state management system using JSON files
//! instead of a complex database. State is human-readable, git-friendly, and
//! requires zero configuration.

pub mod cache;
pub mod learning;
pub mod state;
pub mod types;
// pub mod migration; // Temporarily disabled due to SQLx compile requirements
#[path = "migration_simple.rs"]
pub mod migration;

#[cfg(test)]
mod tests;

pub use cache::CacheManager;
pub use learning::LearningManager;
pub use migration::migrate_from_sqlite;
pub use state::StateManager;
pub use types::*;

use anyhow::Result;
use std::path::PathBuf;

/// Initialize the state management system for a project
pub fn init() -> Result<()> {
    let root = PathBuf::from(".mmm");
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir_all(root.join("history"))?;
    std::fs::create_dir_all(root.join("cache"))?;
    Ok(())
}
