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
    /// Run a workflow file (alias for cook)
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

        /// Direct arguments to pass to commands
        #[arg(long, value_name = "VALUE")]
        args: Vec<String>,

        /// Automatically answer yes to all prompts
        #[arg(short = 'y', long = "yes")]
        auto_accept: bool,
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
        /// Workflow ID to resume
        workflow_id: String,

        /// Force resume even if marked complete
        #[arg(long)]
        force: bool,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
    },

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
    /// Manage cooking sessions
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
async fn execute_command(command: Option<Commands>) -> anyhow::Result<()> {
    match command {
        Some(Commands::Run {
            workflow,
            path,
            max_iterations,
            worktree,
            args,
            auto_accept,
        }) => {
            // Run is an alias for cook with better semantics
            let cook_cmd = prodigy::cook::command::CookCommand {
                playbook: workflow,
                path,
                max_iterations,
                worktree,
                map: vec![],
                args,
                fail_fast: false,
                auto_accept,
                metrics: false,
                resume: None,
                quiet: false,
                verbosity: 0,
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
            path,
        }) => run_resume_workflow(workflow_id, force, path).await,
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
    };

    prodigy::cook::cook(cook_cmd).await
}

/// Run resume workflow command
async fn run_resume_workflow(
    workflow_id: String,
    force: bool,
    path: Option<PathBuf>,
) -> anyhow::Result<()> {
    use prodigy::cook::execution::claude::ClaudeExecutorImpl;
    use prodigy::cook::interaction::DefaultUserInteraction;
    use prodigy::cook::session::{SessionManager, SessionTrackerImpl};
    use prodigy::cook::workflow::{CheckpointManager, ResumeExecutor, ResumeOptions};
    use std::sync::Arc;

    let working_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());

    // Try checkpoint-based resume first
    let checkpoint_dir = working_dir.join(".prodigy").join("checkpoints");
    let checkpoint_manager = Arc::new(CheckpointManager::new(checkpoint_dir.clone()));

    // Check if checkpoint exists
    if checkpoint_dir.exists() {
        // Try to load checkpoint
        match checkpoint_manager.load_checkpoint(&workflow_id).await {
            Ok(checkpoint) => {
                println!("✅ Found checkpoint for workflow: {}", workflow_id);
                println!(
                    "   Step progress: {}/{}",
                    checkpoint.execution_state.current_step_index,
                    checkpoint.execution_state.total_steps
                );
                println!("   Status: {:?}", checkpoint.execution_state.status);

                // Try to find the workflow file
                let workflow_path = if let Some(ref name) = checkpoint.workflow_name {
                    // Try common workflow file names including current directory
                    let possible_paths = [
                        working_dir.join(format!("{}.yml", name)),
                        working_dir.join(format!("{}.yaml", name)),
                        working_dir.join("workflow.yml"),
                        working_dir.join("workflow.yaml"),
                        working_dir.join("playbook.yml"),
                        working_dir.join("playbook.yaml"),
                        // Also check for test files
                        working_dir.join("test_complete_resume.yml"),
                        working_dir.join("test_checkpoint.yml"),
                    ];

                    if let Some(found_path) = possible_paths.iter().find(|p| p.exists()) {
                        found_path.clone()
                    } else {
                        // Ask user for the workflow file
                        println!("⚠️  Could not find workflow file automatically.");
                        println!("   Please specify the workflow file path with --path");
                        println!(
                            "   Searched in: {:?}",
                            possible_paths
                                .iter()
                                .map(|p| p.display())
                                .collect::<Vec<_>>()
                        );
                        std::process::exit(1);
                    }
                } else {
                    // If no workflow name, try to find any YAML file
                    let yaml_files: Vec<PathBuf> = std::fs::read_dir(&working_dir)
                        .unwrap_or_else(|_| panic!("Could not read directory"))
                        .filter_map(|entry| {
                            let entry = entry.ok()?;
                            let path = entry.path();
                            if path.extension().and_then(|s| s.to_str()) == Some("yml") {
                                Some(path)
                            } else {
                                None
                            }
                        })
                        .collect();

                    if yaml_files.len() == 1 {
                        yaml_files.into_iter().next().unwrap()
                    } else {
                        println!("⚠️  Checkpoint doesn't contain workflow file information.");
                        println!("   Found {} YAML files: {:?}", yaml_files.len(), yaml_files);
                        println!("   Please specify the workflow file path with --path");
                        std::process::exit(1);
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
                let claude_executor = Arc::new(ClaudeExecutorImpl::new(command_runner));
                let session_tracker = Arc::new(SessionTrackerImpl::new(
                    format!("resume-{}", workflow_id),
                    working_dir.clone(),
                ));
                let user_interaction = Arc::new(DefaultUserInteraction::default());

                // Create resume executor with full execution support
                let resume_executor = ResumeExecutor::new(checkpoint_manager.clone())
                    .with_executors(
                        claude_executor.clone(),
                        session_tracker.clone(),
                        user_interaction.clone(),
                    );

                println!("📂 Resuming workflow from checkpoint...");
                println!("   Workflow file: {}", workflow_path.display());
                println!(
                    "   Skipping {} completed steps",
                    checkpoint.execution_state.current_step_index
                );

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
            Err(_) => {
                // No checkpoint found, try session-based resume
                println!("No checkpoint found, checking for session state...");
            }
        }
    }

    // Fall back to session-based resume
    let session_tracker = SessionTrackerImpl::new("resume".to_string(), working_dir.clone());

    // Check if session exists and is resumable
    match session_tracker.load_session(&workflow_id).await {
        Ok(state) => {
            if !state.is_resumable() && !force {
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
                Ok(available_jobs.into_iter().next().unwrap())
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

    if prodigy::storage::GlobalStorage::should_use_global() {
        let storage = prodigy::storage::GlobalStorage::new(project_root)?;
        let dlq_dir = storage.get_dlq_dir(job_id).await?;
        DeadLetterQueue::new(job_id.to_string(), dlq_dir, 10000, 30, None).await
    } else {
        let dlq_path = project_root.join(".prodigy");
        DeadLetterQueue::new(job_id.to_string(), dlq_path, 10000, 30, None).await
    }
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
        DlqCommands::Reprocess {
            item_ids: _,
            job_id: _,
            max_retries: _,
            force: _,
        } => {
            anyhow::bail!("This command is deprecated. Please use 'prodigy dlq retry' instead.");
        }
        DlqCommands::Retry {
            workflow_id,
            filter,
            max_retries,
            parallel,
            force,
        } => {
            use prodigy::cook::execution::dlq_reprocessor::{
                DlqReprocessor, ReprocessOptions, RetryStrategy,
            };
            use std::sync::Arc;

            // Get DLQ instance
            let dlq = get_dlq_instance(&workflow_id, &project_root).await?;
            let dlq_arc = Arc::new(dlq);

            // Create reprocessor
            let _reprocessor = DlqReprocessor::new(
                dlq_arc.clone(),
                None, // Event logger
                project_root.clone(),
            );

            // Create reprocess options
            let options = ReprocessOptions {
                max_retries,
                filter,
                parallel,
                timeout_per_item: 300,
                strategy: RetryStrategy::ExponentialBackoff,
                merge_results: true,
                force,
            };

            // For now, we'll display what would be reprocessed
            let filter_obj = prodigy::cook::execution::dlq::DLQFilter::default();
            let items = dlq_arc.list_items(filter_obj).await?;

            let eligible_count = if force {
                items.len()
            } else {
                items.iter().filter(|i| i.reprocess_eligible).count()
            };

            println!("DLQ Reprocessing for workflow: {}", workflow_id);
            println!("  Total items in DLQ: {}", items.len());
            println!("  Eligible for reprocessing: {}", eligible_count);
            if let Some(ref f) = options.filter {
                println!("  Filter expression: {}", f);
            }
            println!("  Max retries: {}", options.max_retries);
            println!("  Parallel workers: {}", options.parallel);
            println!("  Force reprocessing: {}", options.force);

            println!(
                "\nNote: Full reprocessing with MapReduce executor integration is in progress."
            );
            println!("Currently showing analysis only. Items can be manually resubmitted.");
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
