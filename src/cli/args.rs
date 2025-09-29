//! CLI argument structures
//!
//! This module defines all command-line interface structures used by Prodigy.
//! It includes the main CLI structure and all subcommand definitions.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Execute automated workflows with zero configuration
#[derive(Parser)]
#[command(name = "prodigy")]
#[command(about = "prodigy - Execute automated workflows with zero configuration", long_about = None)]
#[command(version)]
pub struct Cli {
    /// Enable verbose output (-v for debug, -vv for trace, -vvv for all)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
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
        #[arg(long, value_name = "SESSION_ID")]
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
pub enum SessionCommands {
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
pub enum CheckpointCommands {
    /// List all available checkpoints
    #[command(name = "list", alias = "ls")]
    List {
        /// Filter by workflow ID
        #[arg(long)]
        workflow_id: Option<String>,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
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

    /// Validate checkpoint integrity
    #[command(name = "validate")]
    Validate {
        /// Checkpoint ID
        checkpoint_id: String,

        /// Attempt to repair if corrupt
        #[arg(long)]
        repair: bool,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
    },

    /// List MapReduce checkpoints
    #[command(name = "mapreduce")]
    MapReduce {
        /// Job ID to list checkpoints for
        job_id: String,

        /// Show detailed information
        #[arg(long)]
        detailed: bool,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
    },

    /// Delete a specific checkpoint
    #[command(name = "delete")]
    Delete {
        /// Checkpoint ID
        checkpoint_id: String,

        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,

        /// Working directory
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum EventCommands {
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
pub enum DlqCommands {
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
pub enum WorktreeCommands {
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
        /// Clean MapReduce-specific worktrees
        #[arg(long)]
        mapreduce: bool,
        /// Clean worktrees older than specified duration (e.g., "1h", "24h")
        #[arg(long)]
        older_than: Option<String>,
        /// Show what would be cleaned without actually cleaning
        #[arg(long)]
        dry_run: bool,
        /// Specific job ID to clean
        #[arg(long)]
        job_id: Option<String>,
    },
}
