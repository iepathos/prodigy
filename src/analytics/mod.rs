//! Claude session correlation and analytics module
//!
//! Provides comprehensive analytics for Claude sessions by correlating real-time
//! events with historical session logs from ~/.claude/projects/

pub mod engine;
pub mod models;
pub mod replay;
pub mod session_watcher;

pub use engine::AnalyticsEngine;
pub use models::*;
pub use replay::SessionReplay;
pub use session_watcher::SessionWatcher;
