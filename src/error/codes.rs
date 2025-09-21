/// Error code registry for Prodigy
///
/// Error codes are organized by category:
/// - 1000-1999: Configuration errors
/// - 2000-2999: Session errors
/// - 3000-3999: Storage errors
/// - 4000-4999: Execution errors
/// - 5000-5999: Workflow errors
/// - 6000-6999: Git errors
/// - 7000-7999: Validation errors
/// - 9000-9999: Other errors
#[allow(dead_code)]
pub struct ErrorCode;

impl ErrorCode {
    // Configuration errors (1000-1999)
    pub const CONFIG_GENERIC: u16 = 1000;
    pub const CONFIG_NOT_FOUND: u16 = 1001;
    pub const CONFIG_INVALID_YAML: u16 = 1002;
    pub const CONFIG_INVALID_JSON: u16 = 1003;
    pub const CONFIG_MISSING_REQUIRED: u16 = 1004;
    pub const CONFIG_INVALID_VALUE: u16 = 1005;
    pub const CONFIG_PATH_ERROR: u16 = 1006;
    pub const CONFIG_PARSE_ERROR: u16 = 1007;
    pub const CONFIG_VALIDATION_FAILED: u16 = 1008;
    pub const CONFIG_INCOMPATIBLE_VERSION: u16 = 1009;

    // Session errors (2000-2999)
    pub const SESSION_GENERIC: u16 = 2000;
    pub const SESSION_NOT_FOUND: u16 = 2001;
    pub const SESSION_ALREADY_EXISTS: u16 = 2002;
    pub const SESSION_CORRUPTED: u16 = 2003;
    pub const SESSION_LOCKED: u16 = 2004;
    pub const SESSION_EXPIRED: u16 = 2005;
    pub const SESSION_INVALID_STATE: u16 = 2006;
    pub const SESSION_PERMISSION_DENIED: u16 = 2007;
    pub const SESSION_STORAGE_FULL: u16 = 2008;

    // Storage errors (3000-3999)
    pub const STORAGE_GENERIC: u16 = 3000;
    pub const STORAGE_IO_ERROR: u16 = 3001;
    pub const STORAGE_PERMISSION_DENIED: u16 = 3002;
    pub const STORAGE_DISK_FULL: u16 = 3003;
    pub const STORAGE_NOT_FOUND: u16 = 3004;
    pub const STORAGE_ALREADY_EXISTS: u16 = 3005;
    pub const STORAGE_CORRUPTED: u16 = 3006;
    pub const STORAGE_LOCK_FAILED: u16 = 3007;
    pub const STORAGE_LOCK_BUSY: u16 = 3008;
    pub const STORAGE_TEMPORARY: u16 = 3009;
    pub const STORAGE_BACKEND_ERROR: u16 = 3010;
    pub const STORAGE_SERIALIZATION_ERROR: u16 = 3011;
    pub const STORAGE_DESERIALIZATION_ERROR: u16 = 3012;

    // Execution errors (4000-4999)
    pub const EXEC_GENERIC: u16 = 4000;
    pub const EXEC_COMMAND_NOT_FOUND: u16 = 4001;
    pub const EXEC_TIMEOUT: u16 = 4002;
    pub const EXEC_SUBPROCESS_FAILED: u16 = 4003;
    pub const EXEC_PERMISSION_DENIED: u16 = 4004;
    pub const EXEC_SIGNAL_RECEIVED: u16 = 4005;
    pub const EXEC_INTERRUPTED: u16 = 4006;
    pub const EXEC_SPAWN_FAILED: u16 = 4007;
    pub const EXEC_OUTPUT_ERROR: u16 = 4008;
    pub const EXEC_STDIN_ERROR: u16 = 4009;
    pub const EXEC_ENVIRONMENT_ERROR: u16 = 4010;

