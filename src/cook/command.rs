use clap::Args;
use std::path::PathBuf;

/// Command-line arguments for the cook command
///
/// This struct represents the configuration options available when running
/// `mmm cook` to automatically enhance code quality through Claude CLI integration.
#[derive(Debug, Args, Clone)]
pub struct CookCommand {
    /// Playbook file to execute (required)
    #[arg(value_name = "PLAYBOOK", help = "Playbook file defining the workflow")]
    pub playbook: PathBuf,

    /// Repository path to run in (defaults to current directory)
    #[arg(
        short = 'p',
        long,
        value_name = "PATH",
        help = "Repository path to run in"
    )]
    pub path: Option<PathBuf>,


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

    /// File patterns to map over
    ///
    /// Run a separate improvement loop for each file matching the pattern(s).
    /// Supports glob patterns like "specs/*.md" or "src/**/*.rs".
    #[arg(long, value_name = "PATTERN")]
    pub map: Vec<String>,

    /// Direct arguments to pass to commands
    ///
    /// Arguments that will be passed to workflow commands via $ARG variable.
    /// Can be used with or without --map.
    #[arg(long, value_name = "VALUE")]
    pub args: Vec<String>,

    /// Stop on first failure
    ///
    /// When processing multiple files with --map, stop immediately on first error.
    /// By default, continues processing remaining files.
    #[arg(long)]
    pub fail_fast: bool,

    /// Automatically answer yes to all prompts
    ///
    /// Enables fully unattended operation by automatically accepting all interactive
    /// prompts, including worktree merge and deletion prompts. Useful for scripts,
    /// CI/CD pipelines, and other automation scenarios.
    #[arg(short = 'y', long = "yes")]
    pub auto_accept: bool,

    /// Enable metrics tracking
    ///
    /// Collect and track metrics for code quality, performance, complexity, and progress
    /// throughout improvement iterations. Metrics are stored in .mmm/metrics/ for
    /// historical analysis and trend tracking.
    #[arg(long)]
    pub metrics: bool,

    /// Resume an interrupted session
    ///
    /// Provide the session ID of an interrupted worktree to resume work from the last checkpoint.
    /// Cannot be used with --worktree flag.
    #[arg(long, value_name = "SESSION_ID", conflicts_with = "worktree")]
    pub resume: Option<String>,

    /// Skip the initial project analysis phase
    ///
    /// Bypasses the comprehensive project analysis that normally runs before Claude commands.
    /// Useful when you want to run Claude commands immediately without gathering context.
    #[arg(long)]
    pub skip_analysis: bool,
}
