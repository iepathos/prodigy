use clap::Args;

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

    /// Focus directive for improvements
    pub focus: Option<String>,
}
