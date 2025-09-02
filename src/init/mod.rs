//! MMM project initialization for Claude Code integration
//!
//! This module provides functionality to initialize MMM commands in a project,
//! setting up the necessary directory structure and Claude Code command files
//! that enable automated code improvement workflows.
//!
//! # Key Features
//!
//! - **Git Repository Validation**: Ensures MMM is only installed in git repositories
//! - **Template Management**: Provides pre-built command templates for common workflows
//! - **Smart Installation**: Handles existing commands and provides options for selective installation
//! - **Interactive Mode**: Prompts user for confirmation when conflicts exist
//! - **Force Mode**: Allows overwriting existing commands when needed
//!
//! # Command Templates
//!
//! The init system provides several built-in command templates:
//! - `prodigy-code-review` - Automated code quality analysis and review
//! - `prodigy-implement-spec` - Implementation of specifications and features
//! - `prodigy-lint` - Code linting and style enforcement
//! - `prodigy-product-enhance` - Product-focused enhancements
//! - `prodigy-merge-worktree` - Git worktree management
//! - `prodigy-cleanup-tech-debt` - Technical debt reduction
//!
//! # Examples
//!
//! ## Initialize All Commands
//!
//! ```rust
//! use prodigy::init::{run, command::InitCommand};
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let cmd = InitCommand {
//!     force: false,
//!     commands: None, // Install all commands
//!     path: Some(PathBuf::from("/path/to/project")),
//! };
//!
//! run(cmd).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Install Specific Commands
//!
//! ```rust
//! # use prodigy::init::{run, command::InitCommand};
//! # use std::path::PathBuf;
//! # async fn example() -> anyhow::Result<()> {
//! let cmd = InitCommand {
//!     force: false,
//!     commands: Some(vec![
//!         "prodigy-code-review".to_string(),
//!         "prodigy-lint".to_string()
//!     ]),
//!     path: None, // Use current directory
//! };
//!
//! run(cmd).await?;
//! # Ok(())
//! # }
//! ```

pub mod command;
pub mod templates;

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::init::command::InitCommand;
use crate::subprocess::SubprocessManager;

/// Check if the current directory is a git repository
async fn is_git_repository(path: &Path, subprocess: &SubprocessManager) -> bool {
    // Check for .git directory or file (in case of git worktree)
    if path.join(".git").exists() {
        return true;
    }

    // Also check using git command to handle edge cases
    use crate::subprocess::ProcessCommandBuilder;

    #[cfg(test)]
    let command = ProcessCommandBuilder::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .suppress_stderr()
        .build();

    #[cfg(not(test))]
    let command = ProcessCommandBuilder::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .build();

    subprocess
        .runner()
        .run(command)
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Select templates based on command configuration
fn select_templates(cmd: &InitCommand) -> Result<Vec<templates::CommandTemplate>> {
    if let Some(ref command_names) = cmd.commands {
        if command_names.is_empty() {
            println!("⚠️  No commands specified. Use --commands or omit to install all.");
            return Ok(vec![]);
        }
        let selected = templates::get_templates_by_names(command_names);
        if selected.is_empty() {
            println!("❌ No matching commands found for: {command_names:?}");
            println!("   Available commands: prodigy-code-review, prodigy-implement-spec, prodigy-lint, prodigy-product-enhance, prodigy-merge-worktree, prodigy-cleanup-tech-debt");
            return Ok(vec![]);
        }
        Ok(selected)
    } else {
        Ok(templates::get_all_templates())
    }
}

/// Check if a command file exists
fn command_exists(commands_dir: &Path, template_name: &str) -> bool {
    commands_dir.join(format!("{}.md", template_name)).exists()
}

/// Find all existing command templates
fn find_existing_commands<'a>(
    commands_dir: &Path,
    templates: &'a [templates::CommandTemplate],
) -> Vec<&'a str> {
    templates
        .iter()
        .filter(|t| command_exists(commands_dir, t.name))
        .map(|t| t.name)
        .collect()
}

/// Check if running in test environment
fn is_test_environment() -> bool {
    std::env::var("CARGO_TARGET_TMPDIR").is_ok()
        || std::env::var("RUST_TEST_THREADS").is_ok()
        || cfg!(test)
}

/// Display warning about existing commands
fn display_existing_commands_warning(existing: &[&str]) {
    println!("\n⚠️  The following commands already exist:");
    for name in existing {
        println!("   - {name}");
    }
    println!(
        "\nUse --force to overwrite existing commands, or --commands to select specific ones."
    );
    println!("Example: prodigy init --commands prodigy-lint,prodigy-product-enhance");
}

