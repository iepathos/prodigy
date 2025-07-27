use clap::Args;
use std::path::PathBuf;

/// Command-line arguments for the improve command
///
/// This struct represents the configuration options available when running
/// `mmm improve` to automatically enhance code quality through Claude CLI integration.
#[derive(Debug, Args, Clone)]
pub struct ImproveCommand {
    /// Show detailed progress
    #[arg(long)]
    pub show_progress: bool,

    /// Focus directive for improvements (e.g., "performance", "security", "testing")
    ///
    /// This optional parameter allows you to guide the code analysis towards
    /// specific areas of concern. Claude will naturally interpret the focus area and
    /// prioritize issues accordingly.
    #[arg(short = 'f', long)]
    pub focus: Option<String>,

    /// Path to configuration file
    ///
    /// Specify a custom configuration file path instead of using the default .mmm/config.toml.
    /// Supports both TOML and YAML formats. If not specified, the system will look for
    /// .mmm/config.toml in the project root.
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,

    /// Maximum number of iterations to run (default: 10)
    ///
    /// This limits how many improvement cycles will be executed.
    #[arg(short = 'n', long, default_value = "10")]
    pub max_iterations: u32,

    /// Run in an isolated git worktree for parallel execution
    ///
    /// Creates a separate git worktree to isolate this improvement session, allowing
    /// multiple MMM sessions to run concurrently without conflicts. Each session will
    /// work in its own branch and worktree, which can be merged back later.
    #[arg(short = 'w', long)]
    pub worktree: bool,
}
