//! Shell integration with completions and git hooks

use anyhow::Result;
use colored::*;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Shell integration manager
pub struct ShellIntegration;

impl ShellIntegration {
    /// Install git hooks
    pub fn install_hooks() -> Result<()> {
        let git_dir = Path::new(".git");
        if !git_dir.exists() {
            anyhow::bail!("Not in a git repository");
        }

        let hooks_dir = git_dir.join("hooks");
        fs::create_dir_all(&hooks_dir)?;

        // Pre-commit hook
        let pre_commit_path = hooks_dir.join("pre-commit");
        let pre_commit_content = r#"#!/bin/sh
# MMM pre-commit hook
# Automatically improve code before committing

# Check if MMM is installed
if ! command -v mmm &> /dev/null; then
    echo "⚠️  MMM not found in PATH. Skipping automatic improvement."
    exit 0
fi

# Check if we should skip
if [ "$MMM_SKIP_HOOK" = "1" ]; then
    exit 0
fi

echo "🚀 Running MMM to improve code before commit..."

# Run MMM with conservative settings
mmm improve --conservative --quick

# Check if MMM made changes
if [ $? -eq 0 ]; then
    # Add the changes MMM made
    git add -A
    echo "✨ Code improvements applied!"
else
    echo "⚠️  MMM improvement failed. Continuing with commit..."
fi

exit 0
"#;

        fs::write(&pre_commit_path, pre_commit_content)?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&pre_commit_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&pre_commit_path, perms)?;
        }

        println!("{} Installed git pre-commit hook", "✅".green());
        println!();
        println!("Now MMM will automatically improve code before each commit.");
        println!("Use {} to skip.", "'git commit --no-verify'".cyan());

        Ok(())
    }

    /// Generate shell completions
    pub fn generate_completions(shell: Shell) -> String {
        match shell {
            Shell::Bash => Self::bash_completions(),
            Shell::Zsh => Self::zsh_completions(),
            Shell::Fish => Self::fish_completions(),
        }
    }

    fn bash_completions() -> String {
        r#"# MMM bash completion

_mmm() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    
    # Main commands
    local commands="improve test config install-hooks help"
    
    # Options for improve command
    local improve_opts="--focus --target --dry-run --preview --resume --verbose --conservative --quick"
    
    # Focus areas
    local focus_areas="errors tests docs performance security types style architecture"
    
    case "${prev}" in
        mmm)
            COMPREPLY=( $(compgen -W "${commands}" -- ${cur}) )
            return 0
            ;;
        improve)
            COMPREPLY=( $(compgen -W "${improve_opts}" -- ${cur}) )
            return 0
            ;;
        --focus)
            COMPREPLY=( $(compgen -W "${focus_areas}" -- ${cur}) )
            return 0
            ;;
        *)
            ;;
    esac
    
    COMPREPLY=( $(compgen -W "${opts}" -- ${cur}) )
    return 0
}

complete -F _mmm mmm
"#.to_string()
    }

    fn zsh_completions() -> String {
        r#"#compdef mmm

_mmm() {
    local -a commands
    commands=(
        'improve:Improve code quality'
        'test:Run tests'
        'config:Configure MMM'
        'install-hooks:Install git hooks'
        'help:Show help'
    )
    
    local -a improve_options
    improve_options=(
        '--focus[Focus on specific area]:area:(errors tests docs performance security types style architecture)'
        '--target[Target quality score]:score:'
        '--dry-run[Preview changes without applying]'
        '--preview[Interactive preview mode]'
        '--resume[Resume from previous session]'
        '--verbose[Verbose output]'
        '--conservative[Safe improvements only]'
        '--quick[Quick improvements only]'
    )
    
    case $words[2] in
        improve)
            _arguments $improve_options
            ;;
        *)
            _describe 'command' commands
            ;;
    esac
}

