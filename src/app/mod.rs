//! Application module
//!
//! This module contains application-level functionality including:
//! - Configuration handling
//! - Logging setup
//! - Runtime initialization
//! - Application state management

pub mod config;
pub mod error_handling;
pub mod logging;
pub mod runtime;

// Re-export main application functions
pub use config::AppConfig;
pub use error_handling::handle_fatal_error;
pub use logging::init_logging;
pub use runtime::{check_and_migrate_storage, initialize_app};
