use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Project error: {0}")]
    Project(String),

    #[error("Specification error: {0}")]
    Specification(String),

    #[error("Specification error: {0}")]
    Spec(String),

    #[error("Command error: {0}")]
    Command(String),

    #[error("Workflow error: {0}")]
    Workflow(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),

    #[error("Other error: {0}")]
    Other(String),

    #[error("External API error: {0}")]
    External(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, Error>;