_mmm "$@"
"#.to_string()
    }

    fn fish_completions() -> String {
        r#"# MMM fish completion

# Commands
complete -c mmm -n "__fish_use_subcommand" -a improve -d "Improve code quality"
complete -c mmm -n "__fish_use_subcommand" -a test -d "Run tests"
complete -c mmm -n "__fish_use_subcommand" -a config -d "Configure MMM"
complete -c mmm -n "__fish_use_subcommand" -a install-hooks -d "Install git hooks"
complete -c mmm -n "__fish_use_subcommand" -a help -d "Show help"

# Improve options
complete -c mmm -n "__fish_seen_subcommand_from improve" -l focus -d "Focus area" -xa "errors tests docs performance security types style architecture"
complete -c mmm -n "__fish_seen_subcommand_from improve" -l target -d "Target score"
complete -c mmm -n "__fish_seen_subcommand_from improve" -l dry-run -d "Preview only"
complete -c mmm -n "__fish_seen_subcommand_from improve" -l preview -d "Interactive mode"
complete -c mmm -n "__fish_seen_subcommand_from improve" -l resume -d "Resume session"
complete -c mmm -n "__fish_seen_subcommand_from improve" -l verbose -d "Verbose output"
complete -c mmm -n "__fish_seen_subcommand_from improve" -l conservative -d "Safe only"
complete -c mmm -n "__fish_seen_subcommand_from improve" -l quick -d "Quick only"
"#.to_string()
    }
}

/// Supported shells
#[derive(Debug, Clone, Copy)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl Shell {
    /// Detect current shell
    pub fn detect() -> Option<Self> {
        std::env::var("SHELL").ok().and_then(|shell| {
            if shell.contains("bash") {
                Some(Shell::Bash)
            } else if shell.contains("zsh") {
                Some(Shell::Zsh)
            } else if shell.contains("fish") {
                Some(Shell::Fish)
            } else {
                None
            }
        })
    }
}

/// Completions installer
pub struct Completions;

impl Completions {
    /// Install completions for current shell
    pub fn install() -> Result<()> {
        let shell = Shell::detect().ok_or_else(|| anyhow::anyhow!("Could not detect shell"))?;

        let completions = ShellIntegration::generate_completions(shell);

        match shell {
            Shell::Bash => Self::install_bash(completions)?,
            Shell::Zsh => Self::install_zsh(completions)?,
            Shell::Fish => Self::install_fish(completions)?,
        }

        println!(
            "{} Installed {} completions",
            "✅".green(),
            format!("{shell:?}").cyan()
        );
        println!(
            "Restart your shell or run {} to enable completions",
            "source ~/.bashrc".cyan()
        );

        Ok(())
    }

    fn install_bash(completions: String) -> Result<()> {
        let completions_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join(".bash_completion.d");

        fs::create_dir_all(&completions_dir)?;
        let completion_file = completions_dir.join("mmm");
        fs::write(&completion_file, completions)?;

        // Try to add to .bashrc
        let bashrc = dirs::home_dir().unwrap().join(".bashrc");
        if bashrc.exists() {
            let content = fs::read_to_string(&bashrc)?;
            if !content.contains("mmm completion") {
                let mut file = fs::OpenOptions::new().append(true).open(&bashrc)?;
                writeln!(file, "\n# MMM completion")?;
                writeln!(
                    file,
                    "[ -f {} ] && source {}",
                    completion_file.display(),
                    completion_file.display()
                )?;
            }
        }

        Ok(())
    }

    fn install_zsh(completions: String) -> Result<()> {
        let completions_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join(".zsh/completions");

        fs::create_dir_all(&completions_dir)?;
        let completion_file = completions_dir.join("_mmm");
        fs::write(&completion_file, completions)?;

        // Try to add to .zshrc
        let zshrc = dirs::home_dir().unwrap().join(".zshrc");
        if zshrc.exists() {
            let content = fs::read_to_string(&zshrc)?;
            if !content.contains("mmm completion") {
                let mut file = fs::OpenOptions::new().append(true).open(&zshrc)?;
                writeln!(file, "\n# MMM completion")?;
                writeln!(file, "fpath=(~/.zsh/completions $fpath)")?;
                writeln!(file, "autoload -Uz compinit && compinit")?;
            }
        }

        Ok(())
    }

    fn install_fish(completions: String) -> Result<()> {
        let completions_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join(".config/fish/completions");

        fs::create_dir_all(&completions_dir)?;
        let completion_file = completions_dir.join("mmm.fish");
        fs::write(&completion_file, completions)?;

        Ok(())
    }
}
