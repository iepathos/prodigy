use clap::Parser;
use std::path::PathBuf;
use tracing::{debug, error, trace};

/// Improve code quality with zero configuration
#[derive(Parser)]
#[command(name = "mmm")]
#[command(about = "Memento Mori Manager - Improve code quality automatically", long_about = None)]
struct Cli {
    /// Target quality score (default: 8.0)
    #[arg(long, default_value = "8.0")]
    target: f32,

    /// Show detailed progress
    #[arg(long)]
    show_progress: bool,

    /// Enable verbose output (-v for debug, -vv for trace, -vvv for all)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Focus directive for initial analysis (e.g., "user experience", "performance")
    #[arg(long)]
    focus: Option<String>,

    /// Path to configuration file
    #[arg(short = 'c', long)]
    config: Option<PathBuf>,

    /// Maximum number of iterations to run (default: 10)
    #[arg(short = 'n', long, default_value = "10")]
    max_iterations: u32,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let log_level = match cli.verbose {
        0 => "info",
        1 => "debug",
        2 => "trace",
        _ => "trace,hyper=debug,tower=debug", // -vvv shows everything including dependencies
    };

    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_target(cli.verbose >= 2) // Show target module for -vv and above
        .with_thread_ids(cli.verbose >= 3) // Show thread IDs for -vvv
        .with_line_number(cli.verbose >= 3) // Show line numbers for -vvv
        .init();

    debug!("MMM started with verbosity level: {}", cli.verbose);
    trace!("Full CLI args: {:?}", std::env::args().collect::<Vec<_>>());

    // Run the improve command directly
    let improve_cmd = mmm::improve::command::ImproveCommand {
        target: cli.target,
        show_progress: cli.show_progress,
        focus: cli.focus,
        config: cli.config,
        max_iterations: cli.max_iterations,
    };

    if let Err(e) = mmm::improve::run(improve_cmd).await {
        error!("Fatal error: {}", e);
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
