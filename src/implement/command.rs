use clap::Args;

/// Command-line arguments for the implement command
///
/// This struct represents the configuration options available when running
/// `mmm implement` to batch implement pre-written specifications through Claude CLI integration.
#[derive(Debug, Args, Clone)]
pub struct ImplementCommand {
    /// Specification files to implement (supports glob patterns)
    ///
    /// One or more paths to specification files. Supports glob patterns like
    /// "specs/*.md" or "specs/pending/*.md". Each spec will be implemented
    /// sequentially using the implement-spec → lint cycle.
    #[arg(required = true)]
    pub spec_files: Vec<String>,

    /// Run in an isolated git worktree for parallel execution
    #[arg(short = 'w', long)]
    pub worktree: bool,

    /// Show detailed progress
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Simulate execution without making changes
    ///
    /// Shows which specifications would be implemented without actually
    /// calling Claude CLI or making any file modifications.
    #[arg(long)]
    pub dry_run: bool,

    /// Maximum number of iterations per spec (default: 10)
    ///
    /// Controls how many implement-spec → lint cycles to run for each
    /// specification before moving to the next one.
    #[arg(short = 'n', long, default_value = "10")]
    pub max_iterations: u32,

    /// Stop on first failure
    ///
    /// By default, batch implementation continues even if one spec fails.
    /// This flag causes the process to stop immediately on any failure.
    #[arg(long)]
    pub fail_fast: bool,
}
