use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=.claude/commands/");
    println!("cargo:rerun-if-changed=src/main.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    // Generate command templates first
    generate_command_templates();

    // Generate man pages
    #[cfg(not(doc))]
    generate_man_pages();
}

fn generate_command_templates() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("command_includes.rs");

    // Get the manifest directory (where Cargo.toml is)
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = Path::new(&manifest_dir);
    let commands_dir = manifest_path.join(".claude").join("commands");

    let mut includes = String::new();
    let mut templates = String::new();

    // Scan for all prodigy-*.md files
    if commands_dir.exists() {
        let mut command_files: Vec<_> = fs::read_dir(&commands_dir)
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                let file_name = path.file_name()?.to_str()?;

                // Only include prodigy-*.md files
                if file_name.starts_with("prodigy-") && file_name.ends_with(".md") {
                    // Use absolute path for include_str!
                    let absolute_path = path.canonicalize().ok()?;

                    Some((
                        file_name.trim_end_matches(".md").to_string(),
                        absolute_path.to_str()?.replace('\\', "/"),
                    ))
                } else {
                    None
                }
            })
            .collect();

        // Sort for consistent ordering
        command_files.sort_by(|a, b| a.0.cmp(&b.0));

        // Generate const declarations for each command
        for (name, path) in &command_files {
            let const_name = name.to_uppercase().replace('-', "_");
            includes.push_str(&format!(
                "pub const {}: &str = include_str!(\"{}\");\n",
                const_name, path
            ));
        }

        // Generate the get_all_command_templates function
        templates.push_str("pub fn get_all_command_templates() -> Vec<CommandTemplate> {\n");
        templates.push_str("    vec![\n");

        for (name, _) in &command_files {
            let const_name = name.to_uppercase().replace('-', "_");
            let description = extract_description(name);

            templates.push_str(&format!(
                "        CommandTemplate {{\n            name: \"{}\",\n            description: \"{}\",\n            content: {},\n        }},\n",
                name, description, const_name
            ));
        }

        templates.push_str("    ]\n");
        templates.push_str("}\n");
    } else {
        // If no commands directory, return empty
        templates.push_str("pub fn get_all_command_templates() -> Vec<CommandTemplate> {\n");
        templates.push_str("    vec![]\n");
        templates.push_str("}\n");
    }

    // Write the generated code
    let generated_code = format!("{}\n{}", includes, templates);
    fs::write(&dest_path, generated_code).unwrap();
}

#[cfg(not(doc))]
fn generate_man_pages() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let man_dir = out_dir.join("man");
    fs::create_dir_all(&man_dir).unwrap();

    // Generate man pages
    let app = build_cli();
    generate_man_for_command(&app, &man_dir, None);

    // Generate man pages for all subcommands
    for subcommand in app.get_subcommands() {
        generate_man_for_command(subcommand, &man_dir, Some(&app));

        // Generate for nested subcommands
        for nested in subcommand.get_subcommands() {
            generate_man_for_command(nested, &man_dir, Some(subcommand));
        }
    }

    // Create installation directory in the target directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let target_man_dir = PathBuf::from(manifest_dir).join("target").join("man");
    if !target_man_dir.exists() {
        fs::create_dir_all(&target_man_dir).unwrap();
    }

    // Copy man pages to target directory for easier installation
    for entry in fs::read_dir(&man_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "1" || e == "gz") {
            let dest = target_man_dir.join(path.file_name().unwrap());
            fs::copy(&path, &dest).ok();
        }
    }

    println!("cargo:note=Generated man pages in {}", man_dir.display());
}