/// Get user confirmation in interactive mode
fn get_user_confirmation() -> Result<bool> {
    use std::io::{self, IsTerminal, Write};

    if std::io::stdin().is_terminal() && !is_test_environment() {
        print!("\nDo you want to continue and skip existing commands? (y/N): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() != "y" {
            println!("❌ Installation cancelled.");
            return Ok(false);
        }
    } else {
        // Non-interactive mode - skip existing by default
        println!("ℹ️  Skipping existing commands (non-interactive mode).");
    }

    Ok(true)
}

/// Check if templates list requires processing
fn should_process_templates(templates: &[templates::CommandTemplate]) -> bool {
    !templates.is_empty()
}

/// Check if existing commands need user confirmation
fn needs_user_confirmation(existing_commands: &[&str]) -> bool {
    !existing_commands.is_empty()
}

/// Process existing commands check and confirmation flow
fn process_existing_commands_check(
    existing: Vec<&str>,
) -> Result<bool> {
    if !needs_user_confirmation(&existing) {
        return Ok(true);
    }
    
    display_existing_commands_warning(&existing);
    get_user_confirmation()
}

/// Handle checking for existing commands and get user confirmation
fn handle_existing_commands(
    commands_dir: &Path,
    templates: &[templates::CommandTemplate],
) -> Result<bool> {
    if !should_process_templates(templates) {
        return Ok(true);
    }

    let existing = find_existing_commands(commands_dir, templates);
    process_existing_commands_check(existing)
}

/// Install all selected templates
fn install_templates(
    commands_dir: &Path,
    templates: &[templates::CommandTemplate],
    force: bool,
) -> Result<(usize, usize)> {
    println!("\n📦 Installing {} command(s)...", templates.len());
    let mut installed = 0;
    let mut skipped = 0;

    for template in templates {
        match install_command(commands_dir, template, force) {
            Ok(_) => installed += 1,
            Err(e) => {
                eprintln!("❌ Failed to install '{}': {}", template.name, e);
                skipped += 1;
            }
        }
    }

    Ok((installed, skipped))
}

/// Display installation summary and next steps
fn display_installation_summary(installed: usize, skipped: usize, commands_dir: &Path) {
    println!("\n✨ Installation complete!");
    println!("   - {installed} command(s) installed");
    if skipped > 0 {
        println!("   - {skipped} command(s) skipped");
    }

    // Show next steps
    if installed > 0 {
        println!("\n📚 Next steps:");
        println!(
            "   1. Review installed commands in: {}",
            commands_dir.display()
        );
        println!("   2. Customize commands as needed for your project");
        println!("   3. Run 'prodigy cook' to start improving your code");
        println!("\n💡 Tip: You can always reinstall default commands with 'prodigy init --force'");
    }
}

/// Install a single command template
fn install_command(
    commands_dir: &Path,
    template: &templates::CommandTemplate,
    force: bool,
) -> Result<()> {
    let file_path = commands_dir.join(format!("{}.md", template.name));

    if file_path.exists() && !force {
        println!(
            "⚠️  Command '{}' already exists. Use --force to overwrite.",
            template.name
        );
        return Ok(());
    }

    fs::write(&file_path, template.content)
        .with_context(|| format!("Failed to write command file: {}", file_path.display()))?;

    println!("✅ Installed command: {}", template.name);
    Ok(())
}

/// Validate project structure and prepare target directory
async fn validate_project_structure(
    cmd: &InitCommand,
    subprocess: &SubprocessManager,
) -> Result<(PathBuf, PathBuf)> {
    // Determine the target directory
    let target_dir = cmd.path.clone().unwrap_or_else(|| PathBuf::from("."));
    let target_dir = target_dir
        .canonicalize()
        .with_context(|| format!("Failed to resolve path: {}", target_dir.display()))?;

    println!("🚀 Initializing MMM commands in: {}", target_dir.display());

    // Check if it's a git repository
    if !is_git_repository(&target_dir, subprocess).await {
        anyhow::bail!(
            "Error: {} is not a git repository.\n\
             MMM commands must be installed in a git repository.\n\
             Run 'git init' first or navigate to a git repository.",
            target_dir.display()
        );
    }

    let commands_dir = initialize_directories(&target_dir)?;
    Ok((target_dir, commands_dir))
}

