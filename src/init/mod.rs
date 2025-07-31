pub mod command;
pub mod templates;

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::init::command::InitCommand;

/// Check if the current directory is a git repository
fn is_git_repository(path: &Path) -> bool {
    // Check for .git directory or file (in case of git worktree)
    if path.join(".git").exists() {
        return true;
    }

    // Also check using git command to handle edge cases
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(path)
        .output()
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
            println!("   Available commands: mmm-code-review, mmm-implement-spec, mmm-lint, mmm-product-enhance, mmm-merge-worktree, mmm-cleanup-tech-debt");
            return Ok(vec![]);
        }
        Ok(selected)
    } else {
        Ok(templates::get_all_templates())
    }
}

/// Handle checking for existing commands and get user confirmation
fn handle_existing_commands(
    commands_dir: &Path,
    templates: &[templates::CommandTemplate],
) -> Result<bool> {
    if templates.is_empty() {
        return Ok(true);
    }

    let existing: Vec<_> = templates
        .iter()
        .filter(|t| commands_dir.join(format!("{}.md", t.name)).exists())
        .map(|t| t.name)
        .collect();

    if !existing.is_empty() {
        println!("\n⚠️  The following commands already exist:");
        for name in &existing {
            println!("   - {name}");
        }
        println!(
            "\nUse --force to overwrite existing commands, or --commands to select specific ones."
        );
        println!("Example: mmm init --commands mmm-lint,mmm-product-enhance");

        // Ask for confirmation in interactive mode
        // Skip interactive prompt in test environments
        let is_test = std::env::var("CARGO_TARGET_TMPDIR").is_ok()
            || std::env::var("RUST_TEST_THREADS").is_ok()
            || cfg!(test);

        if atty::is(atty::Stream::Stdin) && !is_test {
            print!("\nDo you want to continue and skip existing commands? (y/N): ");
            use std::io::{self, Write};
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
    }

    Ok(true)
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
        println!("   3. Run 'mmm cook' to start improving your code");
        println!("\n💡 Tip: You can always reinstall default commands with 'mmm init --force'");
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
fn validate_project_structure(cmd: &InitCommand) -> Result<(PathBuf, PathBuf)> {
    // Determine the target directory
    let target_dir = cmd.path.clone().unwrap_or_else(|| PathBuf::from("."));
    let target_dir = target_dir
        .canonicalize()
        .with_context(|| format!("Failed to resolve path: {}", target_dir.display()))?;

    println!("🚀 Initializing MMM commands in: {}", target_dir.display());

    // Check if it's a git repository
    if !is_git_repository(&target_dir) {
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
    let (_target_dir, commands_dir) = validate_project_structure(&cmd)?;

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

    #[test]
    fn test_is_git_repository() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Should not be a git repo initially
        assert!(!is_git_repository(path));

        // Initialize git repo
        Command::new("git")
            .arg("init")
            .current_dir(path)
            .output()
            .unwrap();

        // Should now be a git repo
        assert!(is_git_repository(path));
    }

    #[test]
    fn test_get_templates() {
        let all_templates = templates::get_all_templates();
        assert_eq!(all_templates.len(), 6);

        // Test filtering by names
        let names = vec!["mmm-lint".to_string(), "mmm-code-review".to_string()];
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
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
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
        assert!(commands_dir.join("mmm-code-review.md").exists());
        assert!(commands_dir.join("mmm-lint.md").exists());
    }

    #[tokio::test]
    async fn test_run_init_with_existing_commands() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        // Create existing command
        let commands_dir = temp_dir.path().join(".claude").join("commands");
        std::fs::create_dir_all(&commands_dir).unwrap();
        std::fs::write(commands_dir.join("mmm-code-review.md"), "existing content").unwrap();

        let cmd = InitCommand {
            force: false,
            commands: None,
            path: Some(temp_dir.path().to_path_buf()),
        };

        // Should skip existing commands
        let result = run(cmd).await;
        assert!(result.is_ok());

        // Check existing file wasn't overwritten
        let content = std::fs::read_to_string(commands_dir.join("mmm-code-review.md")).unwrap();
        assert_eq!(content, "existing content");
    }

    #[tokio::test]
    async fn test_run_init_force_overwrite() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        // Create existing command
        let commands_dir = temp_dir.path().join(".claude").join("commands");
        std::fs::create_dir_all(&commands_dir).unwrap();
        std::fs::write(commands_dir.join("mmm-code-review.md"), "old content").unwrap();

        let cmd = InitCommand {
            force: true,
            commands: None,
            path: Some(temp_dir.path().to_path_buf()),
        };

        let result = run(cmd).await;
        assert!(result.is_ok());

        // Check file was overwritten
        let content = std::fs::read_to_string(commands_dir.join("mmm-code-review.md")).unwrap();
        assert!(content.contains("Analyze code"));
        assert!(!content.contains("old content"));
    }

    #[tokio::test]
    async fn test_run_init_specific_commands() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        let cmd = InitCommand {
            force: false,
            commands: Some(vec!["mmm-code-review".to_string(), "mmm-lint".to_string()]),
            path: Some(temp_dir.path().to_path_buf()),
        };

        let result = run(cmd).await;
        assert!(result.is_ok());

        let commands_dir = temp_dir.path().join(".claude").join("commands");

        // Should only install specified commands
        assert!(commands_dir.join("mmm-code-review.md").exists());
        assert!(commands_dir.join("mmm-lint.md").exists());
        assert!(!commands_dir.join("mmm-implement-spec.md").exists());
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
    fn test_validate_project_structure_not_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let cmd = InitCommand {
            path: Some(temp_dir.path().to_path_buf()),
            commands: None,
            force: false,
        };

        let result = validate_project_structure(&cmd);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("git repository"));
    }

    #[test]
    fn test_validate_project_structure_with_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let real_path = temp_dir.path().join("real");
        let symlink_path = temp_dir.path().join("symlink");

        fs::create_dir_all(&real_path).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_path, &symlink_path).unwrap();

        // Initialize as git repo
        Command::new("git")
            .arg("init")
            .current_dir(&real_path)
            .output()
            .unwrap();

        let cmd = InitCommand {
            path: Some(symlink_path),
            commands: None,
            force: false,
        };

        #[cfg(unix)]
        {
            let result = validate_project_structure(&cmd);
            assert!(result.is_ok());
        }
    }
}