#[cfg(not(doc))]
fn generate_man_for_command(cmd: &clap::Command, man_dir: &Path, parent: Option<&clap::Command>) {
    use clap_mangen::Man;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs::File;
    use std::io::Write;

    let cmd_name = if let Some(parent) = parent {
        format!("{}-{}", parent.get_name(), cmd.get_name())
    } else {
        cmd.get_name().to_string()
    };

    // Create man page with proper metadata
    let man = Man::new(cmd.clone())
        .title(cmd_name.to_uppercase())
        .section("1")
        .date(chrono::Utc::now().format("%Y-%m-%d").to_string())
        .source("Prodigy")
        .manual("Prodigy Manual");

    // Generate the man page content
    let mut buffer = Vec::new();
    man.render(&mut buffer).unwrap();

    // Add additional sections if this is the main command
    if parent.is_none() {
        let additional = r#"
.SH EXAMPLES
.PP
Run a workflow:
.RS 4
prodigy run workflow.yml
.RE
.PP
Execute a single command with retries:
.RS 4
prodigy exec "claude: /refactor app.py" --retry 3
.RE
.PP
Process files in parallel:
.RS 4
prodigy batch "*.py" --command "claude: /add-types" --parallel 5
.RE
.PP
Resume an interrupted workflow:
.RS 4
prodigy resume
.RE
.PP
Run goal-seeking operation:
.RS 4
prodigy goal-seek "Fix all tests" --command "claude: /debug" --validate "cargo test"
.RE
.SH FILES
.PP
~/.prodigy/
    Global storage for events, state, and worktrees
.PP
.prodigy/
    Local project configuration and session state
.PP
.claude/commands/
    Custom Claude command definitions
.PP
workflows/
    Workflow YAML files
.SH ENVIRONMENT
.PP
PRODIGY_AUTOMATION
    Set to "true" to enable automation mode
.PP
PRODIGY_USE_LOCAL_STORAGE
    Set to "true" to use local storage instead of global
.PP
PRODIGY_REMOVE_LOCAL_AFTER_MIGRATION
    Set to "true" to remove local storage after migration
.SH EXIT STATUS
.PP
0
    Successful execution
.PP
1
    General error
.PP
2
    Command line parsing error
.PP
3
    Workflow validation error
.PP
130
    Process interrupted (SIGINT)
.SH SEE ALSO
.PP
prodigy-run(1), prodigy-exec(1), prodigy-batch(1), prodigy-goal-seek(1)
.PP
Full documentation at: <https://github.com/iepathos/prodigy>
.SH BUGS
.PP
Report bugs at: <https://github.com/iepathos/prodigy/issues>
.SH AUTHOR
.PP
Glen Baker <iepathos@gmail.com>
"#
        .to_string();
        buffer.extend_from_slice(additional.as_bytes());
    }

    // Write uncompressed man page
    let man_file = man_dir.join(format!("{}.1", cmd_name));
    let mut file = File::create(&man_file).unwrap();
    file.write_all(&buffer).unwrap();

    // Write compressed version
    let gz_file = man_dir.join(format!("{}.1.gz", cmd_name));
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&buffer).unwrap();
    let compressed = encoder.finish().unwrap();
    fs::write(gz_file, compressed).unwrap();
}

#[cfg(not(doc))]
fn build_cli() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("prodigy")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Glen Baker <iepathos@gmail.com>")
        .about("Cook your code to perfection with zero configuration")
        .long_about("Prodigy - A workflow orchestration tool that executes Claude commands through structured YAML workflows with session state management and parallel execution through MapReduce patterns.\n\nProdigy helps you turn ad-hoc Claude sessions into reproducible development pipelines with parallel AI agents.")
        .arg(Arg::new("verbose")
            .short('v')
            .long("verbose")
            .help("Enable verbose output (-v for debug, -vv for trace, -vvv for all)")
            .action(clap::ArgAction::Count)
            .global(true))
        .subcommands(vec![
            build_run_subcommand(),
            build_exec_subcommand(),
            build_batch_subcommand(),
            build_resume_subcommand(),
            build_goal_seek_subcommand(),
            build_worktree_subcommand(),
            build_init_subcommand(),
            build_events_subcommand(),
            build_dlq_subcommand(),
            build_sessions_subcommand(),
            build_analytics_subcommand(),
            build_checkpoints_subcommand(),
            build_progress_subcommand(),
            build_resume_job_subcommand(),
            build_validate_subcommand(),
            build_migrate_yaml_subcommand(),
        ])
}

