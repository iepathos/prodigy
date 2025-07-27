use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{debug, error, trace};

/// Improve code quality with zero configuration
#[derive(Parser)]
#[command(name = "mmm")]
#[command(about = "Memento Mori Manager - Improve code quality automatically", long_about = None)]
struct Cli {
    /// Enable verbose output (-v for debug, -vv for trace, -vvv for all)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Improve code quality (default command)
    Improve {
        /// Target quality score (default: 8.0)
        #[arg(long, default_value = "8.0")]
        target: f32,

        /// Show detailed progress
        #[arg(long)]
        show_progress: bool,

        /// Focus directive for initial analysis (e.g., "user experience", "performance")
        #[arg(long)]
        focus: Option<String>,

        /// Path to configuration file
        #[arg(short = 'c', long)]
        config: Option<PathBuf>,

        /// Maximum number of iterations to run (default: 10)
        #[arg(short = 'n', long, default_value = "10")]
        max_iterations: u32,
    },
    /// Manage git worktrees for parallel MMM sessions
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
}

#[derive(Subcommand)]
enum WorktreeCommands {
    /// List active MMM worktrees
    List,
    /// Merge a worktree's changes to the current branch
    Merge {
        /// Name of the worktree to merge
        name: String,
        /// Target branch to merge into (default: current branch)
        #[arg(long)]
        target: Option<String>,
    },
    /// Clean up completed or abandoned worktrees
    Clean {
        /// Clean up all MMM worktrees
        #[arg(long)]
        all: bool,
        /// Name of specific worktree to clean
        name: Option<String>,
    },
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

    let result = match cli.command {
        Some(Commands::Improve {
            target,
            show_progress,
            focus,
            config,
            max_iterations,
        }) => run_improve(target, show_progress, focus, config, max_iterations).await,
        Some(Commands::Worktree { command }) => run_worktree_command(command).await,
        None => {
            // Default to improve command with default values
            run_improve(8.0, false, None, None, 10).await
        }
    };

    if let Err(e) = result {
        error!("Fatal error: {}", e);
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run_improve(
    target: f32,
    show_progress: bool,
    focus: Option<String>,
    config: Option<PathBuf>,
    max_iterations: u32,
) -> anyhow::Result<()> {
    let improve_cmd = mmm::improve::command::ImproveCommand {
        target,
        show_progress,
        focus,
        config,
        max_iterations,
    };
    mmm::improve::run(improve_cmd).await
}

async fn run_worktree_command(command: WorktreeCommands) -> anyhow::Result<()> {
    use mmm::worktree::WorktreeManager;

    let worktree_manager = WorktreeManager::new(std::env::current_dir()?)?;

    match command {
        WorktreeCommands::List => {
            let sessions = worktree_manager.list_sessions()?;
            if sessions.is_empty() {
                println!("No active MMM worktrees found.");
            } else {
                println!("Active MMM worktrees:");
                for session in sessions {
                    let focus_str = session
                        .focus
                        .map(|f| format!(" (focus: {})", f))
                        .unwrap_or_default();
                    println!(
                        "  {} - {}{}",
                        session.name,
                        session.path.display(),
                        focus_str
                    );
                }
            }
        }
        WorktreeCommands::Merge { name, target } => {
            println!(
                "Merging worktree '{}' into {}...",
                name,
                target.as_deref().unwrap_or("current branch")
            );
            worktree_manager.merge_session(&name, target.as_deref())?;
            println!("✅ Successfully merged worktree '{}'", name);

            // Ask if user wants to clean up the worktree
            println!("Would you like to clean up the worktree? (y/N)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim().to_lowercase() == "y" {
                worktree_manager.cleanup_session(&name)?;
                println!("✅ Worktree cleaned up");
            }
        }
        WorktreeCommands::Clean { all, name } => {
            if all {
                println!("Cleaning up all MMM worktrees...");
                worktree_manager.cleanup_all_sessions()?;
                println!("✅ All worktrees cleaned up");
            } else if let Some(name) = name {
                println!("Cleaning up worktree '{}'...", name);
                worktree_manager.cleanup_session(&name)?;
                println!("✅ Worktree '{}' cleaned up", name);
            } else {
                eprintln!("Error: Either --all or a worktree name must be specified");
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

