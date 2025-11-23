use super::*;
use crate::git::error::GitError;
use crate::storage::error::StorageError;
use crate::subprocess::error::ProcessError;
use std::time::Duration;

#[test]
fn test_prodigy_error_construction() {
    // Test config error
    let err = ProdigyError::config("Configuration file not found");
    assert!(matches!(err, ProdigyError::Config { .. }));
    assert_eq!(err.exit_code(), 2);
    assert_eq!(err.code(), ErrorCode::CONFIG_GENERIC);

    // Test session error
    let err = ProdigyError::session("Session expired");
    assert!(matches!(err, ProdigyError::Session { .. }));
    assert_eq!(err.exit_code(), 3);
    assert_eq!(err.code(), ErrorCode::SESSION_GENERIC);

    // Test storage error
    let err = ProdigyError::storage("Storage unavailable");
    assert!(matches!(err, ProdigyError::Storage { .. }));
    assert_eq!(err.exit_code(), 4);
    assert_eq!(err.code(), ErrorCode::STORAGE_GENERIC);

    // Test execution error
    let err = ProdigyError::execution("Command failed");
    assert!(matches!(err, ProdigyError::Execution { .. }));
    assert_eq!(err.exit_code(), 5);
    assert_eq!(err.code(), ErrorCode::EXEC_GENERIC);

    // Test workflow error
    let err = ProdigyError::workflow("Workflow failed");
    assert!(matches!(err, ProdigyError::Workflow { .. }));
    assert_eq!(err.exit_code(), 6);
    assert_eq!(err.code(), ErrorCode::WORKFLOW_GENERIC);

    // Test git error
    let err = ProdigyError::git(ErrorCode::GIT_MERGE_CONFLICT, "Merge conflict", "merge");
    assert!(matches!(err, ProdigyError::Git { .. }));
    assert_eq!(err.exit_code(), 7);
    assert_eq!(err.code(), ErrorCode::GIT_MERGE_CONFLICT);

    // Test validation error
    let err = ProdigyError::validation("Invalid input");
    assert!(matches!(err, ProdigyError::Validation { .. }));
    assert_eq!(err.exit_code(), 8);
    assert_eq!(err.code(), ErrorCode::VALIDATION_GENERIC);

    // Test other error
    let err = ProdigyError::other("Unknown error");
    assert!(matches!(err, ProdigyError::Other { .. }));
    assert_eq!(err.exit_code(), 1);
    assert_eq!(err.code(), ErrorCode::OTHER_GENERIC);
}

#[test]
fn test_error_with_context() {
    let err = ProdigyError::config("Config error").with_context("Additional context");
    let err_str = err.to_string();
    assert!(err_str.contains("Config error"));
    assert!(err_str.contains("Additional context"));
}

#[test]
fn test_error_with_source() {
    let source_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let err = ProdigyError::storage("Storage error").with_source(source_err);

    let err_str = err.to_string();
    assert!(err_str.contains("Storage error"));
    assert!(err_str.contains("[E3000]"));
}

#[test]
fn test_git_error_conversion() {
    // Test NotARepository
    let git_err = GitError::NotARepository;
    let prodigy_err: ProdigyError = git_err.into();
    assert!(matches!(prodigy_err, ProdigyError::Git { .. }));
    assert_eq!(prodigy_err.code(), ErrorCode::GIT_NOT_REPO);

    // Test BranchNotFound
    let git_err = GitError::BranchNotFound("main".to_string());
    let prodigy_err: ProdigyError = git_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::GIT_BRANCH_NOT_FOUND);

    // Test MergeConflict
    let git_err = GitError::MergeConflict { files: vec![] };
    let prodigy_err: ProdigyError = git_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::GIT_MERGE_CONFLICT);

    // Test UncommittedChanges
    let git_err = GitError::UncommittedChanges;
    let prodigy_err: ProdigyError = git_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::GIT_UNCOMMITTED);

    // Test WorktreeExists
    let git_err = GitError::WorktreeExists("feature".to_string());
    let prodigy_err: ProdigyError = git_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::GIT_WORKTREE_EXISTS);

    // Test DetachedHead
    let git_err = GitError::DetachedHead;
    let prodigy_err: ProdigyError = git_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::GIT_DETACHED_HEAD);

    // Test CommandFailed
    let git_err = GitError::CommandFailed("git pull failed".to_string());
    let prodigy_err: ProdigyError = git_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::GIT_COMMAND_FAILED);
}