#[cfg(not(doc))]
fn build_run_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("run")
        .about("Run a workflow file")
        .long_about("Execute a workflow file with support for sequential and parallel command execution, MapReduce patterns, and goal-seeking operations.")
        .arg(Arg::new("workflow")
            .help("Workflow file to execute")
            .required(true)
            .value_parser(clap::value_parser!(PathBuf)))
        .arg(Arg::new("path")
            .short('p')
            .long("path")
            .help("Repository path to run in")
            .value_parser(clap::value_parser!(PathBuf)))
        .arg(Arg::new("max-iterations")
            .short('n')
            .long("max-iterations")
            .help("Maximum number of iterations")
            .default_value("1"))
        .arg(Arg::new("worktree")
            .short('w')
            .long("worktree")
            .help("Run in an isolated git worktree")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("args")
            .long("args")
            .help("Direct arguments to pass to commands")
            .action(clap::ArgAction::Append))
        .arg(Arg::new("yes")
            .short('y')
            .long("yes")
            .help("Automatically answer yes to all prompts")
            .action(clap::ArgAction::SetTrue))
}

#[cfg(not(doc))]
fn build_exec_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("exec")
        .about("Execute a single command with retry support")
        .long_about("Execute a single Claude or shell command with configurable retry attempts and timeout settings.")
        .arg(Arg::new("command")
            .help("Command to execute")
            .required(true))
        .arg(Arg::new("retry")
            .long("retry")
            .help("Number of retry attempts")
            .default_value("1"))
        .arg(Arg::new("timeout")
            .long("timeout")
            .help("Timeout in seconds"))
        .arg(Arg::new("path")
            .short('p')
            .long("path")
            .help("Working directory")
            .value_parser(clap::value_parser!(PathBuf)))
}

#[cfg(not(doc))]
fn build_batch_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("batch")
        .about("Process multiple files in parallel")
        .long_about("Process multiple files matching a pattern in parallel using MapReduce, with configurable parallelism and retry settings.")
        .arg(Arg::new("pattern")
            .help("File pattern to match")
            .required(true))
        .arg(Arg::new("command")
            .long("command")
            .help("Command to execute for each file")
            .required(true))
        .arg(Arg::new("parallel")
            .long("parallel")
            .help("Number of parallel workers")
            .default_value("5"))
        .arg(Arg::new("retry")
            .long("retry")
            .help("Number of retry attempts per file"))
        .arg(Arg::new("timeout")
            .long("timeout")
            .help("Timeout per file in seconds"))
        .arg(Arg::new("path")
            .short('p')
            .long("path")
            .help("Working directory")
            .value_parser(clap::value_parser!(PathBuf)))
}

#[cfg(not(doc))]
fn build_resume_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("resume")
        .about("Resume an interrupted workflow")
        .long_about("Resume an interrupted workflow from its last checkpoint, with automatic state restoration and progress tracking.")
        .arg(Arg::new("workflow_id")
            .help("Workflow ID to resume"))
        .arg(Arg::new("force")
            .long("force")
            .help("Force resume even if marked complete")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("from-checkpoint")
            .long("from-checkpoint")
            .help("Resume from specific checkpoint"))
        .arg(Arg::new("path")
            .short('p')
            .long("path")
            .help("Working directory")
            .value_parser(clap::value_parser!(PathBuf)))
}

