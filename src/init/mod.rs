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
        if atty::is(atty::Stream::Stdin) {
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
}
