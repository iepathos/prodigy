//! Developer experience module for delightful code improvement interactions
//!
//! This module provides beautiful progress displays, interactive features,
//! smart suggestions, and celebration mechanics to make code improvement
//! feel magical rather than mechanical.

pub mod celebration;
pub mod error_handling;
pub mod interactive;
pub mod performance;
pub mod progress;
pub mod shell;
pub mod suggestions;
pub mod summary;

pub use celebration::{Achievement, AchievementManager, Streak, SuccessMessage};
pub use error_handling::{ErrorHandler, RollbackManager};
pub use interactive::{ChangeDecision, InterruptHandler, LivePreview};
pub use performance::{FastStartup, IncrementalProcessor};
pub use progress::{Phase, ProgressDisplay};
pub use shell::{Completions, ShellIntegration};
pub use suggestions::{ContextualHelp, NextAction, SmartHelper};
pub use summary::{ImpactMetrics, QualityScore, ResultSummary};

use std::io::{self, IsTerminal};

/// Check if we're in an interactive terminal
pub fn is_interactive() -> bool {
    io::stdout().is_terminal()
}

/// Initialize the developer experience module
pub fn init() -> anyhow::Result<()> {
    // Initialize terminal colors if supported
    if is_interactive() {
        colored::control::set_override(true);
    }

    Ok(())
}
