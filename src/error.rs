use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Project error: {0}")]
    Project(String),

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

    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("Chrono parse error: {0}")]
    ChronoParse(#[from] chrono::ParseError),

    #[error("Chrono out of range error: {0}")]
    ChronoOutOfRange(#[from] chrono::OutOfRangeError),

    #[error("HTTP status error: {0}")]
    HttpStatus(String),

    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Template error: {0}")]
    Template(#[from] tera::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Anyhow error: {0}")]
    Anyhow(#[from] anyhow::Error),

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

    // Plugin-related errors
    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    #[error("Plugin already exists: {0}")]
    PluginAlreadyExists(String),

    #[error("Invalid plugin: {0}")]
    InvalidPlugin(String),

    #[error("Plugin execution error: {0}")]
    PluginExecution(String),

    #[error("Plugin timeout: {0}")]
    PluginTimeout(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid version: {0}")]
    InvalidVersion(String),

    #[error("Incompatible version: {0}")]
    IncompatibleVersion(String),

    #[error("Missing dependency: {0}")]
    MissingDependency(String),

    #[error("Dependency conflict: {0}")]
    DependencyConflict(String),

    #[error("Circular dependency: {0}")]
    CircularDependency(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<axum::http::StatusCode> for Error {
    fn from(status: axum::http::StatusCode) -> Self {
        Error::HttpStatus(format!("HTTP status: {status}"))
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Other(s)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