#[cfg(not(doc))]
fn build_goal_seek_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("goal-seek")
        .alias("seek")
        .about("Execute goal-seeking operation with iterative refinement")
        .long_about("Iteratively refine results through multiple attempts until a validation threshold is met, with automatic convergence detection.")
        .arg(Arg::new("goal")
            .help("Goal description")
            .required(true))
        .arg(Arg::new("command")
            .short('c')
            .long("command")
            .help("Command to execute for attempts")
            .required(true))
        .arg(Arg::new("validate")
            .long("validate")
            .help("Validation command (outputs score: 0-100)")
            .required(true))
        .arg(Arg::new("threshold")
            .short('t')
            .long("threshold")
            .help("Success threshold (0-100)")
            .default_value("80"))
        .arg(Arg::new("max-attempts")
            .short('m')
            .long("max-attempts")
            .help("Maximum attempts")
            .default_value("5"))
        .arg(Arg::new("timeout")
            .long("timeout")
            .help("Overall timeout in seconds"))
        .arg(Arg::new("fail-on-incomplete")
            .long("fail-on-incomplete")
            .help("Exit with error if goal not achieved")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("path")
            .short('p')
            .long("path")
            .help("Working directory")
            .value_parser(clap::value_parser!(PathBuf)))
}

#[cfg(not(doc))]
fn build_worktree_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("worktree")
        .about("Manage git worktrees for parallel Prodigy sessions")
        .long_about("Manage isolated git worktrees for parallel execution, with automatic cleanup and merge capabilities.")
        .subcommands(vec![
            Command::new("ls")
                .alias("list")
                .about("List active Prodigy worktrees"),
            Command::new("merge")
                .about("Merge a worktree's changes to the default branch")
                .arg(Arg::new("name")
                    .help("Name of the worktree to merge"))
                .arg(Arg::new("all")
                    .long("all")
                    .help("Merge all Prodigy worktrees")
                    .action(clap::ArgAction::SetTrue)),
            Command::new("clean")
                .about("Clean up completed or abandoned worktrees")
                .arg(Arg::new("all")
                    .short('a')
                    .long("all")
                    .help("Clean up all Prodigy worktrees")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("name")
                    .help("Name of specific worktree to clean"))
                .arg(Arg::new("force")
                    .short('f')
                    .long("force")
                    .help("Force removal even with changes")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("merged-only")
                    .long("merged-only")
                    .help("Only clean merged sessions")
                    .action(clap::ArgAction::SetTrue)),
        ])
}

#[cfg(not(doc))]
fn build_init_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("init")
        .about("Initialize Prodigy commands in your project")
        .long_about("Initialize Prodigy commands in your project by creating the .claude/commands directory and installing command templates.")
        .arg(Arg::new("force")
            .short('f')
            .long("force")
            .help("Force overwrite existing commands")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("commands")
            .short('c')
            .long("commands")
            .help("Specific commands to install")
            .value_delimiter(','))
        .arg(Arg::new("path")
            .short('p')
            .long("path")
            .help("Directory to initialize")
            .value_parser(clap::value_parser!(PathBuf)))
}

#[cfg(not(doc))]
fn build_events_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("events")
        .about("View and search MapReduce events")
        .long_about("View, search, and analyze MapReduce execution events with filtering and real-time monitoring capabilities.")
        .subcommands(vec![
            Command::new("ls")
                .alias("list")
                .about("List all events")
                .arg(Arg::new("job-id").long("job-id").help("Filter by job ID"))
                .arg(Arg::new("event-type").long("event-type").help("Filter by event type"))
                .arg(Arg::new("agent-id").long("agent-id").help("Filter by agent ID"))
                .arg(Arg::new("since").long("since").help("Show events from last N minutes"))
                .arg(Arg::new("limit").long("limit").help("Limit number of events").default_value("100")),
            Command::new("stats")
                .about("Show event statistics")
                .arg(Arg::new("group-by").long("group-by").help("Group by field").default_value("event_type")),
            Command::new("search")
                .about("Search events by pattern")
                .arg(Arg::new("pattern").help("Search pattern").required(true)),
            Command::new("follow")
                .about("Follow events in real-time")
                .arg(Arg::new("job-id").long("job-id").help("Filter by job ID")),
            Command::new("clean")
                .about("Clean old events")
                .arg(Arg::new("older-than").long("older-than").help("Delete events older than"))
                .arg(Arg::new("dry-run").long("dry-run").help("Preview without deleting").action(clap::ArgAction::SetTrue)),
            Command::new("export")
                .about("Export events to different format")
                .arg(Arg::new("format").long("format").help("Output format").default_value("json")),
        ])
}

