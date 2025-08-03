use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;
use tracing::{debug, error, trace};

/// Cook your code to perfection with zero configuration
#[derive(Parser)]
#[command(name = "mmm")]
#[command(about = "mmm - Cook your code to perfection automatically", long_about = None)]
struct Cli {
    /// Enable verbose output (-v for debug, -vv for trace, -vvv for all)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Cook your code to perfection (make it better)
    #[command(name = "cook", alias = "improve")]
    Cook {
        /// Playbook file to execute (required)
        #[arg(value_name = "PLAYBOOK", help = "Playbook file defining the workflow")]
        playbook: PathBuf,

        /// Repository path to run in (defaults to current directory)
        #[arg(
            short = 'p',
            long,
            value_name = "PATH",
            help = "Repository path to run in"
        )]
        path: Option<PathBuf>,

        /// Maximum number of iterations to run (default: 10)
        #[arg(short = 'n', long, default_value = "10")]
        max_iterations: u32,

        /// Run in an isolated git worktree for parallel execution
        #[arg(short = 'w', long)]
        worktree: bool,

        /// File patterns to map over
        #[arg(long, value_name = "PATTERN")]
        map: Vec<String>,

        /// Direct arguments to pass to commands
        #[arg(long, value_name = "VALUE")]
        args: Vec<String>,

        /// Stop on first failure when processing multiple files
        #[arg(long)]
        fail_fast: bool,

        /// Automatically answer yes to all prompts
        #[arg(short = 'y', long = "yes")]
        auto_accept: bool,
        /// Enable metrics tracking
        #[arg(long)]
        metrics: bool,

        /// Resume an interrupted session
        #[arg(long, value_name = "SESSION_ID", conflicts_with = "worktree")]
        resume: Option<String>,

        /// Skip the initial project analysis phase
        #[arg(long)]
        skip_analysis: bool,
    },
    /// Manage git worktrees for parallel MMM sessions
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
    /// Initialize MMM commands in your project
    Init {
        /// Force overwrite existing commands
        #[arg(short, long)]
        force: bool,

        /// Specific commands to install (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        commands: Option<Vec<String>>,

        /// Directory to initialize (defaults to current)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// Analyze project structure and gather metrics
    Analyze {
        /// Output format (json, pretty, summary)
        #[arg(short = 'o', long, default_value = "summary")]
        output: String,

        /// Save results to .mmm directory
        #[arg(short = 's', long)]
        save: bool,

        /// Path to analyze (defaults to current directory)
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,

        /// Run cargo-tarpaulin for accurate coverage before analysis
        #[arg(long)]
        run_coverage: bool,

        /// Skip automatic git commit after analysis
        #[arg(long)]
        no_commit: bool,
    },
}

#[derive(Subcommand)]
enum WorktreeCommands {
    /// List active MMM worktrees
    List,
    /// Merge a worktree's changes to the default branch (main or master)
    Merge {
        /// Name of the worktree to merge
        name: Option<String>,
        /// Merge all MMM worktrees
        #[arg(long)]
        all: bool,
    },
    /// Clean up completed or abandoned worktrees
    Clean {
        /// Clean up all MMM worktrees
        #[arg(long)]
        all: bool,
        /// Name of specific worktree to clean
        name: Option<String>,
        /// Force removal even if there are untracked or modified files
        #[arg(long)]
        force: bool,
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
        Some(Commands::Cook {
            playbook,
            path,
            max_iterations,
            worktree,
            map,
            args,
            fail_fast,
            auto_accept,
            metrics,
            resume,
            skip_analysis,
        }) => {
            // Check if user used the 'improve' alias (deprecated as of v0.3.0)
            // NOTE: Remove this deprecation warning in v1.0.0
            let cli_args: Vec<String> = std::env::args().collect();
            if cli_args.len() > 1 && cli_args[1] == "improve" {
                eprintln!("Note: 'improve' has been renamed to 'cook'. Please use 'mmm cook' in the future.");
                eprintln!("The 'improve' alias will be removed in a future version.\n");
            }

            let cook_cmd = mmm::cook::command::CookCommand {
                playbook,
                path,
                max_iterations,
                worktree,
                map,
                args,
                fail_fast,
                auto_accept,
                metrics,
                resume,
                skip_analysis,
            };
            mmm::cook::cook(cook_cmd).await
        }
        Some(Commands::Worktree { command }) => run_worktree_command(command).await,
        Some(Commands::Init {
            force,
            commands,
            path,
        }) => {
            let init_cmd = mmm::init::command::InitCommand {
                force,
                commands,
                path,
            };
            mmm::init::run(init_cmd).await
        }
        Some(Commands::Analyze {
            output,
            save,
            path,
            run_coverage,
            no_commit,
        }) => {
            let analyze_cmd = mmm::analyze::command::AnalyzeCommand {
                output,
                save,
                verbose: cli.verbose > 0,
                path,
                run_coverage,
                no_commit,
            };
            mmm::analyze::run(analyze_cmd).await
        }
        None => {
            // Display help when no command is provided (following CLI conventions)
            let mut cmd = Cli::command();
            let _ = cmd.print_help();
            println!(); // Add blank line for better formatting
            return;
        }
    };

