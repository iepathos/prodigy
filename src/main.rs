use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;
use tracing::{debug, error, trace};

/// Cook your code to perfection with zero configuration
#[derive(Parser)]
#[command(name = "prodigy")]
#[command(about = "prodigy - Cook your code to perfection automatically", long_about = None)]
#[command(version)]
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

        /// Maximum number of iterations to run (default: 1)
        #[arg(short = 'n', long, default_value = "1")]
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
    },
    /// Manage git worktrees for parallel Prodigy sessions
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
    /// Initialize Prodigy commands in your project
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
}

#[derive(Subcommand)]
enum WorktreeCommands {
    /// List active Prodigy worktrees
    #[command(alias = "list")]
    Ls,
    /// Merge a worktree's changes to the default branch (main or master)
    Merge {
        /// Name of the worktree to merge
        name: Option<String>,
        /// Merge all Prodigy worktrees
        #[arg(long)]
        all: bool,
    },
    /// Clean up completed or abandoned worktrees
    Clean {
        /// Clean up all Prodigy worktrees
        #[arg(short = 'a', long)]
        all: bool,
        /// Name of specific worktree to clean
        name: Option<String>,
        /// Force removal even if there are untracked or modified files
        #[arg(short = 'f', long)]
        force: bool,
        /// Only clean up sessions that have been merged
        #[arg(long)]
        merged_only: bool,
    },
}

/// Determine the log level based on verbosity count
fn get_log_level(verbose: u8) -> &'static str {
    match verbose {
        0 => "info",
        1 => "debug",
        2 => "trace",
        _ => "trace,hyper=debug,tower=debug", // -vvv shows everything including dependencies
    }
}

/// Initialize the tracing subscriber with the appropriate settings
fn init_tracing(verbose: u8) {
    let log_level = get_log_level(verbose);

    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_target(verbose >= 2) // Show target module for -vv and above
        .with_thread_ids(verbose >= 3) // Show thread IDs for -vvv
        .with_line_number(verbose >= 3) // Show line numbers for -vvv
        .init();

    debug!("Prodigy started with verbosity level: {}", verbose);
    trace!("Full CLI args: {:?}", std::env::args().collect::<Vec<_>>());
}

/// Check if the deprecated 'improve' alias was used and emit a warning
fn check_deprecated_alias() {
    let cli_args: Vec<String> = std::env::args().collect();
    if cli_args.len() > 1 && cli_args[1] == "improve" {
        eprintln!(
            "Note: 'improve' has been renamed to 'cook'. Please use 'prodigy cook' in the future."
        );
        eprintln!("The 'improve' alias will be removed in a future version.\n");
    }
}

/// Execute the appropriate command based on CLI input
async fn execute_command(command: Option<Commands>) -> anyhow::Result<()> {
    match command {
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
        }) => {
            check_deprecated_alias();

            let cook_cmd = prodigy::cook::command::CookCommand {
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
                quiet: false,
                verbosity: 0,
            };
            prodigy::cook::cook(cook_cmd).await
        }
        Some(Commands::Worktree { command }) => run_worktree_command(command).await,
        Some(Commands::Init {
            force,
            commands,
            path,
        }) => {
            let init_cmd = prodigy::init::command::InitCommand {
                force,
                commands,
                path,
            };
            prodigy::init::run(init_cmd).await
        }
        None => {
            // Display help when no command is provided (following CLI conventions)
            let mut cmd = Cli::command();
            let _ = cmd.print_help();
            println!(); // Add blank line for better formatting
            Ok(())
        }
    }
}

/// Handle fatal errors and exit with appropriate status code
fn handle_fatal_error(error: anyhow::Error) -> ! {
    error!("Fatal error: {}", error);
    eprintln!("Error: {error}");
    std::process::exit(1)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    init_tracing(cli.verbose);

    let result = execute_command(cli.command).await;

    if let Err(e) = result {
        handle_fatal_error(e);
    }
}

