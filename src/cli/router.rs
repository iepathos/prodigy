//! Command routing and execution
//!
//! This module handles routing CLI commands to their respective implementations.

use crate::cli::args::Commands;
use crate::cli::commands::*;
use anyhow::Result;
use std::path::PathBuf;

/// Execute a CLI command based on the parsed arguments
pub async fn execute_command(command: Option<Commands>, verbose: u8) -> Result<()> {
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
            let cook_cmd = crate::cook::command::CookCommand {
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
            crate::cook::cook(cook_cmd).await
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
        Some(Commands::Checkpoints { command }) => run_checkpoints_command(command, verbose).await,
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
            let init_cmd = crate::init::command::InitCommand {
                force,
                commands,
                path,
            };
            crate::init::run(init_cmd).await
        }
        Some(Commands::MigrateYaml {
            path,
            backup,
            dry_run,
            force: _,
        }) => {
            use crate::cli::yaml_migrator::YamlMigrator;
            let migrator = YamlMigrator::new(backup);
            let target_path = path.unwrap_or_else(|| PathBuf::from("workflows"));

            let results = if target_path.is_file() {
                vec![migrator.migrate_file(&target_path, dry_run)?]
            } else {
                migrator.migrate_directory(&target_path, dry_run)?
            };

            // Print results
            for result in results {
                if result.was_migrated {
                    println!("✓ Migrated: {}", result.file.display());
                } else if let Some(error) = result.error {
                    eprintln!("✗ Failed: {} - {}", result.file.display(), error);
                }
            }

            Ok(())
        }
        Some(Commands::Validate {
            workflow,
            format: _,
            suggest: _,
            strict,
        }) => {
            use crate::cli::yaml_validator::YamlValidator;
            let validator = YamlValidator::new(strict);
            let result = validator.validate_file(&workflow)?;

            // Print validation results
            if result.is_valid {
                println!("✓ Workflow is valid: {}", workflow.display());
            } else {
                eprintln!("✗ Workflow has issues:");
                for issue in &result.issues {
                    eprintln!("  - {}", issue);
                }
            }

            if !result.suggestions.is_empty() {
                println!("\nSuggestions:");
                for suggestion in &result.suggestions {
                    println!("  - {}", suggestion);
                }
            }

            if !result.is_valid {
                std::process::exit(1);
            }
            Ok(())
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
            // No command provided, show help
            use crate::cli::help::generate_help;
            println!("{}", generate_help());
            Ok(())
        }
    }
}