#[test]
fn test_storage_error_conversion() {
    // Test Io error
    let storage_err = StorageError::Io(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "Permission denied",
    ));
    let prodigy_err: ProdigyError = storage_err.into();
    assert!(matches!(prodigy_err, ProdigyError::Storage { .. }));
    assert_eq!(prodigy_err.code(), ErrorCode::STORAGE_IO_ERROR);

    // Test Serialization error
    let storage_err = StorageError::serialization("JSON parse error");
    let prodigy_err: ProdigyError = storage_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::STORAGE_SERIALIZATION_ERROR);

    // Test NotFound error
    let storage_err = StorageError::not_found("item");
    let prodigy_err: ProdigyError = storage_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::STORAGE_NOT_FOUND);

    // Test Lock error
    let storage_err = StorageError::lock("Lock failed");
    let prodigy_err: ProdigyError = storage_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::STORAGE_LOCK_FAILED);

    // Test Conflict error
    let storage_err = StorageError::conflict("Already exists");
    let prodigy_err: ProdigyError = storage_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::STORAGE_ALREADY_EXISTS);

    // Test Timeout error
    let storage_err = StorageError::Timeout(Duration::from_secs(5));
    let prodigy_err: ProdigyError = storage_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::STORAGE_TEMPORARY);
}

#[test]
fn test_process_error_conversion() {
    // Test CommandNotFound
    let proc_err = ProcessError::CommandNotFound("git".to_string());
    let prodigy_err: ProdigyError = proc_err.into();
    assert!(matches!(prodigy_err, ProdigyError::Execution { .. }));
    assert_eq!(prodigy_err.code(), ErrorCode::EXEC_COMMAND_NOT_FOUND);

    // Test Timeout
    let proc_err = ProcessError::Timeout(Duration::from_secs(30));
    let prodigy_err: ProdigyError = proc_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::EXEC_TIMEOUT);

    // Test ExitCode
    let proc_err = ProcessError::ExitCode(1);
    let prodigy_err: ProdigyError = proc_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::EXEC_SUBPROCESS_FAILED);
    if let ProdigyError::Execution { exit_code, .. } = prodigy_err {
        assert_eq!(exit_code, Some(1));
    }

    // Test Signal
    let proc_err = ProcessError::Signal(15);
    let prodigy_err: ProdigyError = proc_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::EXEC_SIGNAL_RECEIVED);

    // Test Io
    let proc_err = ProcessError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Command not found",
    ));
    let prodigy_err: ProdigyError = proc_err.into();
    assert_eq!(prodigy_err.code(), ErrorCode::EXEC_SPAWN_FAILED);
}

#[test]
fn test_error_code_descriptions() {
    // Test configuration error codes
    assert_eq!(
        describe_error_code(ErrorCode::CONFIG_NOT_FOUND),
        "Configuration file not found"
    );
    assert_eq!(
        describe_error_code(ErrorCode::CONFIG_INVALID_YAML),
        "Invalid YAML syntax in configuration"
    );

    // Test session error codes
    assert_eq!(
        describe_error_code(ErrorCode::SESSION_NOT_FOUND),
        "Session not found"
    );
    assert_eq!(
        describe_error_code(ErrorCode::SESSION_LOCKED),
        "Session is locked by another process"
    );

    // Test storage error codes
    assert_eq!(
        describe_error_code(ErrorCode::STORAGE_IO_ERROR),
        "Storage I/O error"
    );
    assert_eq!(
        describe_error_code(ErrorCode::STORAGE_DISK_FULL),
        "Storage disk is full"
    );

    // Test execution error codes
    assert_eq!(
        describe_error_code(ErrorCode::EXEC_TIMEOUT),
        "Command execution timeout"
    );
    assert_eq!(
        describe_error_code(ErrorCode::EXEC_COMMAND_NOT_FOUND),
        "Command not found"
    );

    // Test workflow error codes
    assert_eq!(
        describe_error_code(ErrorCode::WORKFLOW_NOT_FOUND),
        "Workflow not found"
    );
    assert_eq!(
        describe_error_code(ErrorCode::WORKFLOW_STEP_FAILED),
        "Workflow step failed"
    );

    // Test git error codes
    assert_eq!(
        describe_error_code(ErrorCode::GIT_NOT_REPO),
        "Not a git repository"
    );
    assert_eq!(
        describe_error_code(ErrorCode::GIT_MERGE_CONFLICT),
        "Git merge conflict"
    );

    // Test unknown error code
    assert_eq!(describe_error_code(65535), "Unknown error code");
}

#[test]
fn test_error_chaining() {
    // Create a chain of errors
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let storage_err = StorageError::Io(io_err);
    let prodigy_err: ProdigyError = storage_err.into();

    // Verify the error chain
    assert_eq!(prodigy_err.code(), ErrorCode::STORAGE_IO_ERROR);
    assert!(prodigy_err.to_string().contains("[E3001]"));
    assert!(prodigy_err.to_string().contains("Storage error"));
}

#[test]
fn test_error_formatting() {
    // Test error code formatting
    let err = ProdigyError::config_with_code(1001, "Config file missing");
    let formatted = err.to_string();
    assert!(formatted.contains("[E1001]"));
    assert!(formatted.contains("Configuration error"));
    assert!(formatted.contains("Config file missing"));

    // Test git error with operation
    let err = ProdigyError::git(6003, "Files have conflicts", "merge");
    let formatted = err.to_string();
    assert!(formatted.contains("[E6003]"));
    assert!(formatted.contains("Git operation failed"));
    assert!(formatted.contains("Files have conflicts"));
}