    // Workflow errors (5000-5999)
    pub const WORKFLOW_GENERIC: u16 = 5000;
    pub const WORKFLOW_NOT_FOUND: u16 = 5001;
    pub const WORKFLOW_INVALID_SYNTAX: u16 = 5002;
    pub const WORKFLOW_STEP_FAILED: u16 = 5003;
    pub const WORKFLOW_VALIDATION_FAILED: u16 = 5004;
    pub const WORKFLOW_TIMEOUT: u16 = 5005;
    pub const WORKFLOW_CANCELLED: u16 = 5006;
    pub const WORKFLOW_DEPENDENCY_FAILED: u16 = 5007;
    pub const WORKFLOW_CHECKPOINT_ERROR: u16 = 5008;
    pub const WORKFLOW_RESUME_ERROR: u16 = 5009;
    pub const WORKFLOW_CIRCULAR_DEPENDENCY: u16 = 5010;
    pub const WORKFLOW_VARIABLE_NOT_FOUND: u16 = 5011;
    pub const WORKFLOW_INTERPOLATION_ERROR: u16 = 5012;

    // Git errors (6000-6999)
    pub const GIT_GENERIC: u16 = 6000;
    pub const GIT_REPO_NOT_FOUND: u16 = 6001;
    pub const GIT_DIRTY_WORKTREE: u16 = 6002;
    pub const GIT_MERGE_CONFLICT: u16 = 6003;
    pub const GIT_BRANCH_NOT_FOUND: u16 = 6004;
    pub const GIT_CHECKOUT_FAILED: u16 = 6005;
    pub const GIT_COMMIT_FAILED: u16 = 6006;
    pub const GIT_PUSH_FAILED: u16 = 6007;
    pub const GIT_PULL_FAILED: u16 = 6008;
    pub const GIT_REMOTE_ERROR: u16 = 6009;
    pub const GIT_WORKTREE_ERROR: u16 = 6010;
    pub const GIT_AUTH_FAILED: u16 = 6011;

    // Validation errors (7000-7999)
    pub const VALIDATION_GENERIC: u16 = 7000;
    pub const VALIDATION_REQUIRED_FIELD: u16 = 7001;
    pub const VALIDATION_INVALID_TYPE: u16 = 7002;
    pub const VALIDATION_OUT_OF_RANGE: u16 = 7003;
    pub const VALIDATION_PATTERN_MISMATCH: u16 = 7004;
    pub const VALIDATION_INVALID_FORMAT: u16 = 7005;
    pub const VALIDATION_CONSTRAINT_VIOLATION: u16 = 7006;
    pub const VALIDATION_DUPLICATE_VALUE: u16 = 7007;
    pub const VALIDATION_INVALID_INPUT: u16 = 7008;
    pub const VALIDATION_INVALID_DATA: u16 = 7009;

    // Other errors (9000-9999)
    pub const OTHER_GENERIC: u16 = 9000;
    pub const OTHER_UNEXPECTED: u16 = 9001;
    pub const OTHER_NOT_IMPLEMENTED: u16 = 9002;
    pub const OTHER_DEPRECATED: u16 = 9003;
    pub const OTHER_INTERNAL_ERROR: u16 = 9004;
}