/// Initialize .claude/commands directory structure
fn initialize_directories(target_dir: &Path) -> Result<PathBuf> {
    let claude_dir = target_dir.join(".claude");
    let commands_dir = claude_dir.join("commands");

    if !commands_dir.exists() {
        fs::create_dir_all(&commands_dir)
            .with_context(|| format!("Failed to create directory: {}", commands_dir.display()))?;
        println!("📁 Created directory: {}", commands_dir.display());
    }

    Ok(commands_dir)
}

/// Run the init command
pub async fn run(cmd: InitCommand) -> Result<()> {
    let subprocess = SubprocessManager::production();
    let (_target_dir, commands_dir) = validate_project_structure(&cmd, &subprocess).await?;

    // Get the templates to install
    let templates = select_templates(&cmd)?;

    // Check for existing commands if not forcing
    if !cmd.force && !handle_existing_commands(&commands_dir, &templates)? {
        return Ok(());
    }

    // Install the commands
    let (installed, skipped) = install_templates(&commands_dir, &templates, cmd.force)?;

    // Show summary and next steps
    display_installation_summary(installed, skipped, &commands_dir);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init::command::InitCommand;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_is_git_repository() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let subprocess = SubprocessManager::production();

        // Should not be a git repo initially
        assert!(!is_git_repository(path, &subprocess).await);

        // Initialize git repo
        use crate::subprocess::ProcessCommandBuilder;
        subprocess
            .runner()
            .run(
                ProcessCommandBuilder::new("git")
                    .arg("init")
                    .current_dir(path)
                    .build(),
            )
            .await
            .unwrap();

        // Should now be a git repo
        assert!(is_git_repository(path, &subprocess).await);
    }

    #[test]
    fn test_get_templates() {
        let all_templates = templates::get_all_templates();
        assert_eq!(all_templates.len(), 6);

        // Test filtering by names
        let names = vec![
            "prodigy-lint".to_string(),
            "prodigy-code-review".to_string(),
        ];
        let filtered = templates::get_templates_by_names(&names);
        assert_eq!(filtered.len(), 2);
    }

    #[tokio::test]
    async fn test_run_init_not_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let cmd = InitCommand {
            force: false,
            commands: None,
            path: Some(temp_dir.path().to_path_buf()),
        };

        let result = run(cmd).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not a git repository"));
    }

    #[tokio::test]
    async fn test_run_init_create_commands() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        use crate::subprocess::ProcessCommandBuilder;
        let subprocess = SubprocessManager::production();
        subprocess
            .runner()
            .run(
                ProcessCommandBuilder::new("git")
                    .arg("init")
                    .current_dir(temp_dir.path())
                    .build(),
            )
            .await
            .unwrap();

        let cmd = InitCommand {
            force: false,
            commands: None,
            path: Some(temp_dir.path().to_path_buf()),
        };

        let result = run(cmd).await;
        assert!(result.is_ok());

        // Check commands were created
        let commands_dir = temp_dir.path().join(".claude").join("commands");
        assert!(commands_dir.exists());
        assert!(commands_dir.join("prodigy-code-review.md").exists());
        assert!(commands_dir.join("prodigy-lint.md").exists());
    }

    #[test]
    fn test_should_process_templates() {
        // Test empty templates
        let templates: Vec<templates::CommandTemplate> = vec![];
        assert!(!should_process_templates(&templates));
        
        // Test with templates
        let templates = templates::get_all_templates();
        assert!(should_process_templates(&templates));
    }
    
    #[test]
    fn test_needs_user_confirmation() {
        // Test empty existing commands
        let existing: Vec<&str> = vec![];
        assert!(!needs_user_confirmation(&existing));
        
        // Test with existing commands
        let existing = vec!["prodigy-lint", "prodigy-code-review"];
        assert!(needs_user_confirmation(&existing));
    }
    
    #[test]
    fn test_process_existing_commands_check_no_existing() {
        // When no existing commands, should return Ok(true)
        let existing: Vec<&str> = vec![];
        let result = process_existing_commands_check(existing);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
    
    #[test]
    fn test_process_existing_commands_check_with_existing() {
        // When existing commands present, it will display warning and attempt to get confirmation
        // In test environment, it should handle non-interactive mode
        let existing = vec!["prodigy-lint", "prodigy-code-review"];
        let result = process_existing_commands_check(existing);
        assert!(result.is_ok());
        // In non-interactive mode (test), it returns true (skip existing)
        assert!(result.unwrap());
    }
    
    #[test]
    fn test_refactored_handle_existing_commands_empty() {
        let temp_dir = TempDir::new().unwrap();
        let templates: Vec<templates::CommandTemplate> = vec![];
        let result = handle_existing_commands(temp_dir.path(), &templates);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
    
    #[test]
    fn test_refactored_handle_existing_commands_no_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let templates = templates::get_all_templates();
        // No commands exist yet, so no conflicts
        let result = handle_existing_commands(temp_dir.path(), &templates);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
    
    #[tokio::test]
    async fn test_run_init_with_existing_commands() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        use crate::subprocess::ProcessCommandBuilder;
        let subprocess = SubprocessManager::production();
        subprocess
            .runner()
            .run(
                ProcessCommandBuilder::new("git")
                    .arg("init")
                    .current_dir(temp_dir.path())
                    .build(),
            )
            .await
            .unwrap();

        // Create existing command
        let commands_dir = temp_dir.path().join(".claude").join("commands");
        std::fs::create_dir_all(&commands_dir).unwrap();
        std::fs::write(
            commands_dir.join("prodigy-code-review.md"),
            "existing content",
        )
        .unwrap();

        let cmd = InitCommand {
            force: false,
            commands: None,
            path: Some(temp_dir.path().to_path_buf()),
        };

        // Should skip existing commands
        let result = run(cmd).await;
        assert!(result.is_ok());

        // Check existing file wasn't overwritten
        let content = std::fs::read_to_string(commands_dir.join("prodigy-code-review.md")).unwrap();
        assert_eq!(content, "existing content");
    }

    #[tokio::test]
    async fn test_run_init_force_overwrite() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        use crate::subprocess::ProcessCommandBuilder;
        let subprocess = SubprocessManager::production();
        subprocess
            .runner()
            .run(
                ProcessCommandBuilder::new("git")
                    .arg("init")
                    .current_dir(temp_dir.path())
                    .build(),
            )
            .await
            .unwrap();

        // Create existing command
        let commands_dir = temp_dir.path().join(".claude").join("commands");
        std::fs::create_dir_all(&commands_dir).unwrap();
        std::fs::write(commands_dir.join("prodigy-code-review.md"), "old content").unwrap();

        let cmd = InitCommand {
            force: true,
            commands: None,
            path: Some(temp_dir.path().to_path_buf()),
        };

        let result = run(cmd).await;
        assert!(result.is_ok());

        // Check file was overwritten
        let content = std::fs::read_to_string(commands_dir.join("prodigy-code-review.md")).unwrap();
        assert!(content.contains("Analyze code"));
        assert!(!content.contains("old content"));
    }

    #[tokio::test]
    async fn test_run_init_specific_commands() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        use crate::subprocess::ProcessCommandBuilder;
        let subprocess = SubprocessManager::production();
        subprocess
            .runner()
            .run(
                ProcessCommandBuilder::new("git")
                    .arg("init")
                    .current_dir(temp_dir.path())
                    .build(),
            )
            .await
            .unwrap();

        let cmd = InitCommand {
            force: false,
            commands: Some(vec![
                "prodigy-code-review".to_string(),
                "prodigy-lint".to_string(),
            ]),
            path: Some(temp_dir.path().to_path_buf()),
        };

        let result = run(cmd).await;
        assert!(result.is_ok());

        let commands_dir = temp_dir.path().join(".claude").join("commands");

        // Should only install specified commands
        assert!(commands_dir.join("prodigy-code-review.md").exists());
        assert!(commands_dir.join("prodigy-lint.md").exists());
        assert!(!commands_dir.join("prodigy-implement-spec.md").exists());
    }

    #[test]
    fn test_command_exists() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        // Create an existing command file
        fs::write(commands_dir.join("existing.md"), "content").unwrap();

        assert!(command_exists(&commands_dir, "existing"));
        assert!(!command_exists(&commands_dir, "non-existing"));
    }

    #[test]
    fn test_find_existing_commands() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        // Create some existing command files
        fs::write(commands_dir.join("command1.md"), "content").unwrap();
        fs::write(commands_dir.join("command3.md"), "content").unwrap();

        let templates = vec![
            templates::CommandTemplate {
                name: "command1",
                content: "content1",
                description: "Command 1",
            },
            templates::CommandTemplate {
                name: "command2",
                content: "content2",
                description: "Command 2",
            },
            templates::CommandTemplate {
                name: "command3",
                content: "content3",
                description: "Command 3",
            },
        ];

        let existing = find_existing_commands(&commands_dir, &templates);
        assert_eq!(existing.len(), 2);
        assert!(existing.contains(&"command1"));
        assert!(existing.contains(&"command3"));
        assert!(!existing.contains(&"command2"));
    }

    #[test]
    fn test_find_existing_commands_empty() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        let templates = vec![templates::CommandTemplate {
            name: "command1",
            content: "content1",
            description: "Command 1",
        }];

        let existing = find_existing_commands(&commands_dir, &templates);
        assert_eq!(existing.len(), 0);
    }

    #[test]
    fn test_is_test_environment() {
        // This test runs in a test environment, so it should return true
        assert!(is_test_environment());
    }

    #[test]
    fn test_handle_existing_commands_no_tty() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        let templates = vec![templates::CommandTemplate {
            name: "test-command",
            content: "#!/bin/bash\necho test",
            description: "Test command",
        }];

        // Should return Ok(true) when no TTY is available
        let result = handle_existing_commands(&commands_dir, &templates).unwrap();
        assert!(result);
    }

    #[test]
    fn test_handle_existing_commands_empty_templates() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        let templates = vec![];

        // Should return Ok(true) for empty templates
        let result = handle_existing_commands(&commands_dir, &templates).unwrap();
        assert!(result);
    }

    #[test]
    fn test_handle_existing_commands_no_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        let templates = vec![templates::CommandTemplate {
            name: "new-command",
            content: "content",
            description: "New command",
        }];

        // Should return Ok(true) when no conflicts
        let result = handle_existing_commands(&commands_dir, &templates).unwrap();
        assert!(result);
    }

    #[test]
    fn test_handle_existing_commands_with_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        // Create existing command
        fs::write(commands_dir.join("test-command.md"), "existing content").unwrap();

        let templates = vec![templates::CommandTemplate {
            name: "test-command",
            content: "new content",
            description: "Test command",
        }];

        // Should handle conflicts appropriately
        let result = handle_existing_commands(&commands_dir, &templates);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_project_structure_not_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let cmd = InitCommand {
            path: Some(temp_dir.path().to_path_buf()),
            commands: None,
            force: false,
        };
        let subprocess = SubprocessManager::production();

        let result = validate_project_structure(&cmd, &subprocess).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("git repository"));
    }

    #[tokio::test]
    async fn test_validate_project_structure_with_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let real_path = temp_dir.path().join("real");
        let symlink_path = temp_dir.path().join("symlink");

        fs::create_dir_all(&real_path).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_path, &symlink_path).unwrap();

        // Initialize as git repo
        use crate::subprocess::ProcessCommandBuilder;
        let subprocess = SubprocessManager::production();
        subprocess
            .runner()
            .run(
                ProcessCommandBuilder::new("git")
                    .arg("init")
                    .current_dir(&real_path)
                    .build(),
            )
            .await
            .unwrap();

        let cmd = InitCommand {
            path: Some(symlink_path),
            commands: None,
            force: false,
        };

        #[cfg(unix)]
        {
            let result = validate_project_structure(&cmd, &subprocess).await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_init_run_success() {
        // Test normal operation
        let temp_dir = TempDir::new().unwrap();

        // Initialize as git repo
        use crate::subprocess::ProcessCommandBuilder;
        let subprocess = SubprocessManager::production();
        subprocess
            .runner()
            .run(
                ProcessCommandBuilder::new("git")
                    .arg("init")
                    .current_dir(temp_dir.path())
                    .build(),
            )
            .await
            .unwrap();

        let args = InitCommand {
            path: Some(temp_dir.path().to_path_buf()),
            commands: None,
            force: false,
        };

        let result = run(args).await;
        assert!(result.is_ok());

        // Verify .claude directory was created
        assert!(temp_dir.path().join(".claude").exists());
    }

    #[tokio::test]
    async fn test_init_run_already_initialized() {
        // Test error conditions
        let temp_dir = TempDir::new().unwrap();

        // Initialize as git repo
        use crate::subprocess::ProcessCommandBuilder;
        let subprocess = SubprocessManager::production();
        subprocess
            .runner()
            .run(
                ProcessCommandBuilder::new("git")
                    .arg("init")
                    .current_dir(temp_dir.path())
                    .build(),
            )
            .await
            .unwrap();

        // Create .claude directory
        fs::create_dir(temp_dir.path().join(".claude")).unwrap();

        let args = InitCommand {
            path: Some(temp_dir.path().to_path_buf()),
            commands: None,
            force: false,
        };

        let result = run(args).await;
        assert!(result.is_ok()); // This should succeed but skip existing commands
    }
}
