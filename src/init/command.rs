use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct InitCommand {
    /// Force overwrite existing commands
    #[arg(short, long)]
    pub force: bool,

    /// Specific commands to install (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    pub commands: Option<Vec<String>>,

    /// Directory to initialize (defaults to current)
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_init_command_defaults() {
        let args = vec!["prodigy"];
        let command = InitCommand::try_parse_from(args).unwrap();

        assert!(!command.force);
        assert!(command.commands.is_none());
        assert!(command.path.is_none());
    }

    #[test]
    fn test_init_command_with_force() {
        let args = vec!["prodigy", "--force"];
        let command = InitCommand::try_parse_from(args).unwrap();

        assert!(command.force);
        assert!(command.commands.is_none());
        assert!(command.path.is_none());
    }

    #[test]
    fn test_init_command_with_path() {
        let args = vec!["prodigy", "--path", "/test/path"];
        let command = InitCommand::try_parse_from(args).unwrap();

        assert!(!command.force);
        assert!(command.commands.is_none());
        assert_eq!(command.path, Some(PathBuf::from("/test/path")));
    }

    #[test]
    fn test_init_command_with_single_command() {
        let args = vec!["prodigy", "--commands", "test-command"];
        let command = InitCommand::try_parse_from(args).unwrap();

        assert!(!command.force);
        assert_eq!(command.commands, Some(vec!["test-command".to_string()]));
        assert!(command.path.is_none());
    }

    #[test]
    fn test_init_command_with_multiple_commands() {
        let args = vec!["prodigy", "--commands", "cmd1,cmd2,cmd3"];
        let command = InitCommand::try_parse_from(args).unwrap();

        assert!(!command.force);
        assert_eq!(
            command.commands,
            Some(vec![
                "cmd1".to_string(),
                "cmd2".to_string(),
                "cmd3".to_string()
            ])
        );
        assert!(command.path.is_none());
    }

    #[test]
    fn test_init_command_short_flags() {
        let args = vec!["prodigy", "-f", "-c", "test", "-p", "/path"];
        let command = InitCommand::try_parse_from(args).unwrap();

        assert!(command.force);
        assert_eq!(command.commands, Some(vec!["test".to_string()]));
        assert_eq!(command.path, Some(PathBuf::from("/path")));
    }

    #[test]
    fn test_init_command_combined_options() {
        let args = vec![
            "prodigy",
            "--force",
            "--commands",
            "install,update",
            "--path",
            "/custom/dir",
        ];
        let command = InitCommand::try_parse_from(args).unwrap();

        assert!(command.force);
        assert_eq!(
            command.commands,
            Some(vec!["install".to_string(), "update".to_string()])
        );
        assert_eq!(command.path, Some(PathBuf::from("/custom/dir")));
    }

    #[test]
    fn test_init_command_invalid_args() {
        let args = vec!["prodigy", "--invalid"];
        let result = InitCommand::try_parse_from(args);

        assert!(result.is_err());
    }

    #[test]
    fn test_init_command_help() {
        let args = vec!["prodigy", "--help"];
        let result = InitCommand::try_parse_from(args);

        // Help flag causes parsing to fail with a help error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }
}
