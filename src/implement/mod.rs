pub mod command;
pub mod state;

use anyhow::{anyhow, Context, Result};
use glob::glob;
use std::path::PathBuf;
use std::time::Instant;
use tokio::process::Command as TokioCommand;

use crate::improve::git_ops;
use crate::worktree::WorktreeManager;
use state::BatchImplementState;

/// Run the batch implementation process
pub async fn run(cmd: command::ImplementCommand) -> Result<()> {
    println!("🚀 Starting batch specification implementation");

    // Resolve specification files
    let spec_files = resolve_spec_files(&cmd.spec_files)?;

    if spec_files.is_empty() {
        return Err(anyhow!(
            "No specification files found matching the provided patterns"
        ));
    }

    println!(
        "📁 Found {} specification file(s) to implement",
        spec_files.len()
    );

    // Initialize batch state
    let mut state = BatchImplementState::new(spec_files.clone(), cmd.dry_run);

    // Show what will be implemented
    if cmd.dry_run {
        println!("\n🔍 DRY RUN - The following specifications would be implemented:");
        for (i, spec) in spec_files.iter().enumerate() {
            println!("  [{}] {}", i + 1, spec.display());
        }
        println!("\nNo changes will be made.");
        return Ok(());
    }

    // Handle worktree mode if requested
    let worktree_manager = if cmd.worktree {
        let current_dir = std::env::current_dir()?;
        let manager = WorktreeManager::new(current_dir)?;

        // Create a worktree for batch implementation
        let session = manager.create_session(Some("batch-implement"))?;
        println!("📂 Created worktree: {}", session.name);

        // Change to worktree directory
        std::env::set_current_dir(&session.path)?;

        Some((manager, session.name))
    } else {
        None
    };

    // Implement each specification
    for (index, spec_path) in spec_files.iter().enumerate() {
        let spec_start = Instant::now();
        let spec_id = extract_spec_id(&spec_path)?;

        println!(
            "\n[{}/{}] 📝 Implementing specification: {}",
            index + 1,
            state.total_specs(),
            spec_id
        );

        state.current_spec = Some(spec_id.clone());

        // Run implement-spec → lint cycle
        let success = match implement_spec(&spec_id, &cmd).await {
            Ok(_) => {
                println!("✅ Successfully implemented: {}", spec_id);
                true
            }
            Err(e) => {
                eprintln!("❌ Failed to implement {}: {}", spec_id, e);
                if cmd.fail_fast {
                    return Err(anyhow!("Stopping due to --fail-fast flag"));
                }
                false
            }
        };

        state.complete_current(success, spec_start.elapsed());

        // Show progress
        println!(
            "Progress: {:.1}% complete ({}/{} specs)",
            state.progress_percentage(),
            state.completed_count(),
            state.total_specs()
        );
    }

    // Generate and show summary
    let summary = state.generate_summary();
    println!("{}", summary);

    // Handle worktree cleanup/merge if used
    if let Some((manager, session_name)) = worktree_manager {
        if state.failure_count() == 0 {
            println!("\n✅ All specifications implemented successfully!");
            println!("Would you like to merge the changes? (y/N)");

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            if input.trim().to_lowercase() == "y" {
                // Change back to original directory before merge
                if let Ok(orig_dir) = manager.repo_path.canonicalize() {
                    std::env::set_current_dir(&orig_dir)?;
                }

                println!("📝 Merging worktree changes...");
                manager.merge_session(&session_name)?;
                manager.cleanup_session(&session_name)?;
                println!("✅ Changes merged and worktree cleaned up");
            } else {
                println!("ℹ️ Worktree preserved at: {}", session_name);
                println!(
                    "   You can merge it later with: mmm worktree merge {}",
                    session_name
                );
            }
        } else {
            println!("\n⚠️ Some specifications failed. Worktree preserved for debugging.");
            println!("   Worktree: {}", session_name);
            println!(
                "   You can merge successful changes with: mmm worktree merge {}",
                session_name
            );
        }
    }

    Ok(())
}

/// Resolve specification file paths from patterns
fn resolve_spec_files(patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut resolved_files = Vec::new();

    for pattern in patterns {
        // First check if it's a direct file path
        let path = PathBuf::from(pattern);
        if path.exists() && path.is_file() {
            resolved_files.push(path);
        } else {
            // Try as a glob pattern
            for entry in glob(pattern).context("Failed to parse glob pattern")? {
                match entry {
                    Ok(path) if path.is_file() => {
                        resolved_files.push(path);
                    }
                    Ok(_) => {} // Skip directories
                    Err(e) => eprintln!("Warning: Error processing glob entry: {}", e),
                }
            }
        }
    }

    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    resolved_files.retain(|path| seen.insert(path.clone()));

    Ok(resolved_files)
}

/// Extract spec ID from a specification file path
fn extract_spec_id(spec_path: &PathBuf) -> Result<String> {
    // Try to extract from filename first
    if let Some(file_stem) = spec_path.file_stem() {
        let filename = file_stem.to_string_lossy();

        // Handle patterns like "33-batch-spec-implementation" or "iteration-1234567890-improvements"
        if filename.contains('-') {
            return Ok(filename.to_string());
        }

        // For simple numeric specs like "33.md"
        if filename.chars().all(|c| c.is_numeric()) {
            return Ok(filename.to_string());
        }
    }

    // If we can't extract from filename, use the full filename without extension
    spec_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or_else(|| {
            anyhow!(
                "Could not extract spec ID from path: {}",
                spec_path.display()
            )
        })
}

/// Implement a single specification using implement-spec → lint cycle
async fn implement_spec(spec_id: &str, cmd: &command::ImplementCommand) -> Result<()> {
    println!("🔧 Running /mmm-implement-spec {}...", spec_id);

    // Set automation environment variable
    std::env::set_var("MMM_AUTOMATION", "true");

    // Call /mmm-implement-spec
    let implement_output = TokioCommand::new("claude")
        .args(&["/mmm-implement-spec", spec_id])
        .env("MMM_AUTOMATION", "true")
        .output()
        .await
        .context("Failed to execute claude /mmm-implement-spec")?;

    if !implement_output.status.success() {
        let stderr = String::from_utf8_lossy(&implement_output.stderr);
        return Err(anyhow!("mmm-implement-spec failed: {}", stderr));
    }

    if cmd.verbose {
        let stdout = String::from_utf8_lossy(&implement_output.stdout);
        println!("Implementation output:\n{}", stdout);
    }

    // Check if there were any changes made
    let git_status = git_ops::check_git_status().await.unwrap_or_default();

    if git_status.trim().is_empty() {
        println!("ℹ️ No changes made by implementation");
        return Ok(());
    }

    // Run /mmm-lint
    println!("🧹 Running /mmm-lint...");

    let lint_output = TokioCommand::new("claude")
        .args(&["/mmm-lint"])
        .env("MMM_AUTOMATION", "true")
        .output()
        .await
        .context("Failed to execute claude /mmm-lint")?;

    if !lint_output.status.success() {
        let stderr = String::from_utf8_lossy(&lint_output.stderr);
        eprintln!("⚠️ Lint failed (non-fatal): {}", stderr);
        // Don't fail the whole spec implementation if lint fails
    } else if cmd.verbose {
        let stdout = String::from_utf8_lossy(&lint_output.stdout);
        println!("Lint output:\n{}", stdout);
    }

    Ok(())
}
