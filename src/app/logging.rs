//! Logging configuration and initialization
//!
//! This module handles all logging setup and configuration for the application.

use crate::app::config::AppConfig;
use tracing::{debug, trace};

/// Initialize tracing/logging for the application
pub fn init_logging(config: &AppConfig) {
    let log_level = config.log_level();

    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_target(config.verbose >= 2) // Show target module for -vv and above
        .with_thread_ids(config.verbose >= 3) // Show thread IDs for -vvv
        .with_line_number(config.verbose >= 3) // Show line numbers for -vvv
        .init();

    debug!("Prodigy started with verbosity level: {}", config.verbose);
    trace!("Full CLI args: {:?}", std::env::args().collect::<Vec<_>>());
}

/// Initialize tracing with just verbosity level (for backwards compatibility)
pub fn init_tracing(verbose: u8) {
    let config = AppConfig::default().with_metrics(false);
    let config = AppConfig { verbose, ..config };
    init_logging(&config);
}