/// Get a human-readable description for an error code
pub fn describe_error_code(code: u16) -> &'static str {
    match code {
        // Configuration errors
        1000 => "Generic configuration error",
        1001 => "Configuration file not found",
        1002 => "Invalid YAML syntax in configuration",
        1003 => "Invalid JSON syntax in configuration",
        1004 => "Required configuration field is missing",
        1005 => "Invalid value in configuration",
        1006 => "Configuration path error",
        1007 => "Failed to parse configuration",
        1008 => "Configuration validation failed",
        1009 => "Incompatible configuration version",

        // Session errors
        2000 => "Generic session error",
        2001 => "Session not found",
        2002 => "Session already exists",
        2003 => "Session data is corrupted",
        2004 => "Session is locked by another process",
        2005 => "Session has expired",
        2006 => "Session is in invalid state",
        2007 => "Permission denied for session operation",
        2008 => "Session storage is full",

        // Storage errors
        3000 => "Generic storage error",
        3001 => "Storage I/O error",
        3002 => "Storage permission denied",
        3003 => "Storage disk is full",
        3004 => "Storage item not found",
        3005 => "Storage item already exists",
        3006 => "Storage data is corrupted",
        3007 => "Failed to acquire storage lock",
        3008 => "Storage lock is busy",
        3009 => "Temporary storage error",
        3010 => "Storage backend error",
        3011 => "Storage serialization error",
        3012 => "Storage deserialization error",

        // Execution errors
        4000 => "Generic execution error",
        4001 => "Command not found",
        4002 => "Command execution timeout",
        4003 => "Subprocess failed",
        4004 => "Permission denied to execute command",
        4005 => "Command received signal",
        4006 => "Command execution interrupted",
        4007 => "Failed to spawn subprocess",
        4008 => "Command output error",
        4009 => "Command stdin error",
        4010 => "Command environment error",

        // Workflow errors
        5000 => "Generic workflow error",
        5001 => "Workflow not found",
        5002 => "Invalid workflow syntax",
        5003 => "Workflow step failed",
        5004 => "Workflow validation failed",
        5005 => "Workflow timeout",
        5006 => "Workflow cancelled",
        5007 => "Workflow dependency failed",
        5008 => "Workflow checkpoint error",
        5009 => "Workflow resume error",
        5010 => "Circular dependency in workflow",
        5011 => "Workflow variable not found",
        5012 => "Workflow variable interpolation error",

        // Git errors
        6000 => "Generic git error",
        6001 => "Git repository not found",
        6002 => "Git worktree has uncommitted changes",
        6003 => "Git merge conflict",
        6004 => "Git branch not found",
        6005 => "Git checkout failed",
        6006 => "Git commit failed",
        6007 => "Git push failed",
        6008 => "Git pull failed",
        6009 => "Git remote error",
        6010 => "Git worktree error",
        6011 => "Git authentication failed",

        // Validation errors
        7000 => "Generic validation error",
        7001 => "Required field is missing",
        7002 => "Invalid data type",
        7003 => "Value out of allowed range",
        7004 => "Value doesn't match required pattern",
        7005 => "Invalid format",
        7006 => "Constraint violation",
        7007 => "Duplicate value not allowed",
        7008 => "Invalid input",
        7009 => "Invalid data",

        // Other errors
        9000 => "Generic error",
        9001 => "Unexpected error",
        9002 => "Feature not implemented",
        9003 => "Feature is deprecated",
        9004 => "Internal error",

        _ => "Unknown error code",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_ranges() {
        assert!(ErrorCode::CONFIG_GENERIC >= 1000 && ErrorCode::CONFIG_GENERIC < 2000);
        assert!(ErrorCode::SESSION_GENERIC >= 2000 && ErrorCode::SESSION_GENERIC < 3000);
        assert!(ErrorCode::STORAGE_GENERIC >= 3000 && ErrorCode::STORAGE_GENERIC < 4000);
        assert!(ErrorCode::EXEC_GENERIC >= 4000 && ErrorCode::EXEC_GENERIC < 5000);
        assert!(ErrorCode::WORKFLOW_GENERIC >= 5000 && ErrorCode::WORKFLOW_GENERIC < 6000);
        assert!(ErrorCode::GIT_GENERIC >= 6000 && ErrorCode::GIT_GENERIC < 7000);
        assert!(ErrorCode::VALIDATION_GENERIC >= 7000 && ErrorCode::VALIDATION_GENERIC < 8000);
        assert!(ErrorCode::OTHER_GENERIC >= 9000 && ErrorCode::OTHER_GENERIC < 10000);
    }

    #[test]
    fn test_error_code_descriptions() {
        assert_eq!(describe_error_code(1001), "Configuration file not found");
        assert_eq!(describe_error_code(4002), "Command execution timeout");
        assert_eq!(describe_error_code(65535), "Unknown error code");
    }
}
