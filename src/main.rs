use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;
use tracing::{debug, error, trace};

/// Execute automated workflows with zero configuration
#[derive(Parser)]
#[command(name = "prodigy")]
#[command(about = "prodigy - Execute automated workflows with zero configuration", long_about = None)]
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
    /// Run a workflow file
    #[command(name = "run")]
    Run {
        /// Workflow file to execute
        workflow: PathBuf,

        /// Repository path to run in (defaults to current directory)
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,

        /// Maximum number of iterations
        #[arg(short = 'n', long, default_value = "1")]
        max_iterations: u32,

        /// Run in an isolated git worktree
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

        /// Dry-run mode - show what would be executed without running
        #[arg(long, help = "Preview commands without executing them")]
        dry_run: bool,
    },

    /// Execute a single command with retry support
    #[command(name = "exec")]
    Exec {
        /// Command to execute (e.g., "claude: /refactor app.py" or "shell: npm test")
        command: String,

        /// Number of retry attempts
        #[arg(long, default_value = "1")]
        retry: u32,

        /// Timeout in seconds
        #[arg(long)]
        timeout: Option<u64>,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
    },

    /// Process multiple files in parallel
    #[command(name = "batch")]
    Batch {
        /// File pattern to match (e.g., "*.py", "src/**/*.ts")
        pattern: String,

        /// Command to execute for each file
        #[arg(long)]
        command: String,

        /// Number of parallel workers
        #[arg(long, default_value = "5")]
        parallel: usize,

        /// Number of retry attempts per file
        #[arg(long)]
        retry: Option<u32>,

        /// Timeout per file in seconds
        #[arg(long)]
        timeout: Option<u64>,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
    },

    /// Resume an interrupted workflow
    #[command(name = "resume")]
    Resume {
        /// Workflow ID to resume (optional - will auto-detect last interrupted)
        workflow_id: Option<String>,

        /// Force resume even if marked complete
        #[arg(long)]
        force: bool,

        /// Resume from specific checkpoint
        #[arg(long = "from-checkpoint")]
        from_checkpoint: Option<String>,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
    },

    /// List available workflow checkpoints
    #[command(name = "checkpoints")]
    Checkpoints {
        #[command(subcommand)]
        command: CheckpointCommands,
    },

    /// Execute goal-seeking operation with iterative refinement
    #[command(name = "goal-seek", alias = "seek")]
    GoalSeek {
        /// Goal description
        #[arg(help = "What you want to achieve")]
        goal: String,

        /// Command to execute for attempts
        #[arg(
            short = 'c',
            long,
            help = "Command to execute (gets validation context)"
        )]
        command: String,

        /// Validation command
        #[arg(
            long,
            help = "Command to validate results (should output score: 0-100)"
        )]
        validate: String,

        /// Success threshold (0-100)
        #[arg(
            short = 't',
            long,
            default_value = "80",
            help = "Minimum score to consider success"
        )]
        threshold: u32,

        /// Maximum attempts
        #[arg(
            short = 'm',
            long,
            default_value = "5",
            help = "Maximum attempts before giving up"
        )]
        max_attempts: u32,

        /// Timeout in seconds
        #[arg(long, help = "Overall timeout in seconds")]
        timeout: Option<u64>,

        /// Fail on incomplete
        #[arg(long, help = "Exit with error if goal not achieved")]
        fail_on_incomplete: bool,

        /// Working directory
        #[arg(short = 'p', long, help = "Working directory for commands")]
        path: Option<PathBuf>,
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
    /// Migrate workflow YAML files to simplified syntax
    #[command(name = "migrate-yaml")]
    MigrateYaml {
        /// Workflow file or directory to migrate (defaults to workflows/)
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,

        /// Create backup files (.bak)
        #[arg(long, default_value = "true")]
        backup: bool,

        /// Dry run - show what would be changed without modifying files
        #[arg(long)]
        dry_run: bool,

        /// Force overwrite without backup
        #[arg(short, long)]
        force: bool,
    },
    /// Validate workflow YAML format and suggest improvements
    #[command(name = "validate")]
    Validate {
        /// Workflow file to validate
        workflow: PathBuf,

        /// Check for simplified format
        #[arg(long, default_value = "simplified")]
        format: String,

        /// Show suggestions for improvements
        #[arg(long, default_value = "true")]
        suggest: bool,

        /// Exit with error code if not valid
        #[arg(long)]
        strict: bool,
    },
    /// Resume a MapReduce job from its checkpoint
    #[command(name = "resume-job")]
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
    /// Manage workflow sessions
    Sessions {
        #[command(subcommand)]
        command: SessionCommands,
    },
    /// View MapReduce job progress
    #[command(name = "progress")]
    Progress {
        /// Job ID to view progress for
        job_id: String,

        /// Export progress data to file
        #[arg(long)]
        export: Option<PathBuf>,

        /// Export format (json, csv, html)
        #[arg(long, default_value = "json")]
        format: String,

        /// Start web dashboard on specified port
        #[arg(long)]
        web: Option<u16>,
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
enum CheckpointCommands {
    /// List all available checkpoints
    #[command(name = "list", alias = "ls")]
    List {
        /// Filter by workflow ID
        #[arg(long)]
        workflow_id: Option<String>,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,

        /// Show verbose details
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Delete checkpoints for completed workflows
    #[command(name = "clean")]
    Clean {
        /// Clean checkpoints for specific workflow
        #[arg(long)]
        workflow_id: Option<String>,

        /// Clean all completed workflow checkpoints
        #[arg(long)]
        all: bool,

        /// Force deletion without confirmation
        #[arg(short = 'f', long)]
        force: bool,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
    },

    /// Show detailed checkpoint information
    #[command(name = "show")]
    Show {
        /// Workflow ID
        workflow_id: String,

        /// Checkpoint version (defaults to latest)
        #[arg(long)]
        version: Option<u32>,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum EventCommands {
    /// List all events
    #[command(alias = "list")]
    Ls {
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
    /// Clean old events based on retention policies
    Clean {
        /// Delete events older than specified duration (e.g., "7d", "30d")
        #[arg(long)]
        older_than: Option<String>,

        /// Keep only the most recent N events
        #[arg(long)]
        max_events: Option<usize>,

        /// Keep only events up to specified size (e.g., "10MB", "1GB")
        #[arg(long)]
        max_size: Option<String>,

        /// Preview what would be deleted without actually deleting
        #[arg(long)]
        dry_run: bool,

        /// Archive events instead of deleting them
        #[arg(long)]
        archive: bool,

        /// Path to archive directory
        #[arg(long)]
        archive_path: Option<PathBuf>,

        /// Apply to all jobs instead of current job
        #[arg(long)]
        all_jobs: bool,

        /// Specific job ID to clean
        #[arg(long)]
        job_id: Option<String>,

        /// Specific event file to clean (for testing)
        #[arg(long)]
        file: Option<PathBuf>,
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
    /// Retry failed items from the DLQ
    Retry {
        /// Workflow ID to retry
        workflow_id: String,

        /// Filter expression for selective retry
        #[arg(long)]
        filter: Option<String>,

        /// Maximum retry attempts
        #[arg(long, default_value = "3")]
        max_retries: u32,

        /// Number of parallel workers
        #[arg(long, default_value = "10")]
        parallel: usize,

        /// Force retry even if not eligible
        #[arg(long)]
        force: bool,
    },
    /// Show DLQ statistics
    Stats {
        /// Show stats for specific workflow
        #[arg(long)]
        workflow_id: Option<String>,
    },
    /// Clear processed items from DLQ
    Clear {
        /// Workflow ID to clear
        workflow_id: String,

        /// Confirm clear without prompting
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum WorktreeCommands {
    /// List active Prodigy worktrees
    #[command(alias = "list")]
    Ls {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
        /// Show detailed information for each session
        #[arg(short = 'd', long)]
        detailed: bool,
    },
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

/// Find the most recent checkpoint in the checkpoint directory
async fn find_latest_checkpoint(checkpoint_dir: &PathBuf) -> Option<String> {
    use tokio::fs;

    if !checkpoint_dir.exists() {
        return None;
    }

    let mut entries = match fs::read_dir(checkpoint_dir).await {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    let mut latest_checkpoint = None;
    let mut latest_time = None;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            if let Ok(metadata) = entry.metadata().await {
                if let Ok(modified) = metadata.modified() {
                    if latest_time.is_none_or(|time| modified > time) {
                        latest_time = Some(modified);
                        if let Some(name) = path.file_stem() {
                            latest_checkpoint = Some(name.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }

    latest_checkpoint
}

/// Run checkpoints command
async fn run_checkpoints_command(command: CheckpointCommands) -> anyhow::Result<()> {
    use anyhow::Context;
    use prodigy::cook::workflow::CheckpointManager;

    match command {
        CheckpointCommands::List {
            workflow_id,
            path,
            verbose,
        } => {
            let working_dir = match path {
                Some(p) => p,
                None => std::env::current_dir().context("Failed to get current directory")?,
            };
            let checkpoint_dir = working_dir.join(".prodigy").join("checkpoints");

            if !checkpoint_dir.exists() {
                println!("No checkpoints found.");
                return Ok(());
            }

            let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());

            if let Some(id) = workflow_id {
                // List checkpoints for specific workflow
                match checkpoint_manager.load_checkpoint(&id).await {
                    Ok(checkpoint) => {
                        println!("📋 Checkpoint for workflow: {}", id);
                        println!("   Status: {:?}", checkpoint.execution_state.status);
                        println!(
                            "   Step: {}/{}",
                            checkpoint.execution_state.current_step_index,
                            checkpoint.execution_state.total_steps
                        );
                        println!("   Created: {}", checkpoint.timestamp);

                        if verbose {
                            println!("\n   Completed Steps:");
                            for step in &checkpoint.completed_steps {
                                println!(
                                    "     {} - {} ({})",
                                    step.step_index,
                                    step.command,
                                    if step.success { "✓" } else { "✗" }
                                );
                                if let Some(ref retry) = step.retry_state {
                                    println!(
                                        "       Retry: {}/{}",
                                        retry.current_attempt, retry.max_attempts
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("Error loading checkpoint for {}: {}", id, e);
                    }
                }
            } else {
                // List all checkpoints
                println!("📋 Available checkpoints:");

                let mut entries = tokio::fs::read_dir(&checkpoint_dir).await?;
                let mut checkpoints = Vec::new();

                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                        if let Some(name) = path.file_stem() {
                            let workflow_id = name.to_string_lossy().to_string();
                            if let Ok(checkpoint) =
                                checkpoint_manager.load_checkpoint(&workflow_id).await
                            {
                                checkpoints.push((workflow_id, checkpoint));
                            }
                        }
                    }
                }

                if checkpoints.is_empty() {
                    println!("  No checkpoints found.");
                } else {
                    for (id, checkpoint) in checkpoints {
                        println!(
                            "\n  {} - Status: {:?}",
                            id, checkpoint.execution_state.status
                        );
                        println!(
                            "    Step: {}/{}",
                            checkpoint.execution_state.current_step_index,
                            checkpoint.execution_state.total_steps
                        );
                        println!("    Created: {}", checkpoint.timestamp);
                    }
                }
            }
            Ok(())
        }
        CheckpointCommands::Clean {
            workflow_id,
            all,
            force,
            path,
        } => {
            let working_dir = match path {
                Some(p) => p,
                None => std::env::current_dir().context("Failed to get current directory")?,
            };
            let checkpoint_dir = working_dir.join(".prodigy").join("checkpoints");

            if !checkpoint_dir.exists() {
                println!("No checkpoints to clean.");
                return Ok(());
            }

            if let Some(id) = workflow_id {
                // Clean specific workflow checkpoint
                let checkpoint_path = checkpoint_dir.join(format!("{}.json", id));
                if checkpoint_path.exists() {
                    if !force {
                        print!("Delete checkpoint for {}? [y/N] ", id);
                        use std::io::{self, Write};
                        io::stdout().flush()?;
                        let mut input = String::new();
                        io::stdin().read_line(&mut input)?;
                        if !input.trim().eq_ignore_ascii_case("y") {
                            println!("Cancelled.");
                            return Ok(());
                        }
                    }
                    tokio::fs::remove_file(&checkpoint_path).await?;
                    println!("✅ Deleted checkpoint for {}", id);
                } else {
                    println!("No checkpoint found for {}", id);
                }
            } else if all {
                // Clean all completed workflow checkpoints
                let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());
                let mut entries = tokio::fs::read_dir(&checkpoint_dir).await?;
                let mut deleted = 0;

                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                        if let Some(name) = path.file_stem() {
                            let workflow_id = name.to_string_lossy().to_string();
                            if let Ok(checkpoint) =
                                checkpoint_manager.load_checkpoint(&workflow_id).await
                            {
                                use prodigy::cook::workflow::checkpoint::WorkflowStatus;
                                if checkpoint.execution_state.status == WorkflowStatus::Completed {
                                    if !force {
                                        println!(
                                            "Delete completed checkpoint for {}?",
                                            workflow_id
                                        );
                                    }
                                    tokio::fs::remove_file(&path).await?;
                                    deleted += 1;
                                }
                            }
                        }
                    }
                }

                println!("✅ Deleted {} completed checkpoints", deleted);
            } else {
                println!("Please specify --workflow-id or --all");
            }
            Ok(())
        }
        CheckpointCommands::Show {
            workflow_id,
            version: _,
            path,
        } => {
            let working_dir = match path {
                Some(p) => p,
                None => std::env::current_dir().context("Failed to get current directory")?,
            };
            let checkpoint_dir = working_dir.join(".prodigy").join("checkpoints");
            let checkpoint_manager = CheckpointManager::new(checkpoint_dir);

            match checkpoint_manager.load_checkpoint(&workflow_id).await {
                Ok(checkpoint) => {
                    println!("📋 Checkpoint Details for: {}", workflow_id);
                    println!("\nExecution State:");
                    println!("  Status: {:?}", checkpoint.execution_state.status);
                    println!(
                        "  Current Step: {}/{}",
                        checkpoint.execution_state.current_step_index,
                        checkpoint.execution_state.total_steps
                    );
                    println!("  Start Time: {}", checkpoint.execution_state.start_time);
                    println!(
                        "  Last Checkpoint: {}",
                        checkpoint.execution_state.last_checkpoint
                    );

                    println!("\nWorkflow Info:");
                    if let Some(ref name) = checkpoint.workflow_name {
                        println!("  Name: {}", name);
                    }
                    if let Some(ref path) = checkpoint.workflow_path {
                        println!("  Path: {}", path.display());
                    }
                    println!("  Version: {}", checkpoint.version);
                    println!("  Hash: {}", checkpoint.workflow_hash);

                    println!("\nCompleted Steps: {}", checkpoint.completed_steps.len());
                    for step in &checkpoint.completed_steps {
                        println!(
                            "  [{}] {} - {} (Duration: {:?})",
                            step.step_index,
                            step.command,
                            if step.success {
                                "✓ Success"
                            } else {
                                "✗ Failed"
                            },
                            step.duration
                        );

                        if let Some(ref retry) = step.retry_state {
                            println!(
                                "      Retry: {}/{} attempts",
                                retry.current_attempt, retry.max_attempts
                            );
                            if !retry.failure_history.is_empty() {
                                println!("      Failures: {:?}", retry.failure_history);
                            }
                        }

                        if !step.captured_variables.is_empty() {
                            println!(
                                "      Variables: {:?}",
                                step.captured_variables.keys().collect::<Vec<_>>()
                            );
                        }
                    }

                    if !checkpoint.variable_state.is_empty() {
                        println!("\nVariable State:");
                        for key in checkpoint.variable_state.keys() {
                            println!("  {}", key);
                        }
                    }

                    if let Some(ref mapreduce) = checkpoint.mapreduce_state {
                        println!("\nMapReduce State:");
                        println!("  Completed Items: {}", mapreduce.completed_items.len());
                        println!("  Failed Items: {}", mapreduce.failed_items.len());
                        println!("  In Progress: {}", mapreduce.in_progress_items.len());
                        println!("  Reduce Completed: {}", mapreduce.reduce_completed);
                    }
                }
                Err(e) => {
                    println!("Error loading checkpoint for {}: {}", workflow_id, e);
                }
            }
            Ok(())
        }
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

/// Parameters for goal-seeking operation
struct GoalSeekParams {
    goal: String,
    command: String,
    validate: String,
    threshold: u32,
    max_attempts: u32,
    timeout: Option<u64>,
    fail_on_incomplete: bool,
    path: Option<PathBuf>,
}

/// Run goal-seeking operation from CLI
async fn run_goal_seek(params: GoalSeekParams) -> anyhow::Result<()> {
    use prodigy::cook::goal_seek::{
        shell_executor::ShellCommandExecutor, GoalSeekConfig, GoalSeekEngine,
    };
    use std::env;

    // Change to specified directory if provided
    if let Some(path) = params.path {
        env::set_current_dir(path)?;
    }

    // Create goal-seek configuration
    // CLI uses shell command by default (could be extended to support --claude flag)
    let config = GoalSeekConfig {
        goal: params.goal.clone(),
        claude: None,
        shell: Some(params.command),
        validate: params.validate,
        threshold: params.threshold,
        max_attempts: params.max_attempts,
        timeout_seconds: params.timeout,
        fail_on_incomplete: Some(params.fail_on_incomplete),
    };

    // Create shell executor and engine
    let executor = Box::new(ShellCommandExecutor::new());
    let mut engine = GoalSeekEngine::new(executor);

    // Execute goal-seeking
    println!("🎯 Starting goal-seeking: {}", params.goal);
    let result = engine.seek(config).await?;

    // Handle result
    use prodigy::cook::goal_seek::GoalSeekResult;
    match result {
        GoalSeekResult::Success {
            attempts,
            final_score,
            execution_time,
        } => {
            println!("✅ Goal achieved in {} attempts!", attempts);
            println!("   Final score: {}%", final_score);
            println!("   Time taken: {:?}", execution_time);
            Ok(())
        }
        GoalSeekResult::MaxAttemptsReached {
            attempts,
            best_score,
            ..
        } => {
            let msg = format!(
                "❌ Goal not achieved after {} attempts. Best score: {}%",
                attempts, best_score
            );
            if params.fail_on_incomplete {
                Err(anyhow::anyhow!(msg))
            } else {
                println!("{}", msg);
                Ok(())
            }
        }
        GoalSeekResult::Timeout {
            attempts,
            best_score,
            elapsed,
        } => Err(anyhow::anyhow!(
            "⏱️  Timed out after {} attempts and {:?}. Best score: {}%",
            attempts,
            elapsed,
            best_score
        )),
        GoalSeekResult::Converged {
            attempts,
            final_score,
            reason,
        } => {
            let msg = format!(
                "🔄 Converged after {} attempts. Score: {}%. Reason: {}",
                attempts, final_score, reason
            );
            if params.fail_on_incomplete && final_score < params.threshold {
                Err(anyhow::anyhow!(msg))
            } else {
                println!("{}", msg);
                Ok(())
            }
        }
        GoalSeekResult::Failed { attempts, error } => Err(anyhow::anyhow!(
            "💥 Failed after {} attempts: {}",
            attempts,
            error
        )),
    }
}

/// Execute the appropriate command based on CLI input
async fn execute_command(command: Option<Commands>, verbose: u8) -> anyhow::Result<()> {
    match command {
        Some(Commands::Run {
            workflow,
            path,
            max_iterations,
            worktree,
            map,
            args,
            fail_fast,
            auto_accept,
            metrics,
            resume,
            dry_run,
        }) => {
            // Run is the primary command for workflow execution
            let cook_cmd = prodigy::cook::command::CookCommand {
                playbook: workflow,
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
                verbosity: verbose,
                dry_run,
            };
            prodigy::cook::cook(cook_cmd).await
        }
        Some(Commands::Exec {
            command,
            retry,
            timeout,
            path,
        }) => run_exec_command(command, retry, timeout, path).await,
        Some(Commands::Batch {
            pattern,
            command,
            parallel,
            retry,
            timeout,
            path,
        }) => run_batch_command(pattern, command, parallel, retry, timeout, path).await,
        Some(Commands::Resume {
            workflow_id,
            force,
            from_checkpoint,
            path,
        }) => run_resume_workflow(workflow_id, force, from_checkpoint, path).await,
        Some(Commands::Checkpoints { command }) => run_checkpoints_command(command).await,
        Some(Commands::GoalSeek {
            goal,
            command,
            validate,
            threshold,
            max_attempts,
            timeout,
            fail_on_incomplete,
            path,
        }) => {
            run_goal_seek(GoalSeekParams {
                goal,
                command,
                validate,
                threshold,
                max_attempts,
                timeout,
                fail_on_incomplete,
                path,
            })
            .await
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
        Some(Commands::MigrateYaml {
            path,
            backup,
            dry_run,
            force,
        }) => run_migrate_yaml_command(path, backup, dry_run, force).await,
        Some(Commands::Validate {
            workflow,
            format,
            suggest,
            strict,
        }) => run_validate_command(workflow, format, suggest, strict).await,
        Some(Commands::ResumeJob {
            job_id,
            force,
            max_retries,
            path,
        }) => run_resume_job_command(job_id, force, max_retries, path).await,
        Some(Commands::Events { command }) => run_events_command(command).await,
        Some(Commands::Dlq { command }) => run_dlq_command(command).await,
        Some(Commands::Sessions { command }) => run_sessions_command(command).await,
        Some(Commands::Progress {
            job_id,
            export,
            format,
            web,
        }) => run_progress_command(job_id, export, format, web).await,
        None => {
            // Display help when no command is provided (following CLI conventions)
            let mut cmd = Cli::command();
            let _ = cmd.print_help();
            println!(); // Add blank line for better formatting
            Ok(())
        }
    }
}

/// Run exec command - execute a single command with retry support
async fn run_exec_command(
    command: String,
    retry: u32,
    timeout: Option<u64>,
    path: Option<PathBuf>,
) -> anyhow::Result<()> {
    use prodigy::cli::workflow_generator::{generate_exec_workflow, TemporaryWorkflow};

    // Change to specified directory if provided
    if let Some(p) = path.clone() {
        std::env::set_current_dir(&p)?;
    }

    println!("🚀 Executing command: {}", command);
    if retry > 1 {
        println!("   Retry attempts: {}", retry);
    }
    if let Some(t) = timeout {
        println!("   Timeout: {}s", t);
    }

    // Generate temporary workflow
    let (_workflow, temp_path) = generate_exec_workflow(&command, retry, timeout)?;
    let _temp_workflow = TemporaryWorkflow {
        path: temp_path.clone(),
    };

    // Execute using cook command
    let cook_cmd = prodigy::cook::command::CookCommand {
        playbook: temp_path,
        path,
        max_iterations: 1,
        worktree: false,
        map: vec![],
        args: vec![],
        fail_fast: false,
        auto_accept: true,
        metrics: false,
        resume: None,
        quiet: false,
        verbosity: 0,
        dry_run: false,
    };

    prodigy::cook::cook(cook_cmd).await
}

/// Run batch command - process multiple files in parallel
async fn run_batch_command(
    pattern: String,
    command: String,
    parallel: usize,
    retry: Option<u32>,
    timeout: Option<u64>,
    path: Option<PathBuf>,
) -> anyhow::Result<()> {
    use prodigy::cli::workflow_generator::{generate_batch_workflow, TemporaryWorkflow};

    // Change to specified directory if provided
    if let Some(p) = path.clone() {
        std::env::set_current_dir(&p)?;
    }

    println!("📦 Starting batch processing");
    println!("   Pattern: {}", pattern);
    println!("   Command: {}", command);
    println!("   Parallel workers: {}", parallel);
    if let Some(r) = retry {
        println!("   Retry attempts: {}", r);
    }
    if let Some(t) = timeout {
        println!("   Timeout per file: {}s", t);
    }

    // Generate temporary workflow
    let (_workflow, temp_path) =
        generate_batch_workflow(&pattern, &command, parallel, retry, timeout)?;
    let _temp_workflow = TemporaryWorkflow {
        path: temp_path.clone(),
    };

    // Execute using cook command
    let cook_cmd = prodigy::cook::command::CookCommand {
        playbook: temp_path,
        path,
        max_iterations: 1,
        worktree: false,
        map: vec![],
        args: vec![],
        fail_fast: false,
        auto_accept: true,
        metrics: false,
        resume: None,
        quiet: false,
        verbosity: 0,
        dry_run: false,
    };

    prodigy::cook::cook(cook_cmd).await
}

/// Run resume workflow command
async fn run_resume_workflow(
    workflow_id: Option<String>,
    force: bool,
    from_checkpoint: Option<String>,
    path: Option<PathBuf>,
) -> anyhow::Result<()> {
    use anyhow::Context;
    use prodigy::cook::execution::claude::ClaudeExecutorImpl;
    use prodigy::cook::interaction::DefaultUserInteraction;
    use prodigy::cook::session::SessionManager;
    use prodigy::cook::workflow::{CheckpointManager, ResumeExecutor, ResumeOptions};
    use prodigy::unified_session::CookSessionAdapter;
    use std::sync::Arc;

    let working_dir = match path {
        Some(dir) => dir,
        None => std::env::current_dir().context("Failed to get current working directory")?,
    };

    // Try checkpoint-based resume first
    let checkpoint_dir = working_dir.join(".prodigy").join("checkpoints");
    let checkpoint_manager = Arc::new(CheckpointManager::new(checkpoint_dir.clone()));

    // Auto-detect workflow_id if not provided
    let workflow_id = if let Some(id) = workflow_id {
        id
    } else {
        // Try to find the most recent checkpoint
        match find_latest_checkpoint(&checkpoint_dir).await {
            Some(id) => {
                println!("✅ Auto-detected interrupted workflow: {}", id);
                id
            }
            None => {
                return Err(anyhow::anyhow!(
                    "No workflow ID provided and no checkpoints found. Please specify a workflow ID."
                ));
            }
        }
    };

    // Try to load checkpoint (always attempt, even if local dir doesn't exist - might be in global storage)
    let checkpoint_result = if let Some(ref checkpoint_id) = from_checkpoint {
        checkpoint_manager.load_checkpoint(checkpoint_id).await
    } else {
        checkpoint_manager.load_checkpoint(&workflow_id).await
    };

    match checkpoint_result {
        Ok(checkpoint) => {
            // Use pure functions for formatting
            for message in prodigy::resume_logic::format_checkpoint_status(&checkpoint) {
                println!("{}", message);
            }

            // Use workflow path from checkpoint if available, otherwise try to find it
            let workflow_path = if let Some(ref checkpoint_path) = checkpoint.workflow_path {
                // Use the path stored in the checkpoint
                checkpoint_path.clone()
            } else {
                // Fallback to searching for the workflow file
                let possible_paths = prodigy::resume_logic::possible_workflow_paths(
                    &working_dir,
                    checkpoint.workflow_name.as_deref(),
                );

                match prodigy::resume_logic::find_workflow_file(&possible_paths, |p| p.exists()) {
                    prodigy::resume_logic::WorkflowFileResult::Found(path) => path,
                    prodigy::resume_logic::WorkflowFileResult::NotFound(paths) => {
                        println!("⚠️  Could not find workflow file automatically.");
                        println!("   Please specify the workflow file path explicitly:");
                        println!("   prodigy resume <workflow_id> <workflow_file_path>");
                        println!(
                            "   Searched in: {:?}",
                            paths.iter().map(|p| p.display()).collect::<Vec<_>>()
                        );
                        std::process::exit(2); // ARGUMENT_ERROR
                    }
                    prodigy::resume_logic::WorkflowFileResult::Multiple(paths) => {
                        println!("⚠️  Multiple workflow files found:");
                        for path in &paths {
                            println!("   - {}", path.display());
                        }
                        println!("   Please specify the workflow file path explicitly:");
                        println!("   prodigy resume <workflow_id> <workflow_file_path>");
                        std::process::exit(2); // ARGUMENT_ERROR
                    }
                }
            };

            // Create resume options
            let resume_options = ResumeOptions {
                force,
                from_step: None,
                reset_failures: false,
                skip_validation: false,
            };

            // Create executors for resume
            let command_runner = prodigy::cook::execution::runner::RealCommandRunner::new();
            let resume_session_id = format!("resume-{}", workflow_id);

            // Create event logger for Claude streaming logs
            let event_logger = match prodigy::storage::create_global_event_logger(
                &working_dir,
                &resume_session_id,
            )
            .await
            {
                Ok(logger) => Some(Arc::new(logger)),
                Err(e) => {
                    tracing::warn!(
                        "Failed to create event logger for resume session {}: {}",
                        resume_session_id,
                        e
                    );
                    None
                }
            };

            let claude_executor = Arc::new({
                let mut executor = ClaudeExecutorImpl::new(command_runner);
                if let Some(logger) = event_logger {
                    executor = executor.with_event_logger(logger);
                }
                executor
            });
            let storage = prodigy::storage::GlobalStorage::new()?;
            let session_tracker =
                Arc::new(CookSessionAdapter::new(working_dir.clone(), storage).await?);
            let user_interaction = Arc::new(DefaultUserInteraction::default());

            // Create resume executor with full execution support
            let mut resume_executor = ResumeExecutor::new(checkpoint_manager.clone())
                .with_executors(
                    claude_executor.clone(),
                    session_tracker.clone(),
                    user_interaction.clone(),
                );

            // Print resume action message
            println!(
                "{}",
                prodigy::resume_logic::format_resume_action(
                    force,
                    &checkpoint.execution_state.status
                )
            );
            println!("   Workflow file: {}", workflow_path.display());

            // Calculate and display skip count
            let skip_count = prodigy::resume_logic::calculate_skip_count(&checkpoint, force);
            if skip_count > 0 {
                println!("   Skipping {} completed steps", skip_count);
            } else if force {
                println!("   Starting from the beginning (force mode)");
            }

            // Execute from checkpoint
            match resume_executor
                .execute_from_checkpoint(&workflow_id, &workflow_path, resume_options)
                .await
            {
                Ok(result) => {
                    println!("✅ Workflow resumed successfully!");
                    println!(
                        "   Executed {} new steps (skipped {})",
                        result.new_steps_executed, result.skipped_steps
                    );
                    return Ok(());
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to resume workflow: {}", e));
                }
            }
        }
        Err(e) => {
            // No checkpoint found, try session-based resume
            debug!("Checkpoint loading failed: {}", e);
            println!("No checkpoint found, checking for session state...");
        }
    }

    // Fall back to session-based resume
    let storage = prodigy::storage::GlobalStorage::new()?;
    let session_tracker = CookSessionAdapter::new(working_dir.clone(), storage).await?;

    // Check if session exists and is resumable
    match session_tracker.load_session(&workflow_id).await {
        Ok(state) => {
            if !state.is_resumable() && !force {
                // Check if session is already completed
                if state.status == prodigy::cook::session::SessionStatus::Completed {
                    println!(
                        "✅ Workflow {} already completed - nothing to resume",
                        workflow_id
                    );
                    return Ok(());
                }

                anyhow::bail!(
                    "Session {} is not resumable (status: {:?}). Use --force to override.",
                    workflow_id,
                    state.status
                );
            }

            println!("📂 Resuming workflow: {}", workflow_id);
            println!("   Status: {:?}", state.status);
            println!(
                "   Progress: {} iterations completed",
                state.iterations_completed
            );

            if let Some(workflow_state) = state.workflow_state {
                // Resume the workflow using the saved state
                let cook_cmd = prodigy::cook::command::CookCommand {
                    playbook: workflow_state.workflow_path.clone(),
                    path: Some(working_dir),
                    max_iterations: 1, // Default to 1 for resume
                    worktree: state.worktree_name.is_some(),
                    map: workflow_state.map_patterns.clone(),
                    args: workflow_state.input_args.clone(),
                    fail_fast: false,
                    auto_accept: true,
                    metrics: false,
                    resume: Some(workflow_id),
                    quiet: false,
                    verbosity: 0,
                    dry_run: false,
                };

                prodigy::cook::cook(cook_cmd).await
            } else {
                anyhow::bail!(
                    "Session {} does not have workflow state to resume",
                    workflow_id
                );
            }
        }
        Err(e) => {
            anyhow::bail!("Failed to load session {}: {}", workflow_id, e);
        }
    }
}

/// Handle fatal errors and exit with appropriate status code
fn handle_fatal_error(error: anyhow::Error) -> ! {
    use prodigy::error::ProdigyError;

    error!("Fatal error: {}", error);

    // Check if it's a ProdigyError for better handling
    let exit_code = if let Some(prodigy_err) = error.downcast_ref::<ProdigyError>() {
        // Use the user-friendly message for ProdigyError
        eprintln!("{}", prodigy_err.user_message());

        // Show developer message in debug mode
        if tracing::enabled!(tracing::Level::DEBUG) {
            eprintln!("\nDebug information:\n{}", prodigy_err.developer_message());
        }

        prodigy_err.exit_code()
    } else {
        // Fallback for non-ProdigyError errors
        eprintln!("Error: {error}");

        // Try to determine exit code based on error message
        if error.to_string().contains("No workflow ID provided")
            || error.to_string().contains("required")
            || error.to_string().contains("Please specify")
        {
            2 // ARGUMENT_ERROR
        } else {
            1 // GENERAL_ERROR
        }
    };

    std::process::exit(exit_code)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    init_tracing(cli.verbose);

    let result = execute_command(cli.command, cli.verbose).await;

    if let Err(e) = result {
        handle_fatal_error(e);
    }
}

/// Handle the list command for worktrees
async fn handle_list_command(
    worktree_manager: &prodigy::worktree::WorktreeManager,
    json: bool,
    detailed: bool,
) -> anyhow::Result<()> {
    use prodigy::worktree::SessionDisplay;

    // Get enhanced session information
    let detailed_list = worktree_manager.list_detailed().await?;

    // Display in appropriate format
    if json {
        let json_output = serde_json::to_string_pretty(&detailed_list.format_json())?;
        println!("{}", json_output);
    } else if detailed {
        println!("{}", detailed_list.format_verbose());
    } else {
        println!("{}", detailed_list.format_default());
    }

    Ok(())
}

/// Classify the merge operation type based on parameters
fn classify_merge_type(name: &Option<String>, all: bool) -> MergeType {
    if all {
        MergeType::All
    } else if let Some(n) = name {
        MergeType::Single(n.clone())
    } else {
        MergeType::Invalid
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
            // We only reach this branch if name is Some (see determine_cleanup_action)
            match name {
                Some(n) => handle_single_cleanup(worktree_manager, &n, force).await,
                None => {
                    // This should be unreachable based on determine_cleanup_action logic,
                    // but handle gracefully instead of panicking
                    Err(anyhow::anyhow!(
                        "Internal error: cleanup action Single requires a worktree name"
                    ))
                }
            }
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
    use prodigy::cook::execution::mapreduce::{MapReduceExecutor, ResumeOptions};
    use prodigy::cook::execution::state::{DefaultJobStateManager, JobStateManager};
    use prodigy::cook::orchestrator::ExecutionEnvironment;
    use prodigy::worktree::WorktreeManager;
    use std::sync::Arc;

    println!("📝 Resuming MapReduce job: {}", job_id);
    println!("  Options: force={}, max_retries={}", force, max_retries);

    // Change to specified directory if provided
    let project_root = if let Some(p) = path {
        std::env::set_current_dir(&p)?;
        p
    } else {
        std::env::current_dir()?
    };

    // Create state manager to load job checkpoint
    let state_manager: Arc<dyn JobStateManager> =
        Arc::new(DefaultJobStateManager::new_with_global(project_root.clone()).await?);

    // Load job state to validate it exists
    let job_state = match state_manager.get_job_state(&job_id).await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("❌ Failed to load job state for '{}': {}", job_id, e);
            eprintln!("   Make sure the job ID is correct and the checkpoint exists.");
            return Err(anyhow::anyhow!("Job not found or checkpoint corrupted"));
        }
    };

    // Display job status
    println!("\n📊 Job Status:");
    println!("  Job ID: {}", job_state.job_id);
    println!("  Total items: {}", job_state.total_items);
    println!(
        "  Completed: {} ({:.1}%)",
        job_state.completed_agents.len(),
        (job_state.completed_agents.len() as f64 / job_state.total_items as f64) * 100.0
    );
    println!("  Failed: {}", job_state.failed_agents.len());
    println!("  Pending: {}", job_state.pending_items.len());
    println!("  Checkpoint version: {}", job_state.checkpoint_version);

    if job_state.is_complete && !force {
        println!("\n✅ Job is already complete. Use --force to re-process failed items.");
        return Ok(());
    }

    // Prepare execution environment using the orchestrator's ExecutionEnvironment
    // Generate a unique session ID for this resume operation
    let session_id = format!("resume-{}-{}", job_id, chrono::Utc::now().timestamp());

    let env = ExecutionEnvironment {
        working_dir: Arc::new(project_root.clone()),
        project_dir: Arc::new(project_root.clone()),
        worktree_name: None,
        session_id: Arc::from(session_id.as_str()),
    };

    // Create resume options - only use fields that exist
    let options = ResumeOptions {
        force,
        max_additional_retries: max_retries,
        skip_validation: false,
        from_checkpoint: None,
    };

    println!("\n🔄 Resuming job execution...\n");

    // Create necessary components for MapReduceExecutor
    // Note: This is a simplified version - full implementation would reuse components from cook command
    use prodigy::subprocess::SubprocessManager;
    let subprocess = SubprocessManager::production();
    let worktree_manager = Arc::new(WorktreeManager::new(project_root.clone(), subprocess)?);

    // Create Claude executor using the cook module's implementation
    use prodigy::cook::execution::{ClaudeExecutor, ClaudeExecutorImpl, RealCommandRunner};
    let runner = RealCommandRunner::new();

    // Create event logger for Claude streaming logs
    let event_logger =
        match prodigy::storage::create_global_event_logger(&project_root, &env.session_id).await {
            Ok(logger) => Some(Arc::new(logger)),
            Err(e) => {
                tracing::warn!(
                    "Failed to create event logger for resume job session {}: {}",
                    env.session_id,
                    e
                );
                None
            }
        };

    let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new({
        let mut executor = ClaudeExecutorImpl::new(runner);
        if let Some(logger) = event_logger {
            executor = executor.with_event_logger(logger);
        }
        executor
    });

    // Use unified session manager through the adapter
    use prodigy::cook::session::SessionManager;
    let storage = prodigy::storage::GlobalStorage::new()?;
    let session_manager: Arc<dyn SessionManager> = Arc::new(
        prodigy::unified_session::CookSessionAdapter::new(project_root.clone(), storage).await?,
    );

    // Create user interaction handler
    use prodigy::cook::interaction::{DefaultUserInteraction, UserInteraction};
    let user_interaction: Arc<dyn UserInteraction> = Arc::new(DefaultUserInteraction::new());

    // Create MapReduce executor
    let executor = MapReduceExecutor::new(
        claude_executor,
        session_manager,
        user_interaction,
        worktree_manager,
        project_root,
    )
    .await;

    // Resume the job
    match executor
        .resume_job_with_options(&job_id, options, &env)
        .await
    {
        Ok(result) => {
            println!("\n✅ Job resumed successfully!");
            println!(
                "  Resumed from checkpoint version: {}",
                result.resumed_from_version
            );
            println!("  Items already completed: {}", result.already_completed);
            println!("  Items processed in this run: {}", result.remaining_items);
            println!(
                "  Total successful: {}",
                result
                    .final_results
                    .iter()
                    .filter(|r| matches!(
                        r.status,
                        prodigy::cook::execution::mapreduce::AgentStatus::Success
                    ))
                    .count()
            );

            let failed_count = result
                .final_results
                .iter()
                .filter(|r| {
                    matches!(
                        r.status,
                        prodigy::cook::execution::mapreduce::AgentStatus::Failed(_)
                    )
                })
                .count();

            if failed_count > 0 {
                println!("  ⚠️  Failed items: {}", failed_count);
                println!(
                    "     Check the Dead Letter Queue for details: prodigy dlq list {}",
                    job_id
                );
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("\n❌ Failed to resume job: {}", e);
            Err(anyhow::anyhow!("Job resumption failed: {}", e))
        }
    }
}

// Pure helper functions for DLQ operations
fn create_dlq_filter(eligible: bool) -> prodigy::cook::execution::dlq::DLQFilter {
    prodigy::cook::execution::dlq::DLQFilter {
        reprocess_eligible: if eligible { Some(true) } else { None },
        ..Default::default()
    }
}

fn format_dlq_item_display(item: &prodigy::cook::execution::dlq::DeadLetteredItem) -> String {
    format!(
        "ID: {}\n  Last Attempt: {}\n  Failure Count: {}\n  Error: {}\n  Reprocess Eligible: {}",
        item.item_id,
        item.last_attempt,
        item.failure_count,
        item.error_signature,
        item.reprocess_eligible
    )
}

async fn resolve_job_id(
    provided_job_id: Option<String>,
    project_root: &std::path::Path,
) -> anyhow::Result<String> {
    match provided_job_id {
        Some(id) => Ok(id),
        None => {
            let available_jobs = prodigy::storage::discover_dlq_job_ids(project_root).await?;

            if available_jobs.is_empty() {
                anyhow::bail!("No DLQ data found. Run a MapReduce job first to generate DLQ data.");
            }

            if available_jobs.len() == 1 {
                Ok(available_jobs
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("No jobs available after filtering"))?)
            } else {
                println!("Multiple jobs with DLQ data found:");
                for (i, job_id) in available_jobs.iter().enumerate() {
                    println!("  {}: {}", i + 1, job_id);
                }
                anyhow::bail!("Multiple job IDs found. Use --job-id to specify which one to use.");
            }
        }
    }
}

async fn get_dlq_instance(
    job_id: &str,
    project_root: &std::path::Path,
) -> anyhow::Result<prodigy::cook::execution::dlq::DeadLetterQueue> {
    use prodigy::cook::execution::dlq::DeadLetterQueue;
    use prodigy::storage::{extract_repo_name, GlobalStorage};

    let storage = GlobalStorage::new()?;
    let repo_name = extract_repo_name(project_root)?;
    let dlq_dir = storage.get_dlq_dir(&repo_name, job_id).await?;
    DeadLetterQueue::new(job_id.to_string(), dlq_dir, 10000, 30, None).await
}

fn display_dlq_items(
    items: &[prodigy::cook::execution::dlq::DeadLetteredItem],
    job_id: &str,
    limit: usize,
) {
    let display_items: Vec<_> = items.iter().take(limit).collect();

    if display_items.is_empty() {
        println!("No items in Dead Letter Queue for job {}", job_id);
    } else {
        println!("Dead Letter Queue items for job {}:", job_id);
        println!("{:─<80}", "");
        for item in display_items {
            println!("{}", format_dlq_item_display(item));
            println!("{:─<80}", "");
        }
    }
}

async fn run_dlq_command(command: DlqCommands) -> anyhow::Result<()> {
    use chrono::{Duration, Utc};

    let project_root = std::env::current_dir()?;

    match command {
        DlqCommands::List {
            job_id,
            eligible,
            limit,
        } => {
            let resolved_job_id = resolve_job_id(job_id, &project_root).await?;
            let dlq = get_dlq_instance(&resolved_job_id, &project_root).await?;
            let filter = create_dlq_filter(eligible);
            let items = dlq.list_items(filter).await?;

            display_dlq_items(&items, &resolved_job_id, limit);
        }
        DlqCommands::Inspect { item_id, job_id } => {
            let resolved_job_id = resolve_job_id(job_id, &project_root).await?;
            let dlq = get_dlq_instance(&resolved_job_id, &project_root).await?;

            if let Some(item) = dlq.get_item(&item_id).await? {
                println!("Dead Letter Queue Item Details:");
                println!("{}", serde_json::to_string_pretty(&item)?);
            } else {
                println!("Item {} not found in DLQ", item_id);
            }
        }
        DlqCommands::Analyze { job_id, export } => {
            let resolved_job_id = resolve_job_id(job_id, &project_root).await?;
            let dlq = get_dlq_instance(&resolved_job_id, &project_root).await?;

            let analysis = dlq.analyze_patterns().await?;

            if let Some(export_path) = export {
                let json = serde_json::to_string_pretty(&analysis)?;
                std::fs::write(&export_path, json)?;
                println!("Analysis exported to {:?}", export_path);
            } else {
                println!("Dead Letter Queue Analysis for job {}:", resolved_job_id);
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
            let resolved_job_id = resolve_job_id(job_id, &project_root).await?;
            let dlq = get_dlq_instance(&resolved_job_id, &project_root).await?;

            dlq.export_items(&output).await?;
            println!("DLQ items exported to {:?} in {} format", output, format);
        }
        DlqCommands::Retry {
            workflow_id,
            filter,
            max_retries,
            parallel,
            force,
        } => {
            use prodigy::cook::execution::dlq_reprocessor::{
                DlqFilterAdvanced, DlqReprocessor, ErrorType, ReprocessOptions, RetryStrategy,
            };
            use std::sync::Arc;

            println!("Starting DLQ reprocessing for workflow: {}", workflow_id);

            // Get DLQ instance
            let dlq = get_dlq_instance(&workflow_id, &project_root).await?;
            let dlq_arc = Arc::new(dlq);

            // Create reprocessor
            let reprocessor = DlqReprocessor::new(
                dlq_arc.clone(),
                None, // Event logger
                project_root.clone(),
            );

            // Parse filter if provided
            let advanced_filter = if let Some(filter_str) = filter {
                // Parse simple filter expressions into advanced filter
                let mut adv_filter = DlqFilterAdvanced {
                    error_types: None,
                    date_range: None,
                    item_filter: None,
                    max_failure_count: None,
                };

                // Check for common filter patterns
                if filter_str.contains("error_type=") {
                    // Parse error type filter
                    if filter_str.contains("timeout") {
                        adv_filter.error_types = Some(vec![ErrorType::Timeout]);
                    } else if filter_str.contains("validation") {
                        adv_filter.error_types = Some(vec![ErrorType::Validation]);
                    } else if filter_str.contains("command") {
                        adv_filter.error_types = Some(vec![ErrorType::CommandFailure]);
                    }
                } else if filter_str.contains("failure_count") {
                    // Parse failure count filter
                    if let Some(num_str) = filter_str.split('=').nth(1) {
                        if let Ok(num) = num_str.trim().parse::<u32>() {
                            adv_filter.max_failure_count = Some(num);
                        }
                    }
                } else {
                    // Use as item filter expression
                    adv_filter.item_filter = Some(filter_str);
                }

                Some(adv_filter)
            } else {
                None
            };

            // Create reprocess options
            let options = ReprocessOptions {
                max_retries,
                filter: advanced_filter,
                parallel,
                timeout_per_item: 300,
                strategy: RetryStrategy::ExponentialBackoff,
                merge_results: true,
                force,
            };

            // Execute reprocessing
            println!("Configuration:");
            println!("  Max retries: {}", options.max_retries);
            println!("  Parallel workers: {}", options.parallel);
            println!("  Force reprocessing: {}", options.force);
            if let Some(ref f) = options.filter {
                if let Some(ref types) = f.error_types {
                    println!("  Error type filter: {:?}", types);
                }
                if let Some(ref expr) = f.item_filter {
                    println!("  Item filter: {}", expr);
                }
                if let Some(max) = f.max_failure_count {
                    println!("  Max failure count: {}", max);
                }
            }
            println!();

            // Perform the actual reprocessing
            match reprocessor.reprocess_items(options).await {
                Ok(result) => {
                    println!("\n✅ DLQ Reprocessing completed!");
                    println!("\nSummary:");
                    println!("  Total items processed: {}", result.total_items);
                    println!("  Successful: {} ✓", result.successful);
                    println!("  Failed: {} ✗", result.failed);
                    if result.skipped > 0 {
                        println!("  Skipped: {} ⊘", result.skipped);
                    }
                    println!("  Duration: {:?}", result.duration);
                    println!("  Job ID: {}", result.job_id);

                    if !result.error_patterns.is_empty() {
                        println!("\nError patterns:");
                        for (pattern, count) in &result.error_patterns {
                            println!("  {}: {}", pattern, count);
                        }
                    }

                    if !result.failed_items.is_empty() {
                        println!("\nFailed items ({}):", result.failed_items.len());
                        for (i, item) in result.failed_items.iter().take(5).enumerate() {
                            println!("  {}. {}", i + 1, item);
                        }
                        if result.failed_items.len() > 5 {
                            println!("  ... and {} more", result.failed_items.len() - 5);
                        }
                    }

                    if result.failed > 0 {
                        println!("\n⚠️  Some items failed reprocessing. Review the failed items and consider:");
                        println!("  - Adjusting retry parameters");
                        println!("  - Fixing underlying issues");
                        println!("  - Manual intervention for persistent failures");
                    }
                }
                Err(e) => {
                    eprintln!("❌ DLQ reprocessing failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        DlqCommands::Stats { workflow_id } => {
            if let Some(wf_id) = workflow_id {
                // Show stats for specific workflow
                let dlq = get_dlq_instance(&wf_id, &project_root).await?;
                let stats = dlq.get_stats().await?;

                println!("DLQ Statistics for workflow {}:", wf_id);
                println!("  Total items: {}", stats.total_items);
                println!("  Eligible for reprocess: {}", stats.eligible_for_reprocess);
                println!(
                    "  Requiring manual review: {}",
                    stats.requiring_manual_review
                );
                if let Some(oldest) = stats.oldest_item {
                    println!("  Oldest item: {}", oldest);
                }
                if let Some(newest) = stats.newest_item {
                    println!("  Newest item: {}", newest);
                }
            } else {
                // Show global stats
                println!("Global DLQ Statistics:");
                println!("  Note: Global stats aggregation is not yet fully implemented");
                println!("  Use --workflow-id to see stats for a specific workflow");
            }
        }
        DlqCommands::Clear { workflow_id, yes } => {
            use std::sync::Arc;

            if !yes {
                println!("This will permanently delete all processed items from the DLQ.");
                println!("Continue? (y/N)");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Clear cancelled.");
                    return Ok(());
                }
            }

            let dlq = get_dlq_instance(&workflow_id, &project_root).await?;
            let dlq_arc = Arc::new(dlq);

            let reprocessor = prodigy::cook::execution::dlq_reprocessor::DlqReprocessor::new(
                dlq_arc,
                None,
                project_root.clone(),
            );

            let count = reprocessor.clear_processed_items(&workflow_id).await?;
            println!(
                "Cleared {} processed items from DLQ for workflow {}",
                count, workflow_id
            );
        }
        DlqCommands::Purge {
            older_than_days,
            job_id,
            yes,
        } => {
            let resolved_job_id = resolve_job_id(job_id, &project_root).await?;

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

            let dlq = get_dlq_instance(&resolved_job_id, &project_root).await?;

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
        EventCommands::Ls {
            job_id,
            event_type,
            agent_id,
            since,
            limit,
            file,
        } => EventsArgs {
            command: EventsCommand::Ls {
                job_id,
                event_type,
                agent_id,
                since,
                limit,
                file,
                output_format: "human".to_string(),
            },
        },
        EventCommands::Stats { file, group_by } => EventsArgs {
            command: EventsCommand::Stats {
                file,
                group_by,
                output_format: "human".to_string(),
            },
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
        EventCommands::Clean {
            older_than,
            max_events,
            max_size,
            dry_run,
            archive,
            archive_path,
            all_jobs,
            job_id,
            file,
        } => EventsArgs {
            command: EventsCommand::Clean {
                older_than,
                max_events,
                max_size,
                dry_run,
                archive,
                archive_path,
                all_jobs,
                job_id,
                file,
                output_format: "human".to_string(),
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
    use prodigy::cook::session::SessionManager;
    use prodigy::unified_session::CookSessionAdapter;

    let working_dir = std::env::current_dir()?;
    let storage = prodigy::storage::GlobalStorage::new()?;
    let session_tracker = CookSessionAdapter::new(working_dir.clone(), storage).await?;

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
                        println!("  prodigy run <workflow> --resume {}", session_id);
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

async fn run_progress_command(
    job_id: String,
    export: Option<PathBuf>,
    format: String,
    web: Option<u16>,
) -> anyhow::Result<()> {
    use prodigy::cook::execution::progress::{
        CLIProgressViewer, EnhancedProgressTracker, ExportFormat,
    };
    use std::sync::Arc;

    // Create progress tracker
    let tracker = Arc::new(EnhancedProgressTracker::new(job_id.clone(), 0));

    // Start web dashboard if requested
    if let Some(port) = web {
        let mut tracker_mut = EnhancedProgressTracker::new(job_id.clone(), 0);
        if let Err(e) = tracker_mut.start_web_server(port).await {
            eprintln!("Failed to start web server: {}", e);
        } else {
            println!("Progress dashboard available at http://localhost:{}", port);
            println!("Press Ctrl+C to stop...");

            // Keep the server running
            tokio::signal::ctrl_c().await?;
        }
        return Ok(());
    }

    // Export progress data if requested
    if let Some(output_path) = export {
        let export_format = match format.as_str() {
            "csv" => ExportFormat::Csv,
            "html" => ExportFormat::Html,
            _ => ExportFormat::Json,
        };

        let data = tracker.export_progress(export_format).await?;
        std::fs::write(&output_path, data)?;
        println!("Progress data exported to: {}", output_path.display());
        return Ok(());
    }

    // Otherwise, show CLI progress viewer
    let viewer = CLIProgressViewer::new(tracker);
    viewer.display().await?;

    Ok(())
}

async fn run_worktree_command(command: WorktreeCommands) -> anyhow::Result<()> {
    use prodigy::subprocess::SubprocessManager;
    use prodigy::worktree::WorktreeManager;

    let subprocess = SubprocessManager::production();
    let worktree_manager = WorktreeManager::new(std::env::current_dir()?, subprocess)?;

    match command {
        WorktreeCommands::Ls { json, detailed } => {
            handle_list_command(&worktree_manager, json, detailed).await
        }
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

/// Run migrate-yaml command to convert workflows to simplified syntax
async fn run_migrate_yaml_command(
    path: Option<PathBuf>,
    backup: bool,
    dry_run: bool,
    force: bool,
) -> anyhow::Result<()> {
    use prodigy::cli::yaml_migrator::YamlMigrator;

    let target = path.unwrap_or_else(|| PathBuf::from("workflows"));

    if dry_run {
        println!("🔍 Running migration check (dry run)...");
    } else {
        println!("📝 Migrating YAML files to simplified syntax...");
    }

    let migrator = YamlMigrator::new(backup && !force);
    let results = if target.is_file() {
        vec![migrator.migrate_file(&target, dry_run)?]
    } else {
        migrator.migrate_directory(&target, dry_run)?
    };

    // Print summary
    let migrated_count = results.iter().filter(|r| r.was_migrated).count();
    let error_count = results.iter().filter(|r| r.error.is_some()).count();

    if migrated_count > 0 {
        println!(
            "✅ Migrated {} file(s) to simplified syntax",
            migrated_count
        );
    }
    if error_count > 0 {
        println!("⚠️  {} file(s) had errors", error_count);
    }
    if migrated_count == 0 && error_count == 0 {
        println!("ℹ️  No files needed migration");
    }

    Ok(())
}

/// Run validate command to check workflow format
async fn run_validate_command(
    workflow: PathBuf,
    format: String,
    suggest: bool,
    strict: bool,
) -> anyhow::Result<()> {
    use prodigy::cli::yaml_validator::YamlValidator;

    println!("🔍 Validating workflow: {}", workflow.display());

    let validator = YamlValidator::new(format == "simplified");
    let result = validator.validate_file(&workflow)?;

    if result.is_valid {
        println!("✅ Workflow is valid and uses {} format", format);
    } else {
        println!("⚠️  Workflow validation issues found:");
        for issue in &result.issues {
            println!("   - {}", issue);
        }
    }

    if suggest && !result.suggestions.is_empty() {
        println!("\n💡 Suggestions for improvement:");
        for suggestion in &result.suggestions {
            println!("   - {}", suggestion);
        }
    }

    if strict && !result.is_valid {
        return Err(anyhow::anyhow!("Workflow validation failed"));
    }

    Ok(())
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
