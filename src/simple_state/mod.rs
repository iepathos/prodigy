//! Simple JSON-based state management
//!
//! This module provides a dead simple state management system using JSON files
//! instead of a complex database. State is human-readable, git-friendly, and
//! requires zero configuration.
//!
//! # Features
//!
//! - **JSON Persistence**: Automatic serialization/deserialization of state
//! - **Human-Readable**: State files can be read and edited manually
//! - **Git-Friendly**: Text-based files that work well with version control
//! - **Zero Configuration**: No setup required, just start using
//! - **Atomic Operations**: Safe concurrent state updates
//!
//! # Architecture
//!
//! The state management system consists of:
//! - [`StateManager`] - Main interface for state operations  
//! - [`CacheManager`] - In-memory caching for performance
//! - [`ProjectState`] - Project-specific state container
//! - [`SessionRecord`] - Individual session tracking
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```rust
//! use mmm::simple_state::StateManager;
//! use std::path::PathBuf;
//!
//! # fn example() -> anyhow::Result<()> {
//! let mut state_manager = StateManager::new()?;
//! let project_path = PathBuf::from("/path/to/project");
//!
//! // Load or create project state
//! let mut state = state_manager.load_or_create_state(&project_path)?;
//! 
//! // Update state
//! state.last_run = Some(chrono::Utc::now());
//! state.total_runs += 1;
//!
//! // Save changes
//! state_manager.save_state(&project_path, &state)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Session Tracking
//!
//! ```rust
//! # use mmm::simple_state::{StateManager, SessionRecord};
//! # use std::path::PathBuf;
//! # fn example() -> anyhow::Result<()> {
//! let mut state_manager = StateManager::new()?;
//!
//! // Create session record
//! let session = SessionRecord {
//!     session_id: "session-123".to_string(),
//!     started_at: chrono::Utc::now(),
//!     completed_at: None,
//!     iterations: 3,
//!     files_changed: 5,
//!     summary: "Fixed linting issues".to_string(),
//! };
//!
//! // Save session history
//! state_manager.save_session_record(&session)?;
//! # Ok(())
//! # }
//! ```

pub mod cache;
pub mod state;
#[cfg(test)]
mod tests;
pub mod types;

pub use cache::CacheManager;
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