/// Display a single worktree session with its state and metadata
fn display_worktree_session(
    session: &prodigy::worktree::WorktreeSession,
    worktree_manager: &prodigy::worktree::WorktreeManager,
) -> anyhow::Result<()> {
    let state_file = worktree_manager
        .base_dir
        .join(".metadata")
        .join(format!("{}.json", session.name));

    if let Ok(state_json) = std::fs::read_to_string(&state_file) {
        if let Ok(state) = serde_json::from_str::<prodigy::worktree::WorktreeState>(&state_json) {
            let status_emoji = match state.status {
                prodigy::worktree::WorktreeStatus::InProgress => "🔄",
                prodigy::worktree::WorktreeStatus::Completed => "✅",
                prodigy::worktree::WorktreeStatus::Merged => "🔀",
                prodigy::worktree::WorktreeStatus::CleanedUp => "🧹",
                prodigy::worktree::WorktreeStatus::Failed => "❌",
                prodigy::worktree::WorktreeStatus::Abandoned => "⚠️",
                prodigy::worktree::WorktreeStatus::Interrupted => "⏸️",
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
fn display_worktree_session_legacy(session: &prodigy::worktree::WorktreeSession) {
    println!("  {} - {}", session.name, session.path.display());
}

/// Handle the list command for worktrees
async fn handle_list_command(
    worktree_manager: &prodigy::worktree::WorktreeManager,
) -> anyhow::Result<()> {
    let sessions = worktree_manager.list_sessions().await?;
    if sessions.is_empty() {
        println!("No active Prodigy worktrees found.");
    } else {
        println!("Active Prodigy worktrees:");
        for session in sessions {
            display_worktree_session(&session, worktree_manager)?;
        }
    }
    Ok(())
}

/// Handle the merge command for worktrees
async fn handle_merge_command(
    worktree_manager: &prodigy::worktree::WorktreeManager,
    name: Option<String>,
    all: bool,
) -> anyhow::Result<()> {
    if all {
        // Merge all worktrees
        let sessions = worktree_manager.list_sessions().await?;
        if sessions.is_empty() {
            println!("No active Prodigy worktrees found to merge.");
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

/// Determine the cleanup action type based on command arguments
fn determine_cleanup_action(name: Option<&String>, all: bool, merged_only: bool) -> CleanupAction {
    match () {
        _ if merged_only => CleanupAction::MergedOnly,
        _ if all => CleanupAction::All,
        _ if name.is_some() => CleanupAction::Single,
        _ => CleanupAction::ShowMergeable,
    }
}

/// Represents the type of cleanup action to perform
enum CleanupAction {
    MergedOnly,
    All,
    Single,
    ShowMergeable,
}

/// Handle cleanup of merged sessions only
async fn handle_merged_only_cleanup(
    worktree_manager: &prodigy::worktree::WorktreeManager,
    cleanup_config: &prodigy::worktree::CleanupConfig,
) -> anyhow::Result<()> {
    println!("🔍 Cleaning up merged sessions only...");
    let cleaned_sessions = worktree_manager
        .cleanup_merged_sessions(cleanup_config)
        .await?;
    if cleaned_sessions.is_empty() {
        println!("ℹ️  No merged sessions found for cleanup");
    } else {
        println!(
            "✅ Cleaned up {} merged session(s): {}",
            cleaned_sessions.len(),
            cleaned_sessions.join(", ")
        );
    }
    Ok(())
}

/// Handle cleanup of all sessions
async fn handle_all_cleanup(
    worktree_manager: &prodigy::worktree::WorktreeManager,
    force: bool,
) -> anyhow::Result<()> {
    println!("Cleaning up all Prodigy worktrees...");
    worktree_manager.cleanup_all_sessions(force).await?;
    println!("✅ All worktrees cleaned up");
    Ok(())
}

/// Handle cleanup of a single named session
async fn handle_single_cleanup(
    worktree_manager: &prodigy::worktree::WorktreeManager,
    name: &str,
    force: bool,
) -> anyhow::Result<()> {
    // Check if the session is marked as merged and use appropriate cleanup method
    if let Ok(state) = worktree_manager.get_session_state(name) {
        if state.merged {
            println!("🔍 Session '{name}' is merged, using safe cleanup...");
            worktree_manager.cleanup_session_after_merge(name).await?;
        } else {
            println!("Cleaning up worktree '{name}'...");
            worktree_manager.cleanup_session(name, force).await?;
        }
    } else {
        println!("Cleaning up worktree '{name}'...");
        worktree_manager.cleanup_session(name, force).await?;
    }
    println!("✅ Worktree '{name}' cleaned up");
    Ok(())
}

/// Show mergeable sessions for potential cleanup
async fn handle_show_mergeable(
    worktree_manager: &prodigy::worktree::WorktreeManager,
) -> anyhow::Result<()> {
    println!("🔍 Checking for sessions that can be cleaned up...");
    let mergeable = worktree_manager.detect_mergeable_sessions().await?;
    if mergeable.is_empty() {
        println!("ℹ️  No merged sessions found for cleanup");
        println!("💡 Use --all to clean up all sessions, or specify a session name");
    } else {
        println!(
            "📋 Found {} merged session(s) ready for cleanup:",
            mergeable.len()
        );
        for session in &mergeable {
            println!("  • {session}");
        }
        println!();
        println!("💡 Run with --merged-only to clean up all merged sessions");
    }
    Ok(())
}

/// Handle the clean command for worktrees
async fn handle_clean_command(
    worktree_manager: &prodigy::worktree::WorktreeManager,
    name: Option<String>,
    all: bool,
    force: bool,
    merged_only: bool,
) -> anyhow::Result<()> {
    use prodigy::worktree::CleanupConfig;

    let cleanup_config = CleanupConfig {
        auto_cleanup: false, // Manual cleanup via CLI
        confirm_before_cleanup: std::env::var("PRODIGY_AUTOMATION").is_err(),
        retention_days: 7,
        dry_run: false,
    };

    let action = determine_cleanup_action(name.as_ref(), all, merged_only);

    match action {
        CleanupAction::MergedOnly => {
            handle_merged_only_cleanup(worktree_manager, &cleanup_config).await
        }
        CleanupAction::All => handle_all_cleanup(worktree_manager, force).await,
        CleanupAction::Single => {
            let name = name.unwrap(); // Safe because determine_cleanup_action ensures this
            handle_single_cleanup(worktree_manager, &name, force).await
        }
        CleanupAction::ShowMergeable => handle_show_mergeable(worktree_manager).await,
    }
}

async fn run_worktree_command(command: WorktreeCommands) -> anyhow::Result<()> {
    use prodigy::subprocess::SubprocessManager;
    use prodigy::worktree::WorktreeManager;

    let subprocess = SubprocessManager::production();
    let worktree_manager = WorktreeManager::new(std::env::current_dir()?, subprocess)?;

    match command {
        WorktreeCommands::Ls => handle_list_command(&worktree_manager).await,
        WorktreeCommands::Merge { name, all } => {
            handle_merge_command(&worktree_manager, name, all).await
        }
        WorktreeCommands::Clean {
            all,
            name,
            force,
            merged_only,
        } => handle_clean_command(&worktree_manager, name, all, force, merged_only).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_cleanup_action() {
        // Test merged_only takes precedence
        assert!(matches!(
            determine_cleanup_action(Some(&"test".to_string()), true, true),
            CleanupAction::MergedOnly
        ));

        // Test all flag without merged_only
        assert!(matches!(
            determine_cleanup_action(None, true, false),
            CleanupAction::All
        ));

        // Test single name cleanup
        assert!(matches!(
            determine_cleanup_action(Some(&"test-session".to_string()), false, false),
            CleanupAction::Single
        ));

        // Test default shows mergeable
        assert!(matches!(
            determine_cleanup_action(None, false, false),
            CleanupAction::ShowMergeable
        ));
    }

    #[test]
    fn test_get_log_level() {
        assert_eq!(get_log_level(0), "info");
        assert_eq!(get_log_level(1), "debug");
        assert_eq!(get_log_level(2), "trace");
        assert_eq!(get_log_level(3), "trace,hyper=debug,tower=debug");
        assert_eq!(get_log_level(10), "trace,hyper=debug,tower=debug");
    }

    #[test]
    fn test_cleanup_action_priority() {
        // Verify that merged_only has highest priority
        let name = Some("worktree-123".to_string());
        assert!(matches!(
            determine_cleanup_action(name.as_ref(), true, true),
            CleanupAction::MergedOnly
        ));

        // Verify all has second priority
        assert!(matches!(
            determine_cleanup_action(name.as_ref(), true, false),
            CleanupAction::All
        ));

        // Verify single name has third priority
        assert!(matches!(
            determine_cleanup_action(name.as_ref(), false, false),
            CleanupAction::Single
        ));
    }
}
