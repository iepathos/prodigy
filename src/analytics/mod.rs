//! Claude session correlation and analytics module
//!
//! Provides comprehensive analytics for Claude sessions by correlating real-time
//! events with historical session logs from ~/.claude/projects/

pub mod api_server;
pub mod engine;
pub mod models;
pub mod persistence;
pub mod replay;
pub mod session_watcher;

pub use api_server::AnalyticsApiServer;
pub use engine::AnalyticsEngine;
pub use models::*;
pub use persistence::AnalyticsDatabase;
pub use replay::SessionReplay;
pub use session_watcher::SessionWatcher;