#[cfg(not(doc))]
fn build_dlq_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("dlq")
        .about("Manage Dead Letter Queue for failed items")
        .long_about(
            "Manage and reprocess failed MapReduce work items stored in the Dead Letter Queue.",
        )
        .subcommands(vec![
            Command::new("list")
                .about("List items in the Dead Letter Queue")
                .arg(Arg::new("job-id").long("job-id").help("Filter by job ID")),
            Command::new("inspect")
                .about("Inspect a specific DLQ item")
                .arg(
                    Arg::new("item_id")
                        .help("Item ID to inspect")
                        .required(true),
                ),
            Command::new("analyze")
                .about("Analyze failure patterns")
                .arg(
                    Arg::new("export")
                        .long("export")
                        .help("Export analysis to file")
                        .value_parser(clap::value_parser!(PathBuf)),
                ),
            Command::new("purge")
                .about("Purge old items from the DLQ")
                .arg(
                    Arg::new("older-than-days")
                        .long("older-than-days")
                        .help("Delete items older than N days")
                        .required(true),
                ),
        ])
}

#[cfg(not(doc))]
fn build_sessions_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("sessions")
        .about("Manage cooking sessions")
        .long_about("View and manage Prodigy cooking sessions with state tracking and cleanup capabilities.")
        .subcommands(vec![
            Command::new("ls")
                .alias("list")
                .about("List resumable sessions"),
            Command::new("show")
                .about("Show details about a specific session")
                .arg(Arg::new("session_id").help("Session ID to show").required(true)),
            Command::new("clean")
                .about("Clean up old sessions")
                .arg(Arg::new("all").long("all").help("Clean all sessions").action(clap::ArgAction::SetTrue))
                .arg(Arg::new("force").short('f').long("force").help("Force cleanup without confirmation").action(clap::ArgAction::SetTrue)),
        ])
}

#[cfg(not(doc))]
fn build_analytics_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("analytics")
        .about("Analyze Claude session analytics")
        .long_about("Extract and analyze Claude session data including costs, token usage, tool invocations, and performance metrics.")
        .subcommands(vec![
            Command::new("watch")
                .about("Watch JSONL files for session data"),
            Command::new("report")
                .about("Generate analytics report")
                .arg(Arg::new("session_id").help("Session ID to analyze").required(true)),
            Command::new("cost")
                .about("Calculate session costs")
                .arg(Arg::new("session_id").help("Session ID (optional)")),
            Command::new("replay")
                .about("Replay session interactions")
                .arg(Arg::new("session_id").help("Session ID to replay").required(true)),
        ])
}

#[cfg(not(doc))]
fn build_checkpoints_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("checkpoints")
        .about("List available workflow checkpoints")
        .long_about("Manage workflow checkpoints for resuming interrupted executions with full state restoration.")
        .subcommands(vec![
            Command::new("list")
                .alias("ls")
                .about("List all available checkpoints")
                .arg(Arg::new("workflow-id").long("workflow-id").help("Filter by workflow ID"))
                .arg(Arg::new("verbose").short('v').long("verbose").help("Show verbose details").action(clap::ArgAction::SetTrue)),
            Command::new("clean")
                .about("Delete checkpoints for completed workflows")
                .arg(Arg::new("all").long("all").help("Clean all completed checkpoints").action(clap::ArgAction::SetTrue))
                .arg(Arg::new("force").short('f').long("force").help("Force deletion without confirmation").action(clap::ArgAction::SetTrue)),
            Command::new("show")
                .about("Show detailed checkpoint information")
                .arg(Arg::new("workflow_id").help("Workflow ID").required(true))
                .arg(Arg::new("version").long("version").help("Checkpoint version")),
        ])
}

