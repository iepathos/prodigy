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
