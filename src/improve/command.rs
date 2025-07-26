use clap::Args;
use std::path::PathBuf;

/// Command-line arguments for the improve command
///
/// This struct represents the configuration options available when running
/// `mmm improve` to automatically enhance code quality through Claude CLI integration.
#[derive(Debug, Args)]
pub struct ImproveCommand {
    /// Target quality score (default: 8.0)
    #[arg(long, default_value = "8.0")]
    pub target: f32,

    /// Show detailed progress
    #[arg(long)]
    pub show_progress: bool,

    /// Focus directive for improvements (e.g., "performance", "security", "testing")
    ///
    /// This optional parameter allows you to guide the initial code analysis towards
    /// specific areas of concern. Claude will naturally interpret the focus area and
    /// prioritize issues accordingly during the first iteration.
    #[arg(long)]
    pub focus: Option<String>,

    /// Path to configuration file
    ///
    /// Specify a custom configuration file path instead of using the default .mmm/config.toml.
    /// Supports both TOML and YAML formats. If not specified, the system will look for
    /// .mmm/config.toml in the project root.
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,
}
