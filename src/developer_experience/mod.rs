//! Developer experience module for delightful code improvement interactions
//!
//! This module provides beautiful progress displays, interactive features,
//! smart suggestions, and celebration mechanics to make code improvement
//! feel magical rather than mechanical.

pub mod progress;
pub mod summary;
pub mod interactive;
pub mod error_handling;
pub mod suggestions;
pub mod celebration;
pub mod shell;
pub mod performance;

pub use progress::{ProgressDisplay, Phase};
pub use summary::{ResultSummary, QualityScore, ImpactMetrics};
pub use interactive::{LivePreview, InterruptHandler, ChangeDecision};
pub use error_handling::{ErrorHandler, RollbackManager};
pub use suggestions::{SmartHelper, NextAction, ContextualHelp};
pub use celebration::{Achievement, AchievementManager, Streak, SuccessMessage};
pub use shell::{ShellIntegration, Completions};
pub use performance::{FastStartup, IncrementalProcessor};

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