#[test]
fn test_lib_result_type() {
    // Test that LibResult works with ProdigyError
    fn test_function() -> crate::LibResult<String> {
        Ok("success".to_string())
    }

    fn test_error_function() -> crate::LibResult<String> {
        Err(ProdigyError::config("Test error"))
    }

    assert!(test_function().is_ok());
    assert!(test_error_function().is_err());

    if let Err(err) = test_error_function() {
        assert_eq!(err.exit_code(), 2);
    }
}

#[test]
fn test_error_recovery_checks() {
    // Test GitError recovery checks
    assert!(GitError::UncommittedChanges.is_recoverable());
    assert!(GitError::NothingToCommit.is_recoverable());
    assert!(GitError::DirtyWorkingTree.is_recoverable());
    assert!(GitError::RepositoryLocked.is_recoverable());
    assert!(!GitError::BranchNotFound("main".to_string()).is_recoverable());

    // Test GitError transient checks
    assert!(GitError::NetworkError("timeout".to_string()).is_transient());
    assert!(GitError::RepositoryLocked.is_transient());
    assert!(!GitError::BranchNotFound("main".to_string()).is_transient());

    // Test StorageError retry checks
    assert!(
        StorageError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout"))
            .is_retryable()
    );
    assert!(StorageError::Lock("busy".to_string()).is_retryable());
    assert!(StorageError::Timeout(Duration::from_secs(5)).is_retryable());
    assert!(!StorageError::NotFound("item".to_string()).is_retryable());
}

#[test]
fn test_error_context_chain() {
    let err = ProdigyError::storage("File not found")
        .context("Loading configuration")
        .context("Starting application");

    let chain = err.chain();
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].message, "Loading configuration");
    assert_eq!(chain[1].message, "Starting application");
}

#[test]
fn test_error_context_empty() {
    let err = ProdigyError::config("Config error");
    let chain = err.chain();
    assert!(chain.is_empty());
}

#[test]
fn test_error_source_chain() {
    let source = ProdigyError::storage("Disk full");
    let err = ProdigyError::execution("Command failed").with_error_source(source);

    assert!(err.error_source().is_some());
    let root = err.root_cause();
    assert!(matches!(root, ProdigyError::Storage { .. }));
}

#[test]
fn test_error_root_cause_no_source() {
    let err = ProdigyError::validation("Invalid input");
    let root = err.root_cause();
    // Root cause should be itself when no source
    assert!(matches!(root, ProdigyError::Validation { .. }));
}

#[test]
fn test_error_root_cause_deep_chain() {
    let err1 = ProdigyError::storage("Disk error");
    let err2 = ProdigyError::session("Session error").with_error_source(err1);
    let err3 = ProdigyError::workflow("Workflow error").with_error_source(err2);

    let root = err3.root_cause();
    assert!(matches!(root, ProdigyError::Storage { .. }));
}

#[test]
fn test_context_at_includes_location() {
    let err = ProdigyError::config("Config error").context_at("Loading file");

    let chain = err.chain();
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].message, "Loading file");
    // Location should be set by #[track_caller]
    assert!(chain[0].location.is_some());
}

#[test]
fn test_developer_message_includes_context() {
    let err = ProdigyError::workflow("Workflow failed")
        .context("Executing step 1")
        .context("Running workflow");

    let dev_msg = err.developer_message();
    assert!(dev_msg.contains("Context chain"));
    assert!(dev_msg.contains("Executing step 1"));
    assert!(dev_msg.contains("Running workflow"));
}

#[test]
fn test_developer_message_includes_source() {
    let source = ProdigyError::storage("File not found");
    let err = ProdigyError::workflow("Workflow failed").with_error_source(source);

    let dev_msg = err.developer_message();
    assert!(dev_msg.contains("Caused by"));
}

#[test]
fn test_combined_context_and_source() {
    let source = ProdigyError::storage("Disk full").context("Writing file");
    let err = ProdigyError::execution("Command failed")
        .context("Running build")
        .with_error_source(source);

    let chain = err.chain();
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].message, "Running build");

    let root = err.root_cause();
    assert!(matches!(root, ProdigyError::Storage { .. }));
}

#[test]
fn test_user_message_unchanged() {
    let err = ProdigyError::config("Config error")
        .context("Loading file")
        .context("Starting app");

    // user_message should not include context chain
    let user_msg = err.user_message();
    assert!(user_msg.contains("Config error"));
    // Context is separate from user message
}

#[test]
fn test_context_fluent_api() {
    // Test that context() returns Self for chaining
    let err = ProdigyError::validation("Invalid input")
        .context("Step 1")
        .context("Step 2")
        .context("Step 3");

    assert_eq!(err.chain().len(), 3);
}
