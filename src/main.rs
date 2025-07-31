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

        /// Focus directive for analysis (e.g., "user experience", "performance")
        #[arg(short = 'f', long)]
        focus: Option<String>,

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
        /// Type of analysis to run (context, metrics, all)
        #[arg(short = 't', long, default_value = "all")]
        analysis_type: String,

        /// Output format (json, pretty, summary)
        #[arg(short = 'o', long, default_value = "summary")]
        output: String,

        /// Save results to .mmm/context directory
        #[arg(short = 's', long)]
        save: bool,

        /// Path to analyze (defaults to current directory)
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,

        /// Run cargo-tarpaulin for accurate coverage before analysis
        #[arg(long)]
        run_coverage: bool,
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
            focus,
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
            // Check if user used the deprecated 'improve' alias
            let cli_args: Vec<String> = std::env::args().collect();
            if cli_args.len() > 1 && cli_args[1] == "improve" {
                eprintln!("Note: 'improve' has been renamed to 'cook'. Please use 'mmm cook' in the future.");
                eprintln!("The 'improve' alias will be removed in a future version.\n");
            }

            let cook_cmd = mmm::cook::command::CookCommand {
                playbook,
                path,
                focus,
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
            mmm::cook::run_with_verbosity(cook_cmd, cli.verbose).await
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
            analysis_type,
            output,
            save,
            path,
            run_coverage,
        }) => {
            let analyze_cmd = mmm::analyze::command::AnalyzeCommand {
                analysis_type,
                output,
                save,
                verbose: cli.verbose > 0,
                path,
                run_coverage,
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
                    // Load state for each session
                    let state_file = worktree_manager
                        .base_dir
                        .join(".metadata")
                        .join(format!("{}.json", session.name));
                    if let Ok(state_json) = std::fs::read_to_string(&state_file) {
                        if let Ok(state) =
                            serde_json::from_str::<mmm::worktree::WorktreeState>(&state_json)
                        {
                            let focus_str = state
                                .focus
                                .as_deref()
                                .map(|f| format!(" - {f}"))
                                .unwrap_or_else(|| " - no focus".to_string());

                            let status_emoji = match state.status {
                                mmm::worktree::WorktreeStatus::InProgress => "🔄",
                                mmm::worktree::WorktreeStatus::Completed => "✅",
                                mmm::worktree::WorktreeStatus::Merged => "🔀",
                                mmm::worktree::WorktreeStatus::Failed => "❌",
                                mmm::worktree::WorktreeStatus::Abandoned => "⚠️",
                                mmm::worktree::WorktreeStatus::Interrupted => "⏸️",
                            };

                            println!(
                                "  {} {} - {:?}{} ({}/{})",
                                status_emoji,
                                session.name,
                                state.status,
                                focus_str,
                                state.iterations.completed,
                                state.iterations.max
                            );
                        } else {
                            // Fallback to old display for sessions without valid state
                            let focus_str = session
                                .focus
                                .map(|f| format!(" (focus: {f})"))
                                .unwrap_or_default();
                            println!(
                                "  {} - {}{}",
                                session.name,
                                session.path.display(),
                                focus_str
                            );
                        }
                    } else {
                        // Fallback to old display for sessions without state files
                        let focus_str = session
                            .focus
                            .map(|f| format!(" (focus: {f})"))
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
        }
        WorktreeCommands::Merge { name, all } => {
            if all {
                // Merge all worktrees
                let sessions = worktree_manager.list_sessions()?;
                if sessions.is_empty() {
                    println!("No active MMM worktrees found to merge.");
                } else {
                    println!("Found {} worktree(s) to merge", sessions.len());
                    for session in sessions {
                        println!("\n📝 Merging worktree '{}'...", session.name);
                        match worktree_manager.merge_session(&session.name) {
                            Ok(_) => {
                                println!("✅ Successfully merged worktree '{}'", session.name);
                                // Automatically clean up successfully merged worktrees when using --all
                                if let Err(e) =
                                    worktree_manager.cleanup_session(&session.name, true)
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
                worktree_manager.merge_session(&name)?;
                println!("✅ Successfully merged worktree '{name}'");

                // Ask if user wants to clean up the worktree
                println!("Would you like to clean up the worktree? (y/N)");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if input.trim().to_lowercase() == "y" {
                    worktree_manager.cleanup_session(&name, true)?;
                    println!("✅ Worktree cleaned up");
                }
            } else {
                eprintln!("Error: Either --all or a worktree name must be specified");
                std::process::exit(1);
            }
        }
        WorktreeCommands::Clean { all, name, force } => {
            if all {
                println!("Cleaning up all MMM worktrees...");
                worktree_manager.cleanup_all_sessions(force)?;
                println!("✅ All worktrees cleaned up");
            } else if let Some(name) = name {
                println!("Cleaning up worktree '{name}'...");
                worktree_manager.cleanup_session(&name, force)?;
                println!("✅ Worktree '{name}' cleaned up");
            } else {
                eprintln!("Error: Either --all or a worktree name must be specified");
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
