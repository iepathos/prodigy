use clap::Args;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// Command-line arguments for the cook command
///
/// This struct represents the configuration options available when running
/// `prodigy cook` to automatically enhance code quality through Claude CLI integration.
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

    /// Maximum number of iterations to run (default: 1)
    ///
    /// This limits how many improvement cycles will be executed.
    #[arg(short = 'n', long, default_value = "1")]
    pub max_iterations: u32,

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
    /// throughout improvement iterations. Metrics are stored in .prodigy/metrics/ for
    /// historical analysis and trend tracking.
    #[arg(long)]
    pub metrics: bool,

    /// Resume an interrupted session
    ///
    /// Provide the session ID of an interrupted worktree to resume work from the last checkpoint.
    #[arg(long, value_name = "SESSION_ID")]
    pub resume: Option<String>,

    /// Increase output verbosity (-v verbose, -vv debug, -vvv trace)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count)]
    pub verbosity: u8,

    /// Decrease output verbosity (opposite of -v)
    #[arg(short = 'q', long = "quiet", conflicts_with = "verbosity")]
    pub quiet: bool,

    /// Dry-run mode - show what would be executed without running
    #[arg(long, help = "Preview commands without executing them")]
    pub dry_run: bool,

    /// Template parameters (not a CLI argument, populated from --param and --param-file)
    #[arg(skip)]
    pub params: HashMap<String, Value>,
}