    if let Err(e) = result {
        error!("Fatal error: {}", e);
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

/// Display a single worktree session with its state and metadata
fn display_worktree_session(
    session: &mmm::worktree::WorktreeSession,
    worktree_manager: &mmm::worktree::WorktreeManager,
) -> anyhow::Result<()> {
    let state_file = worktree_manager
        .base_dir
        .join(".metadata")
        .join(format!("{}.json", session.name));

    if let Ok(state_json) = std::fs::read_to_string(&state_file) {
        if let Ok(state) = serde_json::from_str::<mmm::worktree::WorktreeState>(&state_json) {
            let status_emoji = match state.status {
                mmm::worktree::WorktreeStatus::InProgress => "🔄",
                mmm::worktree::WorktreeStatus::Completed => "✅",
                mmm::worktree::WorktreeStatus::Merged => "🔀",
                mmm::worktree::WorktreeStatus::Failed => "❌",
                mmm::worktree::WorktreeStatus::Abandoned => "⚠️",
                mmm::worktree::WorktreeStatus::Interrupted => "⏸️",
            };

            println!(
                "  {} {} - {:?} ({}/{})",
                status_emoji,
                session.name,
                state.status,
                state.iterations.completed,
                state.iterations.max
            );
        } else {
            // Fallback to old display for sessions without valid state
            display_worktree_session_legacy(session);
        }
    } else {
        // Fallback to old display for sessions without state files
        display_worktree_session_legacy(session);
    }

    Ok(())
}

/// Display a worktree session using legacy format
fn display_worktree_session_legacy(session: &mmm::worktree::WorktreeSession) {
    println!("  {} - {}", session.name, session.path.display());
}

/// Handle the list command for worktrees
async fn handle_list_command(
    worktree_manager: &mmm::worktree::WorktreeManager,
) -> anyhow::Result<()> {
    let sessions = worktree_manager.list_sessions().await?;
    if sessions.is_empty() {
        println!("No active MMM worktrees found.");
    } else {
        println!("Active MMM worktrees:");
        for session in sessions {
            display_worktree_session(&session, worktree_manager)?;
        }
    }
    Ok(())
}

/// Handle the merge command for worktrees
async fn handle_merge_command(
    worktree_manager: &mmm::worktree::WorktreeManager,
    name: Option<String>,
    all: bool,
) -> anyhow::Result<()> {
    if all {
        // Merge all worktrees
        let sessions = worktree_manager.list_sessions().await?;
        if sessions.is_empty() {
            println!("No active MMM worktrees found to merge.");
        } else {
            println!("Found {} worktree(s) to merge", sessions.len());
            for session in sessions {
                println!("\n📝 Merging worktree '{}'...", session.name);
                match worktree_manager.merge_session(&session.name).await {
                    Ok(_) => {
                        println!("✅ Successfully merged worktree '{}'", session.name);
                        // Automatically clean up successfully merged worktrees when using --all
                        if let Err(e) = worktree_manager.cleanup_session(&session.name, true).await
                        {
                            eprintln!(
                                "⚠️ Warning: Failed to clean up worktree '{}': {}",
                                session.name, e
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to merge worktree '{}': {}", session.name, e);
                        eprintln!("   Skipping cleanup for failed merge.");
                    }
                }
            }
            println!("\n✅ Bulk merge operation completed");
        }
    } else if let Some(name) = name {
        // Single worktree merge
        println!("Merging worktree '{name}'...");
        worktree_manager.merge_session(&name).await?;
        println!("✅ Successfully merged worktree '{name}'");

        // Ask if user wants to clean up the worktree
        println!("Would you like to clean up the worktree? (y/N)");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() == "y" {
            worktree_manager.cleanup_session(&name, true).await?;
            println!("✅ Worktree cleaned up");
        }
    } else {
        anyhow::bail!("Either --all or a worktree name must be specified");
    }
    Ok(())
}

/// Handle the clean command for worktrees
async fn handle_clean_command(
    worktree_manager: &mmm::worktree::WorktreeManager,
    name: Option<String>,
    all: bool,
    force: bool,
) -> anyhow::Result<()> {
    if all {
        println!("Cleaning up all MMM worktrees...");
        worktree_manager.cleanup_all_sessions(force).await?;
        println!("✅ All worktrees cleaned up");
    } else if let Some(name) = name {
        println!("Cleaning up worktree '{name}'...");
        worktree_manager.cleanup_session(&name, force).await?;
        println!("✅ Worktree '{name}' cleaned up");
    } else {
        anyhow::bail!("Either --all or a worktree name must be specified");
    }
    Ok(())
}

async fn run_worktree_command(command: WorktreeCommands) -> anyhow::Result<()> {
    use mmm::subprocess::SubprocessManager;
    use mmm::worktree::WorktreeManager;

    let subprocess = SubprocessManager::production();
    let worktree_manager = WorktreeManager::new(std::env::current_dir()?, subprocess)?;

    match command {
        WorktreeCommands::List => handle_list_command(&worktree_manager).await,
        WorktreeCommands::Merge { name, all } => {
            handle_merge_command(&worktree_manager, name, all).await
        }
        WorktreeCommands::Clean { all, name, force } => {
            handle_clean_command(&worktree_manager, name, all, force).await
        }
    }
}