#[cfg(not(doc))]
fn build_progress_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("progress")
        .about("View MapReduce job progress")
        .long_about("Monitor MapReduce job execution progress with real-time updates and export capabilities.")
        .arg(Arg::new("job_id").help("Job ID to view progress for").required(true))
        .arg(Arg::new("export").long("export").help("Export progress data").value_parser(clap::value_parser!(PathBuf)))
        .arg(Arg::new("format").long("format").help("Export format").default_value("json"))
        .arg(Arg::new("web").long("web").help("Start web dashboard on port"))
}

#[cfg(not(doc))]
fn build_resume_job_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("resume-job")
        .about("Resume a MapReduce job from checkpoint")
        .long_about("Resume a MapReduce job from its checkpoint with automatic recovery of failed items and progress tracking.")
        .arg(Arg::new("job_id").help("Job ID to resume").required(true))
        .arg(Arg::new("force").long("force").help("Force resume even if complete").action(clap::ArgAction::SetTrue))
        .arg(Arg::new("max-retries").long("max-retries").help("Maximum additional retries").default_value("2"))
        .arg(Arg::new("path").short('p').long("path").help("Repository path").value_parser(clap::value_parser!(PathBuf)))
}

#[cfg(not(doc))]
fn build_validate_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("validate")
        .about("Validate workflow YAML format")
        .long_about(
            "Validate workflow YAML syntax and structure with suggestions for improvements.",
        )
        .arg(
            Arg::new("workflow")
                .help("Workflow file to validate")
                .required(true)
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("format")
                .long("format")
                .help("Check for format")
                .default_value("simplified"),
        )
        .arg(Arg::new("suggest").long("suggest").help("Show suggestions"))
        .arg(
            Arg::new("strict")
                .long("strict")
                .help("Exit with error if not valid")
                .action(clap::ArgAction::SetTrue),
        )
}

#[cfg(not(doc))]
fn build_migrate_yaml_subcommand() -> clap::Command {
    use clap::{Arg, Command};

    Command::new("migrate-yaml")
        .about("Migrate workflow YAML to simplified syntax")
        .long_about("Automatically migrate workflow YAML files from legacy format to the simplified syntax.")
        .arg(Arg::new("path").help("File or directory to migrate").value_parser(clap::value_parser!(PathBuf)))
        .arg(Arg::new("backup").long("backup").help("Create backup files"))
        .arg(Arg::new("dry-run").long("dry-run").help("Show changes without modifying").action(clap::ArgAction::SetTrue))
        .arg(Arg::new("force").short('f').long("force").help("Force overwrite without backup").action(clap::ArgAction::SetTrue))
}

fn extract_description(name: &str) -> &'static str {
    // Extract a description from the command name
    match name {
        "prodigy-code-review" => "Analyzes code quality and creates improvement specs",
        "prodigy-implement-spec" => "Implements Git Good specifications",
        "prodigy-lint" => "Runs formatters, linters, and tests",
        "prodigy-product-enhance" => "Product-focused improvements for user value",
        "prodigy-merge-worktree" => "Claude-assisted worktree merging with conflict resolution",
        "prodigy-cleanup-tech-debt" => {
            "Analyzes technical debt and generates cleanup specifications"
        }
        "prodigy-coverage" => "Analyzes and improves test coverage",
        "prodigy-commit-changes" => "Commits changes with detailed commit messages",
        "prodigy-debug-test-failure" => "Debugs and fixes test failures",
        "prodigy-docs-generate" => "Generates documentation for the codebase",
        "prodigy-performance" => "Analyzes and optimizes performance",
        "prodigy-security-audit" => "Performs security analysis and auditing",
        "prodigy-test-generate" => "Generates test cases for code",
        "prodigy-add-spec" => "Adds new specifications to the project",
        "prodigy-compare-debt-results" => "Compares technical debt analysis results",
        _ => "Claude command for Prodigy automation",
    }
}
