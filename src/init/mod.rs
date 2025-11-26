//! Prodigy project initialization for Claude Code integration
//!
//! This module provides functionality to initialize Prodigy commands in a project,
//! setting up the necessary directory structure and Claude Code command files
//! that enable automated code improvement workflows.
//!
//! # Key Features
//!
//! - **Git Repository Validation**: Ensures Prodigy is only installed in git repositories
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

/// Check if git is installed on the system
async fn is_git_installed(subprocess: &SubprocessManager) -> bool {
    use crate::subprocess::ProcessCommandBuilder;

    let command = ProcessCommandBuilder::new("git").arg("--version").build();

    subprocess
        .runner()
        .run(command)
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Initialize a git repository in the specified directory
async fn initialize_git_repository(path: &Path, subprocess: &SubprocessManager) -> Result<()> {
    use crate::subprocess::ProcessCommandBuilder;

    let command = ProcessCommandBuilder::new("git")
        .arg("init")
        .current_dir(path)
        .build();

    let output = subprocess
        .runner()
        .run(command)
        .await
        .context("Failed to run git init")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(output.stderr.as_bytes());
        anyhow::bail!("Failed to initialize git repository: {}", stderr);
    }

    Ok(())
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
#[cfg(test)]
fn should_process_templates(templates: &[templates::CommandTemplate]) -> bool {
    !templates.is_empty()
}

/// Check if existing commands need user confirmation
#[cfg(test)]
fn needs_user_confirmation(existing_commands: &[&str]) -> bool {
    !existing_commands.is_empty()
}

/// Check if we should proceed with installation based on existing commands
#[allow(dead_code)]
fn should_proceed_with_installation(existing_commands: &[&str]) -> bool {
    existing_commands.is_empty()
}

/// Validate preconditions for command installation
#[cfg(test)]
fn validate_installation_preconditions(templates: &[templates::CommandTemplate]) -> bool {
    !templates.is_empty()
}

/// Process existing commands check and confirmation flow
#[cfg(test)]
fn process_existing_commands_check(existing: Vec<&str>) -> Result<bool> {
    if !needs_user_confirmation(&existing) {
        return Ok(true);
    }

    display_existing_commands_warning(&existing);
    get_user_confirmation()
}

/// Process existing commands and get user confirmation if needed
#[cfg(test)]
fn process_existing_commands_pipeline(existing: &[&str]) -> Result<bool> {
    if existing.is_empty() {
        return Ok(true);
    }

    display_existing_commands_warning(existing);
    get_user_confirmation()
}

/// Determine if we should proceed based on existing commands
fn should_proceed_with_existing(existing: &[&str]) -> Result<bool> {
    if existing.is_empty() {
        Ok(true)
    } else {
        display_existing_commands_warning(existing);
        get_user_confirmation()
    }
}

/// Handle checking for existing commands and get user confirmation
fn handle_existing_commands(
    commands_dir: &Path,
    templates: &[templates::CommandTemplate],
) -> Result<bool> {
    match templates.is_empty() {
        true => Ok(true),
        false => {
            let existing = find_existing_commands(commands_dir, templates);
            should_proceed_with_existing(&existing)
        }
    }
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

    println!(
        "🚀 Initializing Prodigy commands in: {}",
        target_dir.display()
    );

    // Check if it's a git repository
    if !is_git_repository(&target_dir, subprocess).await {
        // Check if git is installed
        if !is_git_installed(subprocess).await {
            anyhow::bail!(
                "Error: git is not installed on your system.\n\
                 Prodigy requires git to manage workflow history.\n\
                 Please install git and try again."
            );
        }

        // Initialize git repository automatically
        println!("📦 Directory is not a git repository. Initializing git...");
        initialize_git_repository(&target_dir, subprocess).await?;
        println!("✅ Git repository initialized successfully.");
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

    #[tokio::test]
    async fn test_is_git_installed() {
        let subprocess = SubprocessManager::production();

        // Git should be installed on test systems
        assert!(is_git_installed(&subprocess).await);
    }

    #[tokio::test]
    async fn test_initialize_git_repository() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let subprocess = SubprocessManager::production();

        // Should not be a git repo initially
        assert!(!is_git_repository(path, &subprocess).await);

        // Initialize git repository
        let result = initialize_git_repository(path, &subprocess).await;
        assert!(result.is_ok());

        // Should now be a git repo
        assert!(is_git_repository(path, &subprocess).await);
        assert!(path.join(".git").exists());
    }

    #[test]
    fn test_get_templates() {
        let all_templates = templates::get_all_templates();
        // We now dynamically discover all prodigy-* commands
        assert!(all_templates.len() >= 6); // At least the original 6 commands

        // Test filtering by names
        let names = vec![
            "prodigy-lint".to_string(),
            "prodigy-code-review".to_string(),
        ];
        let filtered = templates::get_templates_by_names(&names);
        assert_eq!(filtered.len(), 2);
    }

    #[tokio::test]
    async fn test_run_init_not_git_repo_auto_init() {
        let temp_dir = TempDir::new().unwrap();
        let cmd = InitCommand {
            force: false,
            commands: None,
            path: Some(temp_dir.path().to_path_buf()),
        };

        // Since git is installed on test systems, this should auto-initialize
        let result = run(cmd).await;
        assert!(result.is_ok());

        // Verify git repository was created
        assert!(temp_dir.path().join(".git").exists());

        // Verify commands were installed
        let commands_dir = temp_dir.path().join(".claude").join("commands");
        assert!(commands_dir.exists());
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

    #[test]
    fn test_handle_existing_commands_multiple_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        // Create multiple existing commands
        fs::write(commands_dir.join("test-command1.md"), "existing content 1").unwrap();
        fs::write(commands_dir.join("test-command2.md"), "existing content 2").unwrap();
        fs::write(commands_dir.join("test-command3.md"), "existing content 3").unwrap();

        let templates = vec![
            templates::CommandTemplate {
                name: "test-command1",
                content: "new content 1",
                description: "Test command 1",
            },
            templates::CommandTemplate {
                name: "test-command2",
                content: "new content 2",
                description: "Test command 2",
            },
            templates::CommandTemplate {
                name: "test-command3",
                content: "new content 3",
                description: "Test command 3",
            },
            templates::CommandTemplate {
                name: "test-command4",
                content: "new content 4",
                description: "Test command 4",
            },
        ];

        // Should handle multiple conflicts
        let result = handle_existing_commands(&commands_dir, &templates);
        assert!(result.is_ok());
        assert!(result.unwrap()); // In non-interactive mode, should return true
    }

    #[test]
    fn test_display_existing_commands_warning() {
        // This test verifies the display function is callable
        // Output is to stdout, so we just verify it doesn't panic
        let existing = vec!["cmd1", "cmd2", "cmd3"];
        display_existing_commands_warning(&existing);
        // Function should complete without panic
    }

    #[test]
    fn test_display_existing_commands_warning_empty() {
        // Test with empty list
        let existing: Vec<&str> = vec![];
        display_existing_commands_warning(&existing);
        // Function should complete without panic even with empty list
    }

    #[test]
    fn test_display_existing_commands_warning_single() {
        // Test with single item
        let existing = vec!["single-command"];
        display_existing_commands_warning(&existing);
        // Function should complete without panic
    }

    #[test]
    fn test_get_user_confirmation_non_tty() {
        // In test environment, should return Ok(true)
        let result = get_user_confirmation();
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_get_user_confirmation_test_env() {
        // Verify behavior in test environment
        // When running tests, cfg!(test) is true, so get_user_confirmation
        // should skip interactive prompts and return true without needing
        // to set RUST_TEST_THREADS explicitly.
        let result = get_user_confirmation();
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_handle_existing_commands_partial_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        // Create some existing commands
        fs::write(commands_dir.join("existing-cmd.md"), "existing").unwrap();

        let templates = vec![
            templates::CommandTemplate {
                name: "existing-cmd",
                content: "new content",
                description: "Existing command",
            },
            templates::CommandTemplate {
                name: "new-cmd",
                content: "new content",
                description: "New command",
            },
        ];

        // Should handle partial conflicts (some exist, some don't)
        let result = handle_existing_commands(&commands_dir, &templates);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_should_proceed_with_existing_empty() {
        let existing: Vec<&str> = vec![];

        // Should return true when no existing commands
        let result = should_proceed_with_existing(&existing).unwrap();
        assert!(result);
    }

    #[test]
    fn test_should_proceed_with_existing_non_empty() {
        let existing = vec!["command1", "command2"];

        // In test environment, should handle existing commands
        // This will skip the interactive prompt and return true
        let result = should_proceed_with_existing(&existing);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_existing_commands_pattern_matching() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        // Test with empty templates - should use match arm for true
        let empty_templates = vec![];
        let result = handle_existing_commands(&commands_dir, &empty_templates).unwrap();
        assert!(result);

        // Test with non-empty templates - should use match arm for false
        let templates = vec![templates::CommandTemplate {
            name: "test-cmd",
            content: "content",
            description: "desc",
        }];
        let result = handle_existing_commands(&commands_dir, &templates).unwrap();
        assert!(result);
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

        // Should now auto-initialize git repository
        let result = validate_project_structure(&cmd, &subprocess).await;
        assert!(result.is_ok());

        // Verify git repository was created
        assert!(temp_dir.path().join(".git").exists());
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

    #[test]
    fn test_should_proceed_with_installation() {
        // Test with empty existing commands - should proceed
        let empty_existing: Vec<&str> = vec![];
        assert!(should_proceed_with_installation(&empty_existing));

        // Test with existing commands - should not proceed without confirmation
        let existing = vec!["prodigy-lint", "prodigy-review"];
        assert!(!should_proceed_with_installation(&existing));
    }

    #[test]
    fn test_validate_installation_preconditions() {
        use crate::init::templates::CommandTemplate;

        // Test with empty templates - should not proceed
        let empty_templates: Vec<CommandTemplate> = vec![];
        assert!(!validate_installation_preconditions(&empty_templates));

        // Test with templates - should proceed
        let templates = vec![CommandTemplate {
            name: "test-command",
            description: "Test command",
            content: "test content",
        }];
        assert!(validate_installation_preconditions(&templates));
    }

    #[test]
    fn test_process_existing_commands_pipeline_empty() {
        // Test with empty existing commands - should return Ok(true)
        let empty_existing: Vec<&str> = vec![];
        let result = process_existing_commands_pipeline(&empty_existing);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_process_existing_commands_pipeline_with_items() {
        // Test with existing commands - in test environment should still return Ok
        let existing = vec!["cmd1", "cmd2"];
        let result = process_existing_commands_pipeline(&existing);
        // In test environment (is_test_environment() returns true),
        // this should return Ok(true) after displaying warning
        assert!(result.is_ok());
    }
}
