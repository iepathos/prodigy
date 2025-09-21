//! Error types for the storage abstraction layer

use std::fmt;
use thiserror::Error;
use crate::error::{ProdigyError, ErrorCode};

/// Result type for storage operations
pub type StorageResult<T> = Result<T, StorageError>;

/// Storage error types
#[derive(Error, Debug)]
pub enum StorageError {
    /// I/O operation failed
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/deserialization failed
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Database operation failed
    #[error("Database error: {0}")]
    Database(String),

    /// Lock acquisition failed
    #[error("Lock error: {0}")]
    Lock(String),

    /// Item not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Operation conflict (concurrent modification)
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Storage backend unavailable
    #[error("Backend unavailable: {0}")]
    Unavailable(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Transaction failed
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// Connection error
    #[error("Connection error: {0}")]
    Connection(String),

    /// Timeout occurred
    #[error("Timeout: operation took longer than {0:?}")]
    Timeout(std::time::Duration),

    /// Generic error wrapper
    #[error("Storage error: {0}")]
    Other(#[from] anyhow::Error),
}

impl StorageError {
    /// Create a serialization error
    pub fn serialization<E: fmt::Display>(err: E) -> Self {
        Self::Serialization(err.to_string())
    }

    /// Create a deserialization error
    pub fn deserialization<E: fmt::Display>(err: E) -> Self {
        Self::Serialization(err.to_string())
    }

    /// Create a database error
    pub fn database<E: fmt::Display>(err: E) -> Self {
        Self::Database(err.to_string())
    }

    /// Create a lock error
    pub fn lock<E: fmt::Display>(err: E) -> Self {
        Self::Lock(err.to_string())
    }

    /// Create a not found error
    pub fn not_found<E: fmt::Display>(item: E) -> Self {
        Self::NotFound(item.to_string())
    }

    /// Create a conflict error
    pub fn conflict<E: fmt::Display>(msg: E) -> Self {
        Self::Conflict(msg.to_string())
    }

    /// Create an unavailable error
    pub fn unavailable<E: fmt::Display>(msg: E) -> Self {
        Self::Unavailable(msg.to_string())
    }

    /// Create a configuration error
    pub fn configuration<E: fmt::Display>(msg: E) -> Self {
        Self::Configuration(msg.to_string())
    }

    /// Create a transaction error
    pub fn transaction<E: fmt::Display>(msg: E) -> Self {
        Self::Transaction(msg.to_string())
    }

    /// Create a connection error
    pub fn connection<E: fmt::Display>(msg: E) -> Self {
        Self::Connection(msg.to_string())
    }

    /// Create an I/O error
    pub fn io_error<E: fmt::Display>(msg: E) -> Self {
        Self::Database(msg.to_string()) // Use Database for now as it's closest
    }

    /// Create an operation error
    pub fn operation<E: fmt::Display>(msg: E) -> Self {
        Self::Other(anyhow::anyhow!(msg.to_string()))
    }

    /// Check if this is a retryable error
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Io(_)
                | Self::Database(_)
                | Self::Lock(_)
                | Self::Unavailable(_)
                | Self::Connection(_)
                | Self::Timeout(_)
        )
    }

    /// Check if this is a conflict error
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict(_))
    }

    /// Check if this is a not found error
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(err: serde_json::Error) -> Self {
        Self::serialization(err)
    }
}

impl From<serde_yaml::Error> for StorageError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::serialization(err)
    }
}

/// Convert StorageError to ProdigyError
impl From<StorageError> for ProdigyError {
    fn from(err: StorageError) -> Self {
        let (code, path) = match &err {
            StorageError::Io(_) => (ErrorCode::STORAGE_IO_ERROR, None),
            StorageError::Serialization(_) => (ErrorCode::STORAGE_SERIALIZATION_ERROR, None),
            StorageError::Database(_) => (ErrorCode::STORAGE_BACKEND_ERROR, None),
            StorageError::Lock(_) => (ErrorCode::STORAGE_LOCK_FAILED, None),
            StorageError::NotFound(_) => (ErrorCode::STORAGE_NOT_FOUND, None),
            StorageError::Conflict(_) => (ErrorCode::STORAGE_ALREADY_EXISTS, None),
            StorageError::Unavailable(_) => (ErrorCode::STORAGE_BACKEND_ERROR, None),
            StorageError::Configuration(_) => (ErrorCode::STORAGE_BACKEND_ERROR, None),
            StorageError::Transaction(_) => (ErrorCode::STORAGE_BACKEND_ERROR, None),
            StorageError::Connection(_) => (ErrorCode::STORAGE_BACKEND_ERROR, None),
            StorageError::Timeout(_) => (ErrorCode::STORAGE_TEMPORARY, None),
            StorageError::Other(_) => (ErrorCode::STORAGE_GENERIC, None),
        };

        ProdigyError::storage_with_code(code, err.to_string(), path)
            .with_source(err)
    }
}
