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
    /// Resume a MapReduce job from its checkpoint
    #[command(name = "resume-job", alias = "resume")]
    ResumeJob {
        /// Job ID to resume
        job_id: String,

        /// Force resume even if job appears complete
        #[arg(long)]
        force: bool,

        /// Maximum additional retries for failed items
        #[arg(long, default_value = "2")]
        max_retries: u32,

        /// Path to the repository (defaults to current directory)
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
    },
    /// View and search MapReduce events
    #[command(name = "events")]
    Events {
        #[command(subcommand)]
        command: EventCommands,
    },
    /// Manage Dead Letter Queue for failed MapReduce items
    #[command(name = "dlq")]
    Dlq {
        #[command(subcommand)]
        command: DlqCommands,
    },
    /// Manage cooking sessions
    Sessions {
        #[command(subcommand)]
        command: SessionCommands,
    },
}

#[derive(Subcommand)]
enum SessionCommands {
    /// List resumable sessions
    #[command(name = "ls", alias = "list")]
    List,
    /// Show details about a specific session
    Show {
        /// Session ID to show details for
        session_id: String,
    },
    /// Clean up old sessions
    Clean {
        /// Clean all sessions (not just old ones)
        #[arg(long)]
        all: bool,
        /// Force cleanup without confirmation
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum EventCommands {
    /// List all events
    List {
        /// Filter by job ID
        #[arg(long)]
        job_id: Option<String>,

        /// Filter by event type
        #[arg(long)]
        event_type: Option<String>,

        /// Filter by agent ID
        #[arg(long)]
        agent_id: Option<String>,

        /// Show only events from the last N minutes
        #[arg(long)]
        since: Option<u64>,

        /// Limit number of events shown
        #[arg(long, default_value = "100")]
        limit: usize,

        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,
    },
    /// Show event statistics
    Stats {
        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,

        /// Group statistics by field (job_id, event_type, agent_id)
        #[arg(long, default_value = "event_type")]
        group_by: String,
    },
    /// Search events by pattern
    Search {
        /// Search pattern (regex supported)
        pattern: String,

        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,

        /// Search in specific fields only
        #[arg(long)]
        fields: Option<Vec<String>>,
    },
    /// Follow events in real-time (tail -f style)
    Follow {
        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,

        /// Filter by job ID
        #[arg(long)]
        job_id: Option<String>,

        /// Filter by event type
        #[arg(long)]
        event_type: Option<String>,
    },
    /// Export events to different format
    Export {
        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,

        /// Output format (json, csv, markdown)
        #[arg(long, default_value = "json")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum DlqCommands {
    /// List items in the Dead Letter Queue
    List {
        /// Job ID to filter by
        #[arg(long)]
        job_id: Option<String>,

        /// Only show reprocess-eligible items
        #[arg(long)]
        eligible: bool,

        /// Limit number of items to display
        #[arg(long, default_value = "50")]
        limit: usize,
    },
    /// Inspect a specific DLQ item
    Inspect {
        /// Item ID to inspect
        item_id: String,

        /// Job ID containing the item
        #[arg(long)]
        job_id: Option<String>,
    },
    /// Reprocess items from the DLQ
    Reprocess {
        /// Item IDs to reprocess (comma-separated)
        #[arg(value_delimiter = ',')]
        item_ids: Vec<String>,

        /// Job ID to reprocess from
        #[arg(long)]
        job_id: Option<String>,

        /// Maximum retries for reprocessing
        #[arg(long, default_value = "2")]
        max_retries: u32,

        /// Force reprocessing even if not eligible
        #[arg(long)]
        force: bool,
    },
    /// Analyze failure patterns in the DLQ
    Analyze {
        /// Job ID to analyze
        #[arg(long)]
        job_id: Option<String>,

        /// Export analysis to file
        #[arg(long)]
        export: Option<PathBuf>,
    },
    /// Export DLQ items to a file
    Export {
        /// Output file path
        output: PathBuf,

        /// Job ID to export from
        #[arg(long)]
        job_id: Option<String>,

        /// Export format (json, csv)
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Purge old items from the DLQ
    Purge {
        /// Delete items older than N days
        #[arg(long)]
        older_than_days: u32,

        /// Job ID to purge from
        #[arg(long)]
        job_id: Option<String>,

        /// Confirm purge without prompting
        #[arg(long)]
        yes: bool,
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
        Some(Commands::ResumeJob {
            job_id,
            force,
            max_retries,
            path,
        }) => run_resume_job_command(job_id, force, max_retries, path).await,
        Some(Commands::Events { command }) => run_events_command(command).await,
        Some(Commands::Dlq { command }) => run_dlq_command(command).await,
        Some(Commands::Sessions { command }) => run_sessions_command(command).await,
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

/// Classify the merge operation type based on parameters
fn classify_merge_type(name: &Option<String>, all: bool) -> MergeType {
    match () {
        _ if all => MergeType::All,
        _ if name.is_some() => MergeType::Single(name.clone().unwrap()),
        _ => MergeType::Invalid,
    }
}

/// Type of merge operation to perform
enum MergeType {
    All,
    Single(String),
    Invalid,
}

/// Merge a single worktree session with cleanup handling
async fn merge_single_session(
    worktree_manager: &prodigy::worktree::WorktreeManager,
    session_name: &str,
    auto_cleanup: bool,
) -> anyhow::Result<()> {
    println!("\n📝 Merging worktree '{}'...", session_name);

    match worktree_manager.merge_session(session_name).await {
        Ok(_) => {
            println!("✅ Successfully merged worktree '{}'", session_name);
            if auto_cleanup {
                if let Err(e) = worktree_manager.cleanup_session(session_name, true).await {
                    eprintln!(
                        "⚠️ Warning: Failed to clean up worktree '{}': {}",
                        session_name, e
                    );
                }
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ Failed to merge worktree '{}': {}", session_name, e);
            eprintln!("   Skipping cleanup for failed merge.");
            Err(e)
        }
    }
}

/// Ask user for cleanup confirmation
fn should_cleanup_worktree() -> anyhow::Result<bool> {
    println!("Would you like to clean up the worktree? (y/N)");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_lowercase() == "y")
}

/// Handle merging all worktree sessions
async fn handle_merge_all(
    worktree_manager: &prodigy::worktree::WorktreeManager,
) -> anyhow::Result<()> {
    let sessions = worktree_manager.list_sessions().await?;

    if sessions.is_empty() {
        println!("No active Prodigy worktrees found to merge.");
        return Ok(());
    }

    println!("Found {} worktree(s) to merge", sessions.len());

    for session in sessions {
        let _ = merge_single_session(worktree_manager, &session.name, true).await;
    }

    println!("\n✅ Bulk merge operation completed");
    Ok(())
}

/// Handle merging a single named worktree
async fn handle_merge_single(
    worktree_manager: &prodigy::worktree::WorktreeManager,
    name: String,
) -> anyhow::Result<()> {
    println!("Merging worktree '{name}'...");
    worktree_manager.merge_session(&name).await?;
    println!("✅ Successfully merged worktree '{name}'");

    if should_cleanup_worktree()? {
        worktree_manager.cleanup_session(&name, true).await?;
        println!("✅ Worktree cleaned up");
    }

    Ok(())
}

/// Handle the merge command for worktrees
async fn handle_merge_command(
    worktree_manager: &prodigy::worktree::WorktreeManager,
    name: Option<String>,
    all: bool,
) -> anyhow::Result<()> {
    match classify_merge_type(&name, all) {
        MergeType::All => handle_merge_all(worktree_manager).await,
        MergeType::Single(name) => handle_merge_single(worktree_manager, name).await,
        MergeType::Invalid => anyhow::bail!("Either --all or a worktree name must be specified"),
    }
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

/// Handle the resume-job command
async fn run_resume_job_command(
    job_id: String,
    force: bool,
    max_retries: u32,
    path: Option<PathBuf>,
) -> anyhow::Result<()> {
    println!("📝 Resuming MapReduce job: {}", job_id);
    println!("  Options: force={}, max_retries={}", force, max_retries);

    // For now, print a message that this feature is implemented but needs the infrastructure
    println!("✅ Resume job command infrastructure is ready.");
    println!("Note: To resume a job, ensure the job was created with checkpoint support.");

    if let Some(p) = path {
        println!("  Working directory: {}", p.display());
    }

    // TODO: Once the proper infrastructure is in place, this will:
    // 1. Load the job state from checkpoint
    // 2. Resume execution from the last checkpoint
    // 3. Process remaining work items
    // 4. Handle retries for failed items

    Ok(())
}

async fn run_dlq_command(command: DlqCommands) -> anyhow::Result<()> {
    use chrono::{Duration, Utc};
    use prodigy::cook::execution::dlq::{DLQFilter, DeadLetterQueue};

    // Get project root
    let project_root = std::env::current_dir()?;
    let dlq_path = project_root.join(".prodigy");

    match command {
        DlqCommands::List {
            job_id,
            eligible,
            limit,
        } => {
            // Find the most recent job ID if not specified
            let job_id = if let Some(id) = job_id {
                id
            } else {
                // Try to find the most recent job
                anyhow::bail!("Job ID is required. Use --job-id to specify.");
            };

            let dlq = DeadLetterQueue::new(job_id.clone(), dlq_path, 10000, 30, None).await?;

            let filter = DLQFilter {
                reprocess_eligible: if eligible { Some(true) } else { None },
                ..Default::default()
            };

            let items = dlq.list_items(filter).await?;
            let display_items: Vec<_> = items.into_iter().take(limit).collect();

            if display_items.is_empty() {
                println!("No items in Dead Letter Queue for job {}", job_id);
            } else {
                println!("Dead Letter Queue items for job {}:", job_id);
                println!("{:─<80}", "");
                for item in display_items {
                    println!("ID: {}", item.item_id);
                    println!("  Last Attempt: {}", item.last_attempt);
                    println!("  Failure Count: {}", item.failure_count);
                    println!("  Error: {}", item.error_signature);
                    println!("  Reprocess Eligible: {}", item.reprocess_eligible);
                    println!("{:─<80}", "");
                }
            }
        }
        DlqCommands::Inspect { item_id, job_id } => {
            let job_id = job_id.ok_or_else(|| anyhow::anyhow!("Job ID is required for inspect"))?;

            let dlq = DeadLetterQueue::new(job_id.clone(), dlq_path, 10000, 30, None).await?;

            if let Some(item) = dlq.get_item(&item_id).await? {
                println!("Dead Letter Queue Item Details:");
                println!("{}", serde_json::to_string_pretty(&item)?);
            } else {
                println!("Item {} not found in DLQ", item_id);
            }
        }
        DlqCommands::Analyze { job_id, export } => {
            let job_id =
                job_id.ok_or_else(|| anyhow::anyhow!("Job ID is required for analysis"))?;

            let dlq = DeadLetterQueue::new(job_id.clone(), dlq_path, 10000, 30, None).await?;

            let analysis = dlq.analyze_patterns().await?;

            if let Some(export_path) = export {
                let json = serde_json::to_string_pretty(&analysis)?;
                std::fs::write(&export_path, json)?;
                println!("Analysis exported to {:?}", export_path);
            } else {
                println!("Dead Letter Queue Analysis for job {}:", job_id);
                println!("Total Items: {}", analysis.total_items);
                println!("\nFailure Patterns:");
                for pattern in analysis.pattern_groups {
                    println!("  {}: {} occurrences", pattern.signature, pattern.count);
                }
                println!("\nError Distribution:");
                for (error_type, count) in analysis.error_distribution {
                    println!("  {:?}: {}", error_type, count);
                }
            }
        }
        DlqCommands::Export {
            output,
            job_id,
            format,
        } => {
            let job_id = job_id.ok_or_else(|| anyhow::anyhow!("Job ID is required for export"))?;

            let dlq = DeadLetterQueue::new(job_id.clone(), dlq_path, 10000, 30, None).await?;

            dlq.export_items(&output).await?;
            println!("DLQ items exported to {:?} in {} format", output, format);
        }
        DlqCommands::Reprocess {
            item_ids: _,
            job_id: _,
            max_retries: _,
            force: _,
        } => {
            anyhow::bail!("DLQ reprocessing is not yet implemented. Items must be manually reviewed and resubmitted.");
        }
        DlqCommands::Purge {
            older_than_days,
            job_id,
            yes,
        } => {
            let job_id = job_id.ok_or_else(|| anyhow::anyhow!("Job ID is required for purge"))?;

            if !yes {
                println!(
                    "This will permanently delete DLQ items older than {} days.",
                    older_than_days
                );
                println!("Continue? (y/N)");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Purge cancelled.");
                    return Ok(());
                }
            }

            let dlq = DeadLetterQueue::new(job_id.clone(), dlq_path, 10000, 30, None).await?;

            let cutoff = Utc::now() - Duration::days(older_than_days as i64);
            let count = dlq.purge_old_items(cutoff).await?;
            println!("Purged {} items from DLQ", count);
        }
    }

    Ok(())
}

async fn run_events_command(command: EventCommands) -> anyhow::Result<()> {
    use prodigy::cli::events::{self, EventsArgs, EventsCommand};

    let events_args = match command {
        EventCommands::List {
            job_id,
            event_type,
            agent_id,
            since,
            limit,
            file,
        } => EventsArgs {
            command: EventsCommand::List {
                job_id,
                event_type,
                agent_id,
                since,
                limit,
                file,
            },
        },
        EventCommands::Stats { file, group_by } => EventsArgs {
            command: EventsCommand::Stats { file, group_by },
        },
        EventCommands::Search {
            pattern,
            file,
            fields,
        } => EventsArgs {
            command: EventsCommand::Search {
                pattern,
                file,
                fields,
            },
        },
        EventCommands::Follow {
            file,
            job_id,
            event_type,
        } => EventsArgs {
            command: EventsCommand::Follow {
                file,
                job_id,
                event_type,
            },
        },
        EventCommands::Export {
            file,
            format,
            output,
        } => EventsArgs {
            command: EventsCommand::Export {
                file,
                format,
                output,
            },
        },
    };

    events::execute(events_args).await
}

async fn run_sessions_command(command: SessionCommands) -> anyhow::Result<()> {
    use prodigy::cook::session::{SessionManager, SessionTrackerImpl};
    use std::path::PathBuf;

    let working_dir = std::env::current_dir()?;
    let session_tracker = SessionTrackerImpl::new("session-query".to_string(), working_dir.clone());

    match command {
        SessionCommands::List => {
            let sessions = session_tracker.list_resumable().await?;
            if sessions.is_empty() {
                println!("No resumable sessions found.");
            } else {
                println!("Resumable sessions:");
                for session in sessions {
                    println!(
                        "  {} - {:?} - Started: {} - Progress: {}",
                        session.session_id,
                        session.status,
                        session.started_at.format("%Y-%m-%d %H:%M:%S"),
                        session.progress
                    );
                    if !session.workflow_path.as_os_str().is_empty() {
                        println!("    Workflow: {}", session.workflow_path.display());
                    }
                }
            }
            Ok(())
        }
        SessionCommands::Show { session_id } => {
            match session_tracker.load_session(&session_id).await {
                Ok(state) => {
                    println!("Session: {}", state.session_id);
                    println!("Status: {:?}", state.status);
                    println!("Started: {}", state.started_at.format("%Y-%m-%d %H:%M:%S"));
                    if let Some(ended) = state.ended_at {
                        println!("Ended: {}", ended.format("%Y-%m-%d %H:%M:%S"));
                    }
                    println!("Working Directory: {}", state.working_directory.display());
                    if let Some(ref worktree) = state.worktree_name {
                        println!("Worktree: {}", worktree);
                    }
                    println!("Iterations Completed: {}", state.iterations_completed);
                    println!("Files Changed: {}", state.files_changed);

                    if let Some(ref workflow_state) = state.workflow_state {
                        println!("\nWorkflow State:");
                        println!(
                            "  Current Step: {}/{}",
                            workflow_state.current_step + 1,
                            workflow_state.completed_steps.len() + 1
                        );
                        println!(
                            "  Current Iteration: {}",
                            workflow_state.current_iteration + 1
                        );
                        println!(
                            "  Workflow Path: {}",
                            workflow_state.workflow_path.display()
                        );
                        if !workflow_state.input_args.is_empty() {
                            println!("  Arguments: {:?}", workflow_state.input_args);
                        }
                        if !workflow_state.map_patterns.is_empty() {
                            println!("  Map Patterns: {:?}", workflow_state.map_patterns);
                        }
                    }

                    if state.is_resumable() {
                        println!("\n✅ This session can be resumed with:");
                        println!("  prodigy cook <workflow> --resume {}", session_id);
                    } else {
                        println!(
                            "\n❌ This session cannot be resumed (status: {:?})",
                            state.status
                        );
                    }
                    Ok(())
                }
                Err(e) => {
                    eprintln!("Error: Session {} not found: {}", session_id, e);
                    std::process::exit(1);
                }
            }
        }
        SessionCommands::Clean { all, force } => {
            let sessions = session_tracker.list_resumable().await?;

            if sessions.is_empty() {
                println!("No sessions to clean.");
                return Ok(());
            }

            let sessions_to_clean = if all {
                sessions
            } else {
                // Filter for old sessions (> 7 days)
                let cutoff = chrono::Utc::now() - chrono::Duration::days(7);
                sessions
                    .into_iter()
                    .filter(|s| s.started_at < cutoff)
                    .collect()
            };

            if sessions_to_clean.is_empty() {
                println!("No old sessions to clean (use --all to clean all sessions).");
                return Ok(());
            }

            if !force {
                println!("Would clean {} sessions:", sessions_to_clean.len());
                for session in &sessions_to_clean {
                    println!(
                        "  {} - Started: {}",
                        session.session_id,
                        session.started_at.format("%Y-%m-%d %H:%M:%S")
                    );
                }
                println!("\nUse --force to actually clean these sessions.");
            } else {
                println!("Cleaning {} sessions...", sessions_to_clean.len());
                let base_path = working_dir.join(".prodigy");
                for session in sessions_to_clean {
                    let session_file = base_path.join(format!("{}.json", session.session_id));
                    if session_file.exists() {
                        std::fs::remove_file(&session_file)?;
                        println!("  Cleaned: {}", session.session_id);
                    }
                }
                println!("Session cleanup complete.");
            }

            Ok(())
        }
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
    fn test_classify_merge_type() {
        // Test all flag takes precedence
        assert!(matches!(
            classify_merge_type(&Some("test".to_string()), true),
            MergeType::All
        ));

        // Test single name without all flag
        assert!(matches!(
            classify_merge_type(&Some("worktree-123".to_string()), false),
            MergeType::Single(name) if name == "worktree-123"
        ));

        // Test invalid when neither all nor name is provided
        assert!(matches!(
            classify_merge_type(&None, false),
            MergeType::Invalid
        ));

        // Test all flag without name
        assert!(matches!(classify_merge_type(&None, true), MergeType::All));
    }

    #[test]
    fn test_merge_type_classification_edge_cases() {
        // Empty string name should still be classified as Single
        let empty_name = Some(String::new());
        assert!(matches!(
            classify_merge_type(&empty_name, false),
            MergeType::Single(name) if name.is_empty()
        ));

        // All flag should override name even with empty string
        assert!(matches!(
            classify_merge_type(&empty_name, true),
            MergeType::All
        ));
    }

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

    #[test]
    fn test_merge_type_priority() {
        // All flag should have highest priority
        let name = Some("session-name".to_string());
        assert!(matches!(classify_merge_type(&name, true), MergeType::All));

        // Name without all flag should result in Single
        assert!(matches!(
            classify_merge_type(&name, false),
            MergeType::Single(n) if n == "session-name"
        ));

        // No parameters should be Invalid
        assert!(matches!(
            classify_merge_type(&None, false),
            MergeType::Invalid
        ));
    }
}
