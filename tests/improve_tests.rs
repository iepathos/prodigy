use std::process::Command;
use tempfile::TempDir;

/// Test extract_spec_from_git function
#[test]
fn test_extract_spec_from_git() {
    // Create a temporary git repository
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init git repo");

    // Configure git user
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to config name");

    // Create initial commit
    Command::new("git")
        .args(["commit", "--allow-empty", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create initial commit");

    // Create a commit with spec ID
    Command::new("git")
        .args([
            "commit",
            "--allow-empty",
            "-m",
            "review: generate improvement spec for iteration-1234567890-improvements",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create spec commit");

    // Now test extracting the spec ID
    let output = Command::new("git")
        .args(["log", "-1", "--pretty=format:%s"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get git log");

    let commit_msg = String::from_utf8_lossy(&output.stdout);
    assert!(commit_msg.contains("iteration-1234567890-improvements"));

    // Extract spec ID using regex (same pattern as in improve/mod.rs)
    let re = regex::Regex::new(r"iteration-\d+-improvements").unwrap();
    let spec_id = re.find(&commit_msg).map(|m| m.as_str());
    assert_eq!(spec_id, Some("iteration-1234567890-improvements"));
}

/// Test subprocess error handling
#[test]
fn test_subprocess_error_handling() {
    // Test running a non-existent command
    let result = std::process::Command::new("non_existent_command_xyz").output();

    assert!(result.is_err());
}

/// Test git command availability
#[test]
fn test_git_command_exists() {
    let output = Command::new("git")
        .arg("--version")
        .output()
        .expect("Failed to execute git");

    assert!(output.status.success());
    let version = String::from_utf8_lossy(&output.stdout);
    assert!(version.contains("git version"));
}

#[cfg(test)]
mod improve_command_tests {
    use mmm::improve::command::ImproveCommand;

    #[test]
    fn test_improve_command_creation() {
        let cmd = ImproveCommand {
            target: 8.0,
            show_progress: false,
            focus: None,
            config: None,
        };

        assert_eq!(cmd.target, 8.0);
        assert!(!cmd.show_progress);
        assert!(cmd.focus.is_none());
    }

    #[test]
    fn test_improve_command_with_focus() {
        let cmd = ImproveCommand {
            target: 9.0,
            show_progress: true,
            focus: Some("performance".to_string()),
            config: None,
        };

        assert_eq!(cmd.target, 9.0);
        assert!(cmd.show_progress);
        assert_eq!(cmd.focus, Some("performance".to_string()));
    }
}

#[cfg(test)]
mod session_tests {
    use mmm::improve::session::SessionSummary;

    #[test]
    fn test_session_summary_creation() {
        let summary = SessionSummary {
            initial_score: 6.0,
            final_score: 8.0,
            iterations: 3,
            files_changed: 5,
        };

        assert_eq!(summary.initial_score, 6.0);
        assert_eq!(summary.final_score, 8.0);
        assert_eq!(summary.iterations, 3);
        assert_eq!(summary.files_changed, 5);
    }
}
